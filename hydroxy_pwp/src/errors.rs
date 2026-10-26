use thiserror::Error;

#[derive(Debug, Error)]
pub enum PeerWireProtocolError {
    #[error("Network IO Error, {0}")]
    NetIOError(String),
    #[error("Failed to do Handshake")]
    HandshakeFailed,
    #[error("Invalid Handshake Protocol Length, Expected:{0}, Got:{1}")]
    InvalidProtocolLength(u8, u8),
    #[error("Invalid Handshake Bytes, {0}")]
    InvalidHandshakeBytes(String),
    #[error("Invalid Peer Message , {0}")]
    InvalidPeerMessage(String),
    #[error("IO/TCP Error: {0}")]
    TcpError(#[from] std::io::Error),
}
