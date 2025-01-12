use axum::extract::Multipart;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;

use crate::error::ApiResult;
use crate::server::appstate::AppState;

async fn post_updater(mut multipart: Multipart) -> ApiResult<impl IntoResponse> {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let data = field.bytes().await.unwrap();

        log::info!("Length of `{}` is {} bytes", name, data.len());
    }

    Ok(Json(json!({})))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(post_updater))
}
