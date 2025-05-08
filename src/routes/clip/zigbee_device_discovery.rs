use serde_json::Value;

use bifrost_api::backend::BackendRequest;
use hue::api::{ResourceLink, ZigbeeDeviceDiscovery, ZigbeeDeviceDiscoveryUpdate};

use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_zigbee_device_discovery(
    state: &AppState,
    rlink: ResourceLink,
    put: Value,
) -> ApiV2Result {
    let lock = state.res.lock().await;
    lock.get::<ZigbeeDeviceDiscovery>(&rlink)?;

    let upd: ZigbeeDeviceDiscoveryUpdate = serde_json::from_value(put)?;

    lock.backend_request(BackendRequest::ZigbeeDeviceDiscovery(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}
