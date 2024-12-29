use thiserror::Error;

#[derive(Debug, Error)]
pub enum PngError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Not ios png")]
    NotIosPng,
    #[error("{0}")]
    Error(String),
}
