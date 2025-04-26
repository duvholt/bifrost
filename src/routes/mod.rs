use axum::extract::DefaultBodyLimit;
use axum::response::{IntoResponse, Response};
use axum::Router;
use hue::error::HueError;
use hyper::StatusCode;
use serde_json::Value;

use crate::error::ApiError;
use crate::routes::clip::{V2Error, V2Reply};
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

pub mod api;
pub mod auth;
pub mod clip;
pub mod eventstream;
pub mod extractor;
pub mod licenses;
pub mod updater;
pub mod upnp;

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

                HueError::UpdateUnsupported(_) | HueError::WrongType(_, _) => {
                    StatusCode::NOT_ACCEPTABLE
                }

                HueError::NotFound(_) | HueError::V1NotFound(_) | HueError::AuxNotFound(_) => {
                    StatusCode::NOT_FOUND
                }

                HueError::Full(_) => StatusCode::INSUFFICIENT_STORAGE,

                HueError::IOError(_) | HueError::HueZigbeeDecodeError | HueError::Undiffable => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            },

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
        .with_state(appstate)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
}
