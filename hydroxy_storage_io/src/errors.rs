use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageIOError {
    #[error("IO Error: {0}")]
    FileIoError(#[from] std::io::Error),
}
