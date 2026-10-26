use anyhow::{Context, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::errors::PeerWireProtocolError;

const PROTOCOL_HANDSHAKE_STRING: &'static str = "BitTorrent protocol";

pub struct Handshake {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self { info_hash, peer_id }
    }

    pub fn to_bytes(&self) -> [u8; 68] {
        let mut buffer = [0u8; 68];
        // Protocol String Length [ 1 Byte ]
        buffer[0] = PROTOCOL_HANDSHAKE_STRING.len() as u8;
        // Protocol Handshake String [ 19 Bytes]
        buffer[1..20].copy_from_slice(PROTOCOL_HANDSHAKE_STRING.as_bytes());
        // {19 to 28} 8 Bytes are reserved/empty (used for extensions like DHT or UT_PEX).
        // so it will be Zero for now,
        // Info Hash will take 20 Bytes {28 to 48}
        buffer[28..48].copy_from_slice(&self.info_hash);
        // Peer ID will take next 20 Bytes {48 to 68}
        buffer[48..68].copy_from_slice(&self.peer_id);
        buffer
    }

    pub fn from_bytes(bytes_buffer: &[u8; 68]) -> Result<Self, PeerWireProtocolError> {
        if bytes_buffer[0] != PROTOCOL_HANDSHAKE_STRING.len() as u8 {
            return Err(PeerWireProtocolError::InvalidProtocolLength(
                PROTOCOL_HANDSHAKE_STRING.len() as u8,
                bytes_buffer[0],
            ));
        }

        if &bytes_buffer[1..20] != PROTOCOL_HANDSHAKE_STRING.as_bytes() {
            return Err(PeerWireProtocolError::InvalidHandshakeBytes(
                "
            Error validating Protocol String 
            "
                .into(),
            ));
        }

        let mut info_hash = [0u8; 20];
        let mut peer_id = [0u8; 20];

        info_hash.copy_from_slice(&bytes_buffer[28..48]);
        peer_id.copy_from_slice(&bytes_buffer[48..68]);

        Ok(Handshake { info_hash, peer_id })
    }

    pub async fn do_handshake(
        self,
        tcp_stream: &mut TcpStream,
    ) -> Result<Handshake, PeerWireProtocolError> {
        // let out_going_handshake = Handshake::new(self.info_hash, self.peer_id);
        tcp_stream
            .write_all(
                // out_going_handshake
                &self.to_bytes(),
            )
            .await
            .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;

        let mut response_buffer = [0u8; 68];
        tcp_stream
            .read_exact(&mut response_buffer)
            .await
            .context("Failed to read TCP Handshake from peer.")
            .map_err(|e| PeerWireProtocolError::NetIOError(e.to_string()))?;

        let incomming_handshake = Handshake::from_bytes(&response_buffer)?;

        if incomming_handshake.info_hash != self.info_hash {
            return Err(PeerWireProtocolError::HandshakeFailed);
        }

        Ok(incomming_handshake)
    }
}
