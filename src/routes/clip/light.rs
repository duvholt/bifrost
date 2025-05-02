use serde_json::Value;

use bifrost_api::backend::BackendRequest;
use hue::api::{Light, LightUpdate, ResourceLink};

use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_light(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let lock = state.res.lock().await;

    let _ = lock.get::<Light>(&rlink)?;

    let upd: LightUpdate = serde_json::from_value(put)?;

    lock.backend_request(BackendRequest::LightUpdate(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}
