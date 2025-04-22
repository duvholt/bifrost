use serde_json::Value;
use uuid::Uuid;

use hue::api::{Light, LightUpdate, RType};

use crate::backend::BackendRequest;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_light(state: &AppState, id: Uuid, put: Value) -> ApiV2Result {
    let rlink = RType::Light.link_to(id);
    let lock = state.res.lock().await;

    let _ = lock.get::<Light>(&rlink)?;

    let upd: LightUpdate = serde_json::from_value(put)?;

    lock.backend_request(BackendRequest::LightUpdate(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}
