use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Config Error, {0}")]
    Error(String),
}
