use axum::Router;
use axum::extract::{Multipart, State};
use axum::response::IntoResponse;
use axum::routing::post;
use hyper::HeaderMap;
use hyper::header::CONTENT_TYPE;

use crate::error::ApiResult;
use crate::server::appstate::AppState;

async fn post_updater(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<impl IntoResponse> {
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap_or_default().to_string();
        let data = field.bytes().await.unwrap();

        log::info!("Length of `{}` is {} bytes", name, data.len());
    }

    // Reset version updater cache, and fetch newest version
    let version = {
        let updater = state.updater();
        let mut lock = updater.lock().await;
        lock.reset_cache();
        lock.get().await.clone()
    };

    // Patch bridge state with newest software version
    let mut lock = state.res.lock().await;
    lock.update_bridge_version(version);
    drop(lock);

    // This is the exact output needed to trick the hue app into thinking we're
    // a real hue bridge. After waiting a while, the app will check the bridge
    // again, and happily conclude that the firmware has been upgraded.
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, "text/html".parse().unwrap());
    Ok((headers, "Upload OK, system will reboot"))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(post_updater))
}
