use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::Router;
use serde_json::Value;
use uuid::Uuid;

use hue::api::{Light, LightUpdate, RType};

use crate::backend::BackendRequest;
use crate::routes::clip::generic::get_resource;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

async fn put_light(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(put): Json<Value>,
) -> ApiV2Result {
    log::info!("PUT light/{id}");
    log::debug!("json data\n{}", serde_json::to_string_pretty(&put)?);

    let rlink = RType::Light.link_to(id);
    let lock = state.res.lock().await;

    let _ = lock.get::<Light>(&rlink)?;

    let upd: LightUpdate = serde_json::from_value(put)?;

    lock.backend_request(BackendRequest::LightUpdate(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}

async fn get_light(State(state): State<AppState>, Path(id): Path<Uuid>) -> ApiV2Result {
    V2Reply::ok(state.res.lock().await.get_resource(RType::Light, &id)?)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(|state| get_resource(state, Path(RType::Light))))
        .route("/{id}", get(get_light))
        .route("/{id}", put(put_light))
}
