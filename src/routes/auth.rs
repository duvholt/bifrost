use axum::http::HeaderValue;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use hyper::HeaderMap;
use serde_json::json;

use crate::hue::api::HueStreamKey;
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

pub const STANDARD_APPLICATION_ID: &str = "01010101-0202-0303-0404-050505050505";

/// This 16-byte key is used for all DTLS entertainment streams
pub const STANDARD_CLIENT_KEY: HueStreamKey = HueStreamKey::new(*b"BifrostHueTlsKey");

pub async fn auth_v1() -> impl IntoResponse {
    let value = HeaderValue::from_static(STANDARD_APPLICATION_ID);

    let mut headers = HeaderMap::new();
    headers.append("hue-application-id", value);

    (headers, Json(json!({})))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1", get(auth_v1))
}
