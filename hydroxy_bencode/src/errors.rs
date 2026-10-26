use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("Empty Byte Sequence")]
    Empty,
    #[error("Invalid Bytes")]
    InvalidBytes,
    #[error("Invalid Integer")]
    InvalidInteger,
    #[error("Invalid String")]
    InvalidString,
    #[error("Invalid List")]
    InvalidList,
    #[error("Invalid Dictionary")]
    InvalidDictionary,
}
