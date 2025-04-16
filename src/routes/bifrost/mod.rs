pub mod service;

use std::error::Error;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use hyper::StatusCode;
use serde::Serialize;
use serde_json::json;

use bifrost_api::config::AppConfig;

use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

#[derive(Debug, Serialize)]
/// Simple bifrost api error wrapper.
///
/// Bifrost API results need to implement [`IntoResponse`], but since
/// [`BifrostError`] comes from [`bifrost_api`], we can't implement
/// [`IntoResponse`] for it, without gaining a dependency on [`axum`] for that
/// crate. So for now, we use this thin wrapper for an [`IntoResponse`] impl.
struct BifrostApiError(String);

type BifrostApiResult<T> = Result<T, BifrostApiError>;

impl<E: Error> From<E> for BifrostApiError {
    fn from(value: E) -> Self {
        Self(value.to_string())
    }
}

impl IntoResponse for BifrostApiError {
    fn into_response(self) -> Response {
        log::error!("Request failed: {}", self.0);

        let res = json!({"error": self.0});

        (StatusCode::INTERNAL_SERVER_ERROR, Json(res)).into_response()
    }
}

async fn get_config(State(state): State<AppState>) -> BifrostApiResult<Json<AppConfig>> {
    Ok(Json((*state.config()).clone()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/service", service::router())
        .route("/config", get(get_config))
}
