use hydroxy_peer_wire_proto::errors::PeerWireProtocolError;
use hydroxy_state::errors::StateError;
use hydroxy_storage_io::errors::StorageIOError;
use hydroxy_tracker::errors::TrackerError;
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum EngineError {
    #[error("State Error: {0}")]
    Bencode(#[from] StateError),

    #[error("Tracker Error: {0}")]
    Tracker(#[from] TrackerError),

    #[error("State Error, {0}")]
    Error(String),

    #[error("IO/TCP Error: {0}")]
    TcpError(#[from] std::io::Error),

    #[error("PWP Error: {0}")]
    PWPError(#[from] PeerWireProtocolError),

    #[error("Storage I/O Error: {0}")]
    StorageIOError(#[from] StorageIOError),

    #[error("Peer connection timed out")]
    Timeout,
}
