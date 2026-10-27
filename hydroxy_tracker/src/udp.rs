use std::{net::Ipv4Addr, time::Duration};

use tokio::net::UdpSocket;

use crate::{Peer, Tracker, TrackerEvents, TrackerResponse, errors::TrackerError};

const MAGIC_CONSTANT: i64 = 0x41727101980;
const ACTION_CONNECT: i32 = 0;
const ACTION_ANNOUNCE: i32 = 1;

pub async fn announce(tracker: &Tracker) -> Result<TrackerResponse, TrackerError> {
    // host:port
    let tracker_addr = tracker
        .announce_url
        .trim_start_matches("udp://")
        .split("/")
        .next()
        .ok_or(TrackerError::InvalidUrl(tracker.announce_url.clone()))?;

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    tokio::time::timeout(Duration::from_secs(5), socket.connect(tracker_addr)).await??;

    // connecting
    let transaction_id: i32 = rand::random();
    let mut connect_req = [0u8; 16];
    connect_req[0..8].copy_from_slice(&MAGIC_CONSTANT.to_be_bytes());
    connect_req[8..12].copy_from_slice(&ACTION_CONNECT.to_be_bytes());
    connect_req[12..16].copy_from_slice(&transaction_id.to_be_bytes());

    socket.send(&connect_req).await?;

    let mut connect_resp = [0u8; 16];
    tokio::time::timeout(Duration::from_secs(5), socket.recv(&mut connect_resp)).await??;

    let connection_id = i64::from_be_bytes(connect_resp[8..16].try_into().unwrap());

    // announcing
    let mut announce_req = [0u8; 98];
    announce_req[0..8].copy_from_slice(&connection_id.to_be_bytes());
    announce_req[8..12].copy_from_slice(&ACTION_ANNOUNCE.to_be_bytes());
    announce_req[12..16].copy_from_slice(&transaction_id.to_be_bytes());
    announce_req[16..36].copy_from_slice(&tracker.info_hash);
    announce_req[36..56].copy_from_slice(&tracker.peer_id);

    // downloaded, left, uploaded (8 bytes each) — leave as 0
    announce_req[56..64].copy_from_slice(&tracker.downloaded.to_be_bytes());
    announce_req[64..72].copy_from_slice(&tracker.remaining_size.to_be_bytes());
    announce_req[72..80].copy_from_slice(&tracker.uploaded.to_be_bytes());

    // event
    let event = match tracker.event {
        Some(TrackerEvents::Completed) => 1i32,
        Some(TrackerEvents::Started) => 2i32,
        Some(TrackerEvents::Stopped) => 3i32,
        None => 0i32,
    };
    announce_req[80..84].copy_from_slice(&event.to_be_bytes());
    // num_want: -1 = default
    announce_req[92..94].copy_from_slice(&(-1i16).to_be_bytes());
    announce_req[96..98].copy_from_slice(&1111u16.to_be_bytes());

    socket.send(&announce_req).await?;

    let mut announce_resp = vec![0u8; 1024];
    let n = tokio::time::timeout(Duration::from_secs(5), socket.recv(&mut announce_resp)).await??;

    // checking header is atlest 20 bytes or not
    if n < 20 {
        return Err(TrackerError::TrackerResponseParseError(
            "Response packet too short (Less than 20 Bytes)".to_string(),
        ));
    }

    let interval = u32::from_be_bytes([
        announce_resp[8],
        announce_resp[9],
        announce_resp[10],
        announce_resp[11],
    ]) as usize;

    let incomplete = u32::from_be_bytes([
        announce_resp[12],
        announce_resp[13],
        announce_resp[14],
        announce_resp[15],
    ]) as usize;

    let complete = u32::from_be_bytes([
        announce_resp[16],
        announce_resp[17],
        announce_resp[18],
        announce_resp[19],
    ]) as usize;

    // parsing peers from response
    // each peer is 6 Bytes (4 <IP> + 3 <Port>)
    let mut peers = Vec::new();
    let peer_data = &announce_resp[20..n];
    for chunk in peer_data.chunks_exact(6) {
        let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
        let port = u16::from_be_bytes([chunk[4], chunk[5]]);
        if !ip.is_unspecified() && !ip.is_broadcast() && !ip.is_loopback() && port != 0 {
            peers.push(Peer {
                ipv4_addr: ip,
                port,
            });
        }
    }

    Ok(TrackerResponse {
        complete,
        downloaded: 0,
        incomplete,
        interval,
        ipv4_peers: peers,
        ipv6_peers: Vec::new(),
        min_interval: 0,
    })
}
