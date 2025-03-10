use axum::response::{IntoResponse, Response};
use axum::Router;
use hue::error::HueError;
use hyper::StatusCode;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::routes::clip::V2Reply;
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

pub mod api;
pub mod auth;
pub mod clip;
pub mod eventstream;
pub mod extractor;
pub mod licenses;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error_msg = format!("{self}");
        log::error!("Request failed: {error_msg}");
        let res = Json(V2Reply::<Value> {
            data: vec![],
            errors: vec![json!({"description": error_msg}).to_string()],
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

                HueError::IOError(_) | HueError::HueZigbeeDecodeError => {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            },
            Self::DeleteDenied(_) => StatusCode::FORBIDDEN,
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
        .nest("/licenses", licenses::router())
        .nest("/clip/v2/resource", clip::router())
        .nest("/eventstream", eventstream::router())
        .with_state(appstate)
}
