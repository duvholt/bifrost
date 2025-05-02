use std::num::{ParseIntError, TryFromIntError};
use std::sync::Arc;

use camino::Utf8PathBuf;
use hue::api::RType;
use thiserror::Error;
use tokio::task::JoinError;

use bifrost_api::backend::BackendRequest;
use hue::event::EventBlock;
use hue::legacy_api::ApiResourceType;
use svc::error::SvcError;

#[derive(Error, Debug)]
pub enum ApiError {
    /* mapped errors */
    #[error(transparent)]
    FromUtf8Error(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    SerdeYaml(#[from] serde_yml::Error),

    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[error(transparent)]
    JoinError(#[from] JoinError),

    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),

    #[error(transparent)]
    TryFromIntError(#[from] TryFromIntError),

    #[error(transparent)]
    FromHexError(#[from] hex::FromHexError),

    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),

    #[error(transparent)]
    MdnsSdError(#[from] mdns_sd::Error),

    #[error(transparent)]
    ConfigError(#[from] config::ConfigError),

    #[error(transparent)]
    QuickXmlSeError(#[from] quick_xml::se::SeError),

    #[error(transparent)]
    SendErrorHue(#[from] tokio::sync::broadcast::error::SendError<EventBlock>),

    #[error(transparent)]
    SendErrorZ2m(#[from] tokio::sync::broadcast::error::SendError<Arc<BackendRequest>>),

    #[error(transparent)]
    SetLoggerError(#[from] log::SetLoggerError),

    #[error(transparent)]
    BroadcastStreamRecvError(#[from] tokio_stream::wrappers::errors::BroadcastStreamRecvError),

    #[error(transparent)]
    TokioRecvError(#[from] tokio::sync::broadcast::error::RecvError),

    #[error(transparent)]
    AxumError(#[from] axum::Error),

    #[error(transparent)]
    TungsteniteError(#[from] tokio_tungstenite::tungstenite::Error),

    #[error(transparent)]
    X509DerError(#[from] der::Error),

    #[error(transparent)]
    X509SpkiError(#[from] x509_cert::spki::Error),

    #[error(transparent)]
    X509BuilderError(#[from] x509_cert::builder::Error),

    #[error(transparent)]
    X509DerConstOidError(#[from] der::oid::Error),

    #[error(transparent)]
    P256Pkcs8Error(#[from] p256::pkcs8::Error),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    UuidError(#[from] uuid::Error),

    #[error(transparent)]
    HueError(#[from] hue::error::HueError),

    #[error(transparent)]
    OpenSslError(#[from] openssl::error::Error),

    #[error(transparent)]
    OpenSslErrors(#[from] openssl::error::ErrorStack),

    #[error(transparent)]
    SslError(#[from] openssl::ssl::Error),

    #[error(transparent)]
    NativeTlsError(#[from] native_tls::Error),

    #[error("Service error: {0}")]
    SvcError(String),

    /* zigbee2mqtt errors */
    #[error("Unexpected eof on z2m socket")]
    UnexpectedZ2mEof,

    #[error("Unexpected z2m message: {0:?}")]
    UnexpectedZ2mReply(tokio_tungstenite::tungstenite::Message),

    /* hue api v1 errors */
    #[error("Cannot create resources of type: {0:?}")]
    V1CreateUnsupported(ApiResourceType),

    /* hue api v2 errors */
    #[error("Failed to get firmware version reply from update server")]
    NoUpdateInformation,

    /* bifrost errors: routes */
    #[error("Creating object of type {0:?} is not yet supported by Bifrost")]
    CreateNotYetSupported(RType),

    #[error("Creating object of type {0:?} is not allowed by hue protocol")]
    CreateNotAllowed(RType),

    #[error("Updating object of type {0:?} is not yet supported by Bifrost")]
    UpdateNotYetSupported(RType),

    #[error("Updating object of type {0:?} is not allowed by hue protocol")]
    UpdateNotAllowed(RType),

    #[error("Deleting object of type {0:?} is not yet supported by Bifrost")]
    DeleteNotYetSupported(RType),

    #[error("Deleting object of type {0:?} is not allowed by hue protocol")]
    DeleteNotAllowed(RType),

    /* bifrost errors */
    #[error("Missing auxiliary data resource {0:?}")]
    AuxNotFound(uuid::Uuid),

    #[error("Cannot parse state file: no version field found")]
    StateVersionNotFound,

    #[error("Cannot load certificate: {0:?}")]
    Certificate(Utf8PathBuf, std::io::Error),

    #[error("Cannot load certificate: {0:?}")]
    CertificateOpenSSL(Utf8PathBuf, openssl::ssl::Error),

    #[error("Cannot parse certificate: {0:?}")]
    CertificateInvalid(Utf8PathBuf),

    #[error("Invalid hex color")]
    InvalidHexColor,

    #[error("Entertainment Stream init error")]
    EntStreamInitError,

    #[error("Entertainment Stream desynchronized")]
    EntStreamDesync,

    #[error("Invalid zigbee message")]
    ZigbeeMessageError,
}

impl From<SvcError> for ApiError {
    fn from(value: SvcError) -> Self {
        Self::SvcError(value.to_string())
    }
}

impl ApiError {
    #[allow(clippy::needless_pass_by_value)]
    pub fn service_error(value: impl ToString) -> Self {
        Self::SvcError(value.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
