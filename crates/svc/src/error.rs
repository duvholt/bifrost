use std::error::Error;

use thiserror::Error;

use crate::manager::{ServiceEvent, SvmRequest};
use crate::traits::ServiceState;

#[derive(Error, Debug)]
pub enum SvcError {
    /* mapped errors */
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    TryFromIntError(#[from] std::num::TryFromIntError),

    #[error(transparent)]
    UuidError(#[from] uuid::Error),

    #[error(transparent)]
    MpscSendError(#[from] tokio::sync::mpsc::error::SendError<SvmRequest>),

    #[error(transparent)]
    MpscSendEventError(#[from] tokio::sync::mpsc::error::SendError<ServiceEvent>),

    #[error(transparent)]
    WatchSendError(#[from] tokio::sync::watch::error::SendError<ServiceState>),

    #[error(transparent)]
    WatchRecvError(#[from] tokio::sync::watch::error::RecvError),

    #[error(transparent)]
    OneshotRecvError(#[from] tokio::sync::oneshot::error::RecvError),

    #[error("Service {0} not registered")]
    ServiceNameNotFound(String),

    #[error("Service {0} already exists")]
    ServiceAlreadyExists(String),

    #[error("All services stopped")]
    Shutdown,

    #[error("Service has failed")]
    ServiceFailed,
}

#[derive(Error, Debug)]
pub enum RunSvcError {
    /* mapped errors */
    #[error(transparent)]
    MpscSendError(#[from] tokio::sync::mpsc::error::SendError<SvmRequest>),

    #[error(transparent)]
    WatchSendError(#[from] tokio::sync::watch::error::SendError<ServiceState>),

    #[error(transparent)]
    MpscSendEventError(#[from] tokio::sync::mpsc::error::SendError<ServiceEvent>),

    #[error(transparent)]
    WatchRecvError(#[from] tokio::sync::watch::error::RecvError),

    /* errors from run service */
    #[error(transparent)]
    ServiceError(Box<dyn Error + Send>),
}

pub type SvcResult<T> = Result<T, SvcError>;
