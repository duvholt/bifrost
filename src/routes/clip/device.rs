use serde_json::Value;

use hue::api::{Device, DeviceUpdate, ResourceLink};

use crate::routes::V2Reply;
use crate::routes::clip::ApiV2Result;
use crate::server::appstate::AppState;

pub async fn put_device(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let upd: DeviceUpdate = serde_json::from_value(put)?;

    let mut lock = state.res.lock().await;
    lock.update::<Device>(&rlink.rid, |obj| *obj += upd)?;
    drop(lock);

    V2Reply::ok(rlink)
}
