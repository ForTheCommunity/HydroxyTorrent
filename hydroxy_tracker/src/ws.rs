use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use base64::Engine;
use rustls_pki_types::ServerName;
use serde_json::Value;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
};
use tokio_rustls::{
    TlsConnector,
    rustls::{ClientConfig, RootCertStore},
};
use url::Url;

use crate::{
    Peer, Tracker, TrackerEvents, TrackerResponse,
    errors::TrackerError,
    http_s,
};

enum WsStream {
    Plain(TcpStream),
    Tls(tokio_rustls::client::TlsStream<TcpStream>),
}

impl AsyncRead for WsStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WsStream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            WsStream::Tls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for WsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            WsStream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            WsStream::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WsStream::Plain(s) => Pin::new(s).poll_flush(cx),
            WsStream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            WsStream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            WsStream::Tls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

fn shared_tls_connector() -> &'static TlsConnector {
    static CONNECTOR: std::sync::OnceLock<TlsConnector> = std::sync::OnceLock::new();
    CONNECTOR.get_or_init(|| {
        let mut root_store = RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let client_config = ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        TlsConnector::from(Arc::new(client_config))
    })
}

pub async fn announce(tracker: &Tracker) -> Result<TrackerResponse, TrackerError> {
    let parsed_url = Url::parse(&tracker.announce_url)
        .map_err(|e| TrackerError::InvalidUrl(format!("Failed to parse URL: {}", e)))?;

    let is_secure = match parsed_url.scheme() {
        "ws" => false,
        "wss" => true,
        other => {
            return Err(TrackerError::InvalidUrl(format!(
                "Unsupported scheme for websocket tracker: {}",
                other
            )));
        }
    };

    let host = parsed_url
        .host_str()
        .ok_or_else(|| TrackerError::InvalidUrl("Missing host in announce URL".into()))?;

    let default_port = if is_secure { 443 } else { 80 };
    let port = parsed_url.port().unwrap_or(default_port);

    // Establish TCP connection
    let addr_str = format!("{}:{}", host, port);
    let tcp_stream = tokio::time::timeout(
        Duration::from_secs(8),
        TcpStream::connect(&addr_str),
    )
    .await
    .map_err(|_| TrackerError::NetworkError(format!("Connection timeout to {}", addr_str)))?
    .map_err(|e| TrackerError::NetworkError(format!("Failed to connect to {}: {}", addr_str, e)))?;

    // Establish TLS if wss://
    let mut stream = if is_secure {
        let connector = shared_tls_connector();
        let server_name = ServerName::try_from(host.to_string())
            .map_err(|e| TrackerError::InvalidUrl(format!("Invalid DNS name {}: {}", host, e)))?;

        let tls_stream = tokio::time::timeout(
            Duration::from_secs(8),
            connector.connect(server_name, tcp_stream),
        )
        .await
        .map_err(|_| TrackerError::NetworkError("TLS handshake timeout".into()))?
        .map_err(|e| TrackerError::NetworkError(format!("TLS handshake failed: {}", e)))?;

        WsStream::Tls(tls_stream)
    } else {
        WsStream::Plain(tcp_stream)
    };

    // WebSocket Opening Handshake
    let mut nonce = [0u8; 16];
    rand::fill(&mut nonce);
    let sec_websocket_key = base64::engine::general_purpose::STANDARD.encode(nonce);

    let host_header = if parsed_url.port().is_some() {
        format!("{}:{}", host, port)
    } else {
        host.to_string()
    };

    let path_and_query = match parsed_url.query() {
        Some(q) => format!("{}?{}", parsed_url.path(), q),
        None => parsed_url.path().to_string(),
    };
    let request_path = if path_and_query.is_empty() {
        "/".to_string()
    } else {
        path_and_query
    };

    let handshake_req = format!(
        "GET {} HTTP/1.1\r\n\
         Host: {}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         User-Agent: HydroxyTorrent/0.1.0\r\n\
         \r\n",
        request_path, host_header, sec_websocket_key
    );

    stream
        .write_all(handshake_req.as_bytes())
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;
    stream
        .flush()
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;

    // Read HTTP 101 response headers
    let mut resp_header = Vec::new();
    let mut byte_buf = [0u8; 1];
    while !resp_header.ends_with(b"\r\n\r\n") {
        tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut byte_buf))
            .await
            .map_err(|_| TrackerError::NetworkError("Timeout reading handshake response".into()))?
            .map_err(|e| TrackerError::NetworkError(e.to_string()))?;

        resp_header.push(byte_buf[0]);
        if resp_header.len() > 8192 {
            return Err(TrackerError::NetworkError(
                "WebSocket handshake response header too long".into(),
            ));
        }
    }

    let resp_str = String::from_utf8_lossy(&resp_header);
    if !resp_str.contains("101 Switching Protocols")
        && !resp_str.starts_with("HTTP/1.1 101")
        && !resp_str.starts_with("HTTP/1.0 101")
    {
        return Err(TrackerError::NetworkError(format!(
            "WebSocket upgrade rejected: {}",
            resp_str.lines().next().unwrap_or("Unknown error")
        )));
    }

    // Send Announce JSON Frame
    // Format info_hash & peer_id as 20-character binary string per WebTorrent spec
    let binary_info_hash: String = tracker.info_hash.iter().map(|&b| b as char).collect();
    let binary_peer_id: String = tracker.peer_id.iter().map(|&b| b as char).collect();

    let mut announce_json = serde_json::json!({
        "action": "announce",
        "info_hash": binary_info_hash,
        "peer_id": binary_peer_id,
        "uploaded": tracker.uploaded,
        "downloaded": tracker.downloaded,
        "left": tracker.remaining_size,
        "numwant": 50,
    });

    if let Some(event) = tracker.event {
        let ev_str = match event {
            TrackerEvents::Started => "started",
            TrackerEvents::Completed => "completed",
            TrackerEvents::Stopped => "stopped",
        };
        announce_json["event"] = serde_json::Value::String(ev_str.to_string());
    }

    let req_payload = announce_json.to_string();
    write_frame(&mut stream, 0x1, req_payload.as_bytes()).await?;

    // Read Announce Response Frame
    let (opcode, payload) = tokio::time::timeout(
        Duration::from_secs(10),
        read_frame(&mut stream),
    )
    .await
    .map_err(|_| TrackerError::NetworkError("Timeout waiting for tracker response frame".into()))??;

    // Send close frame politely
    write_frame(&mut stream, 0x8, &[]).await.ok();

    // Parse Tracker Response
    if opcode == 0x8 {
        return Err(TrackerError::TrackerResponseParseError(
            "WebSocket tracker closed connection unexpectedly".into(),
        ));
    }

    parse_ws_response(&payload)
}

fn parse_ws_response(payload: &[u8]) -> Result<TrackerResponse, TrackerError> {
    // Attempt 1: JSON payload (standard WebTorrent tracker)
    if let Ok(json_val) = serde_json::from_slice::<Value>(payload) {
        if let Some(failure) = json_val.get("failure reason").and_then(|v| v.as_str()) {
            return Err(TrackerError::TrackerResponseParseError(format!(
                "Tracker returned failure: {}",
                failure
            )));
        }

        let interval = json_val
            .get("interval")
            .and_then(|v| v.as_u64())
            .unwrap_or(120) as usize;

        let min_interval = json_val
            .get("min interval")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let complete = json_val
            .get("complete")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let incomplete = json_val
            .get("incomplete")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let mut ipv4_peers = Vec::new();

        if let Some(peers_arr) = json_val.get("peers").and_then(|v| v.as_array()) {
            for item in peers_arr {
                if let Some(ip_str) = item
                    .get("ip")
                    .or_else(|| item.get("ipv4"))
                    .or_else(|| item.get("addr"))
                    .and_then(|v| v.as_str())
                {
                    let port = item.get("port").and_then(|v| v.as_u64()).unwrap_or(6881) as u16;
                    if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                        ipv4_peers.push(Peer {
                            ipv4_addr: ip,
                            port,
                        });
                    }
                } else if let Some(addr_str) = item.as_str() {
                    if let Ok(socket_addr) = addr_str.parse::<SocketAddr>() {
                        if let SocketAddr::V4(v4) = socket_addr {
                            ipv4_peers.push(Peer {
                                ipv4_addr: *v4.ip(),
                                port: v4.port(),
                            });
                        }
                    }
                }
            }
        } else if let Some(peers_str) = json_val.get("peers").and_then(|v| v.as_str()) {
            let bytes: Vec<u8> = peers_str.chars().map(|c| c as u8).collect();
            ipv4_peers.extend(Peer::parse_peers(&bytes));
        }

        return Ok(TrackerResponse {
            complete,
            downloaded: 0,
            incomplete,
            interval,
            min_interval,
            ipv4_peers,
            ipv6_peers: Vec::new(),
        });
    }

    // Attempt 2: Bencoded payload fallback
    if let Ok(bencoded) = hydroxy_bencode::decode::decode(payload) {
        return http_s::convert(&bencoded);
    }

    Err(TrackerError::TrackerResponseParseError(
        "Could not decode tracker response as JSON or Bencoded dictionary".into(),
    ))
}

async fn write_frame<S: AsyncWrite + Unpin>(
    stream: &mut S,
    opcode: u8,
    payload: &[u8],
) -> Result<(), TrackerError> {
    let mut header = Vec::with_capacity(14);
    header.push(0x80 | (opcode & 0x0F)); // FIN = 1

    let len = payload.len();
    if len < 126 {
        header.push(0x80 | (len as u8)); // Mask bit = 1
    } else if len <= 65535 {
        header.push(0x80 | 126);
        header.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        header.push(0x80 | 127);
        header.extend_from_slice(&(len as u64).to_be_bytes());
    }

    let mask_key: [u8; 4] = rand::random();
    header.extend_from_slice(&mask_key);

    let mut masked_payload = Vec::with_capacity(len);
    for (i, &b) in payload.iter().enumerate() {
        masked_payload.push(b ^ mask_key[i % 4]);
    }

    stream
        .write_all(&header)
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;
    stream
        .write_all(&masked_payload)
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;
    stream
        .flush()
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;

    Ok(())
}

async fn read_frame<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
) -> Result<(u8, Vec<u8>), TrackerError> {
    let mut header = [0u8; 2];
    stream
        .read_exact(&mut header)
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;

    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;
    let len_code = header[1] & 0x7F;

    let payload_len: usize = match len_code {
        126 => {
            let mut len_bytes = [0u8; 2];
            stream
                .read_exact(&mut len_bytes)
                .await
                .map_err(|e| TrackerError::NetworkError(e.to_string()))?;
            u16::from_be_bytes(len_bytes) as usize
        }
        127 => {
            let mut len_bytes = [0u8; 8];
            stream
                .read_exact(&mut len_bytes)
                .await
                .map_err(|e| TrackerError::NetworkError(e.to_string()))?;
            u64::from_be_bytes(len_bytes) as usize
        }
        other => other as usize,
    };

    if payload_len > 10 * 1024 * 1024 {
        return Err(TrackerError::TrackerResponseParseError(
            "WebSocket frame payload exceeds limit (10MB)".into(),
        ));
    }

    let mask_key = if masked {
        let mut m = [0u8; 4];
        stream
            .read_exact(&mut m)
            .await
            .map_err(|e| TrackerError::NetworkError(e.to_string()))?;
        Some(m)
    } else {
        None
    };

    let mut payload = vec![0u8; payload_len];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;

    if let Some(mask) = mask_key {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= mask[i % 4];
        }
    }

    // Ping frame: reply with Pong and continue reading
    if opcode == 0x9 {
        write_frame(stream, 0xA, &payload).await?;
        return Box::pin(read_frame(stream)).await;
    }

    Ok((opcode, payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_ws_response() {
        let json_data = r#"{
            "action": "announce",
            "interval": 120,
            "complete": 10,
            "incomplete": 2,
            "peers": [
                {"ip": "192.168.1.100", "port": 6881},
                {"ip": "10.0.0.5", "port": 51413}
            ]
        }"#;

        let resp = parse_ws_response(json_data.as_bytes()).unwrap();
        assert_eq!(resp.interval, 120);
        assert_eq!(resp.complete, 10);
        assert_eq!(resp.incomplete, 2);
        assert_eq!(resp.ipv4_peers.len(), 2);
        assert_eq!(
            resp.ipv4_peers[0].ipv4_addr,
            Ipv4Addr::new(192, 168, 1, 100)
        );
        assert_eq!(resp.ipv4_peers[0].port, 6881);
    }
}
