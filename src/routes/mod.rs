use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::response::{IntoResponse, Response};
use hue::error::{HueApiV1Error, HueError};
use hyper::StatusCode;
use serde_json::{Value, json};
use thiserror::Error;

use crate::error::ApiError;
use crate::routes::clip::{V2Error, V2Reply};
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

pub mod api;
pub mod auth;
pub mod bifrost;
pub mod clip;
pub mod eventstream;
pub mod extractor;
pub mod licenses;
pub mod updater;
pub mod upnp;

#[derive(Error, Debug)]
pub enum ApiV1Error {
    #[error(transparent)]
    ApiError(#[from] ApiError),

    #[error(transparent)]
    HueError(#[from] HueError),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    HueApiV1(#[from] HueApiV1Error),
}

impl ApiV1Error {
    pub const fn http_status_code(&self) -> StatusCode {
        // Hue bridge seems to return 200 OK in almost all cases, and use the
        // .error field to indicate the error.
        match self {
            Self::ApiError(_)
            | Self::HueError(_)
            | Self::SerdeJsonError(_)
            | Self::HueApiV1(
                HueApiV1Error::UnauthorizedUser
                | HueApiV1Error::BodyContainsInvalidJson
                | HueApiV1Error::ResourceNotfound
                | HueApiV1Error::MethodNotAvailableForResource
                | HueApiV1Error::MissingParametersInBody
                | HueApiV1Error::ParameterNotAvailable
                | HueApiV1Error::InvalidValueForParameter
                | HueApiV1Error::ParameterNotModifiable
                | HueApiV1Error::TooManyItemsInList
                | HueApiV1Error::PortalConnectionIsRequired,
            ) => StatusCode::OK,

            Self::HueApiV1(HueApiV1Error::BridgeInternalError) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub const fn hue_error_code(&self) -> u32 {
        match self {
            Self::HueError(HueError::V1NotFound(_) | HueError::WrongType(_, _)) => {
                HueApiV1Error::ResourceNotfound.error_code()
            }
            Self::HueApiV1(err) => err.error_code(),
            Self::ApiError(_) | Self::HueError(_) | Self::SerdeJsonError(_) => {
                HueApiV1Error::BridgeInternalError.error_code()
            }
        }
    }
}

type ApiV1Result<T> = Result<T, ApiV1Error>;

impl IntoResponse for ApiV1Error {
    fn into_response(self) -> Response {
        let error_msg = format!("{self}");
        log::error!("V1 request failed: {error_msg}");

        let res = Json(json!([
            {
                "error": {
                    "type": self.hue_error_code(),
                    "address": "/",
                    "description": format!("{self}"),
                }
            }
        ]));

        let status = self.http_status_code();

        (status, res).into_response()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error_msg = format!("{self}");
        log::error!("Request failed: {error_msg}");

        let res = Json(V2Reply::<Value> {
            data: vec![],
            errors: vec![V2Error {
                description: error_msg,
            }],
        });

        let status = match self {
            Self::HueError(err) => match err {
                HueError::FromUtf8Error(_)
                | HueError::SerdeJson(_)
                | HueError::TryFromIntError(_)
                | HueError::FromHexError(_)
                | HueError::PackedStructError(_)
                | HueError::UuidError(_)
                | HueError::HueEntertainmentBadHeader
                | HueError::HueZigbeeUnknownFlags(_) => StatusCode::BAD_REQUEST,

                HueError::NotFound(_) | HueError::V1NotFound(_) | HueError::WrongType(_, _) => {
                    StatusCode::NOT_FOUND
                }

                HueError::Full(_) => StatusCode::INSUFFICIENT_STORAGE,

                HueError::IOError(_)
                | HueError::HueZigbeeDecodeError
                | HueError::HueZigbeeEncodeError
                | HueError::Undiffable => StatusCode::INTERNAL_SERVER_ERROR,
            },

            Self::AuxNotFound(_) => StatusCode::NOT_FOUND,

            Self::CreateNotAllowed(_) | Self::UpdateNotAllowed(_) | Self::DeleteNotAllowed(_) => {
                StatusCode::METHOD_NOT_ALLOWED
            }

            Self::CreateNotYetSupported(_)
            | Self::UpdateNotYetSupported(_)
            | Self::DeleteNotYetSupported(_) => StatusCode::FORBIDDEN,

            Self::V1CreateUnsupported(_) => StatusCode::NOT_IMPLEMENTED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, res).into_response()
    }
}

pub fn router(appstate: AppState) -> Router<()> {
    Router::new()
        .nest("/api", api::router())
        .nest("/auth", auth::router())
        .nest("/updater", updater::router())
        .nest("/licenses", licenses::router())
        .nest("/description.xml", upnp::router())
        .nest("/clip/v2/resource", clip::router())
        .nest("/eventstream", eventstream::router())
        .nest("/bifrost", bifrost::router())
        .with_state(appstate)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}
