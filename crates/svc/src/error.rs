use thiserror::Error;
use uuid::Uuid;

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
    MpscSendError(#[from] tokio::sync::mpsc::error::SendError<(Uuid, ServiceState)>),

    #[error(transparent)]
    WatchSendError(#[from] tokio::sync::watch::error::SendError<ServiceState>),

    #[error(transparent)]
    WatchRecvError(#[from] tokio::sync::watch::error::RecvError),

    #[error("Service {0} not registered")]
    ServiceNameNotFound(String),

    #[error("Service {0} already exists")]
    ServiceAlreadyExists(String),

    #[error("All services stopped")]
    Shutdown,
}

#[derive(Error, Debug)]
pub enum RunSvcError<E> {
    /* mapped errors */
    #[error(transparent)]
    MpscSendError(#[from] tokio::sync::mpsc::error::SendError<(Uuid, ServiceState)>),

    #[error(transparent)]
    WatchSendError(#[from] tokio::sync::watch::error::SendError<ServiceState>),

    #[error(transparent)]
    WatchRecvError(#[from] tokio::sync::watch::error::RecvError),

    #[error(transparent)]
    ServiceError(E),
}

pub type SvcResult<T> = Result<T, SvcError>;
