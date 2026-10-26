use std::{num::ParseIntError, str::Utf8Error};

use hydroxy_bencode::errors::DecodeError;
use hydroxy_config::errors::ConfigError;
use hydroxy_storage_io::errors::StorageIOError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("IO Error: {0}")]
    Utf8Error(#[from] Utf8Error),

    #[error("IO Error: {0}")]
    IntParseError(#[from] ParseIntError),

    #[error("Bencode Error: {0}")]
    Bencode(#[from] DecodeError),

    #[error("Storage IO Error: {0}")]
    StorageIO(#[from] StorageIOError),

    #[error("Storage IO Error: {0}")]
    ConfigError(#[from] ConfigError),

    #[error("State Error, {0}")]
    Error(String),
}
