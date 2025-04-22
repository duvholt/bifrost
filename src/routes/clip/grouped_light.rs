use serde_json::Value;

use hue::api::{GroupedLight, GroupedLightUpdate, ResourceLink};

use crate::backend::BackendRequest;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_grouped_light(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let upd: GroupedLightUpdate = serde_json::from_value(put)?;

    let lock = state.res.lock().await;
    lock.get::<GroupedLight>(&rlink)?;
    lock.backend_request(BackendRequest::GroupedLightUpdate(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}
