use serde_json::Value;
use uuid::Uuid;

use hue::api::{GroupedLight, GroupedLightUpdate, RType};

use crate::backend::BackendRequest;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_grouped_light(state: &AppState, id: Uuid, put: Value) -> ApiV2Result {
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
