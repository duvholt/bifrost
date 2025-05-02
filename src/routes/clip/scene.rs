use serde_json::Value;

use bifrost_api::backend::BackendRequest;
use hue::api::{RType, ResourceLink, Scene, SceneUpdate};

use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn post_scene(state: &AppState, req: Value) -> ApiV2Result {
    let scene: Scene = serde_json::from_value(req)?;

    let lock = state.res.lock().await;

    let sid = lock.get_next_scene_id(&scene.group)?;

    let link_scene = RType::Scene.deterministic((scene.group.rid, sid));

    lock.backend_request(BackendRequest::SceneCreate(link_scene, sid, scene))?;

    drop(lock);

    V2Reply::ok(link_scene)
}

pub async fn put_scene(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let mut lock = state.res.lock().await;

    let upd: SceneUpdate = serde_json::from_value(put)?;

    if let Some(md) = &upd.metadata {
        lock.update::<Scene>(&rlink.rid, |scn| scn.metadata += md.clone())?;
    }

    let _scene = lock.get::<Scene>(&rlink)?;

    lock.backend_request(BackendRequest::SceneUpdate(rlink, upd))?;
    drop(lock);

    V2Reply::ok(rlink)
}

pub async fn delete_scene(state: &AppState, rlink: ResourceLink) -> ApiV2Result {
    let lock = state.res.lock().await;

    let _scene: &Scene = lock.get(&rlink)?;

    lock.backend_request(BackendRequest::Delete(rlink))?;

    drop(lock);

    V2Reply::ok(rlink)
}
