use thiserror::Error;

#[derive(Debug, Error)]
pub enum DHTError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Bencode decode error: {0}")]
    BencodeDecode(#[from] hydroxy_bencode::errors::DecodeError),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Invalid compact node info: {0}")]
    InvalidCompactNodeInfo(String),

    #[error("Invalid compact peer info: {0}")]
    InvalidCompactPeerInfo(String),

    #[error("Query timeout for transaction: {0:?}")]
    Timeout(Vec<u8>),

    #[error("Node returned KRPC error {code}: {message}")]
    KrpcError { code: i64, message: String },

    #[error("Channel closed")]
    ChannelClosed,
}
