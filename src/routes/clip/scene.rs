use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::Router;
use serde_json::Value;
use uuid::Uuid;

use hue::api::{RType, Resource, Scene, SceneUpdate};

use crate::backend::BackendRequest;
use crate::error::{ApiError, ApiResult};
use crate::routes::clip::generic::get_resource;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

async fn post_scene(
    State(state): State<AppState>,
    Json(req): Json<Value>,
) -> ApiResult<impl IntoResponse> {
    log::info!("POST: scene {}", serde_json::to_string(&req)?);

    let scene: Scene = serde_json::from_value(req)?;

    let lock = state.res.lock().await;

    let sid = lock.get_next_scene_id(&scene.group)?;

    let link_scene = RType::Scene.deterministic((scene.group.rid, sid));

    lock.backend_request(BackendRequest::SceneCreate(link_scene, sid, scene))?;

    drop(lock);

    V2Reply::ok(link_scene)
}

async fn put_scene(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(put): Json<Value>,
) -> ApiV2Result {
    log::info!("PUT scene/{id}");
    log::debug!("json data\n{}", serde_json::to_string_pretty(&put)?);

    let rlink = RType::Scene.link_to(id);
    let mut lock = state.res.lock().await;

    log::info!("PUT scene/{id}: updating");

    let upd: SceneUpdate = serde_json::from_value(put)?;

    if let Some(md) = &upd.metadata {
        lock.update::<Scene>(&id, |scn| scn.metadata += md.clone())?;
    }

    let _scene = lock.get::<Scene>(&rlink)?;

    lock.backend_request(BackendRequest::SceneUpdate(rlink, upd))?;
    drop(lock);

    V2Reply::ok(rlink)
}

async fn delete_scene(State(state): State<AppState>, Path(id): Path<Uuid>) -> ApiV2Result {
    log::info!("DELETE scene/{id}");
    let link = RType::Scene.link_to(id);

    let lock = state.res.lock().await;
    let res = lock.get_resource(RType::Scene, &id)?;

    match res.obj {
        Resource::Scene(_) => {
            lock.backend_request(BackendRequest::Delete(link))?;

            drop(lock);

            V2Reply::ok(link)
        }
        _ => Err(ApiError::DeleteDenied(id))?,
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(|state| get_resource(state, Path(RType::Scene))))
        .route("/", post(post_scene))
        .route("/{id}", put(put_scene))
        .route("/{id}", delete(delete_scene))
}
