use bifrost::error::ApiError;
use svc::error::SvcError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TestError {
    #[error(transparent)]
    Svc(#[from] SvcError),

    #[error(transparent)]
    Api(#[from] ApiError),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Parse(#[from] url::ParseError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    EventStreamError(#[from] eventsource_stream::EventStreamError<reqwest::Error>),

    #[error(transparent)]
    Elapsed(#[from] tokio::time::error::Elapsed),

    #[error("Timed out waiting for event")]
    EventTimeout,
}
pub type TestResult<T> = Result<T, TestError>;
