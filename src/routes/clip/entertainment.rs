use axum::extract::{Path, State};
use axum::routing::get;
use axum::Router;
use uuid::Uuid;

use crate::hue::api::RType;
use crate::routes::clip::generic;
use crate::routes::clip::ApiV2Result;
use crate::server::appstate::AppState;

pub async fn get_resource(state: State<AppState>) -> ApiV2Result {
    generic::get_resource(state, Path(RType::Entertainment)).await
}

async fn get_resource_id(state: State<AppState>, Path(id): Path<Uuid>) -> ApiV2Result {
    generic::get_resource_id(state, Path((RType::Entertainment, id))).await
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_resource))
        .route("/{id}", get(get_resource_id))
}
