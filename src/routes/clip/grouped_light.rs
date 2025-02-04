use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::Router;
use serde_json::Value;
use uuid::Uuid;

use crate::backend::BackendRequest;
use crate::hue::api::{GroupedLight, GroupedLightUpdate, RType, V2Reply};
use crate::routes::clip::generic::get_resource;
use crate::routes::clip::ApiV2Result;
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

async fn put_grouped_light(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(put): Json<Value>,
) -> ApiV2Result {
    log::info!("PUT grouped_light/{id}");
    log::debug!("json data\n{}", serde_json::to_string_pretty(&put)?);

    let rlink = RType::GroupedLight.link_to(id);
    let lock = state.res.lock().await;
    lock.get::<GroupedLight>(&rlink)?;

    log::info!("PUT grouped_light/{id}: updating");

    let upd: GroupedLightUpdate = serde_json::from_value(put)?;

    lock.backend_request(BackendRequest::GroupedLightUpdate(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(|st| get_resource(st, Path(RType::GroupedLight))))
        .route("/{id}", put(put_grouped_light))
}
