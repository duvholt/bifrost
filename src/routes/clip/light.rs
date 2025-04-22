use serde_json::Value;
use uuid::Uuid;

use hue::api::{Light, LightUpdate, RType};

use crate::backend::BackendRequest;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_light(state: &AppState, id: Uuid, put: Value) -> ApiV2Result {
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
