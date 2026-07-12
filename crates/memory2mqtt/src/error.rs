use thiserror::Error;

#[derive(Error, Debug)]
pub enum M2MError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    TungsteniteError(#[from] tungstenite::Error),
}

pub type M2MResult<T> = Result<T, M2MError>;
