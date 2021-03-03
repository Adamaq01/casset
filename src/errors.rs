use thiserror::Error;

#[derive(Error, Debug)]
pub enum CassetError {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    HotwatchError(#[from] hotwatch::Error),
    #[error("Metadata is required for loading but none was given")]
    MetadataRequired,
    #[error("Incorrect Metadata was given for loading asset")]
    IncorrectMetadata,
    #[error("Error while running reload routine {0}")]
    ReloadError(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CassetError>;
