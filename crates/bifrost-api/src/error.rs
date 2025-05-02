use thiserror::Error;

#[derive(Error, Debug)]
pub enum BifrostError {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),

    #[error("Server error: {0}")]
    ServerError(String),
}

pub type BifrostResult<T> = Result<T, BifrostError>;
