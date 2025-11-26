use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    BinError(#[from] binrw::Error),
    #[error("Not ios png")]
    NotIosPng,
    #[error("{0}")]
    Error(String),
}
impl<T> Into<Result<T, Error>> for Error {
    fn into(self) -> Result<T, Error> {
        Err(self)
    }
}
