use bifrost::error::ApiError;
use hue::event::{Event, EventBlock};
use svc::error::SvcError;
use thiserror::Error;
use z2m::api::RawMessage;

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
    HueEventSend(#[from] tokio::sync::broadcast::error::SendError<Vec<EventBlock>>),

    #[error(transparent)]
    BroadcastRecvError(#[from] tokio::sync::broadcast::error::RecvError),

    #[error(transparent)]
    Z2mSend(#[from] tokio::sync::broadcast::error::SendError<RawMessage>),

    #[error(transparent)]
    OneshotRecvError(#[from] tokio::sync::oneshot::error::RecvError),

    #[error("unexpected Z2M requests\nexpected: {expected:#?}\nactual: {actual:#?}")]
    UnexpectedZ2mRequests {
        expected: Vec<RawMessage>,
        actual: Vec<RawMessage>,
    },

    #[error("timeout waiting for Hue event: {0}")]
    HueEventTimeout(String),

    #[error("timeout waiting for z2m request")]
    Z2mRequestTimeout,

    #[error("unexpected Hue events: {0:?}")]
    UnexpectedHueEvents(Vec<Event>),
}
pub type TestResult<T> = Result<T, TestError>;
