use hydroxy_bencode::errors::DecodeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("Network Error : {0}")]
    NetworkError(String),

    #[error("Tracker Response Decode Error: {0}")]
    DecodeError(#[from] DecodeError),

    #[error("Error: {0}")]
    Error(#[from] std::io::Error),

    #[error("Request timed out: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),

    #[error("Tracker Response Parse Error : {0}")]
    TrackerResponseParseError(String),

    #[error("Invalid Announce Url : {0}")]
    InvalidUrl(String),
}
