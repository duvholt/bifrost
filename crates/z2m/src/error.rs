use thiserror::Error;

#[derive(Error, Debug)]
pub enum Z2mError {
    /* mapped errors */
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    HueError(#[from] hue::error::HueError),

    #[error("Invalid hex color")]
    InvalidHexColor,
}

pub type Z2mResult<T> = Result<T, Z2mError>;
