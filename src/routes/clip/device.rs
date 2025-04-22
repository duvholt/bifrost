use serde_json::Value;
use uuid::Uuid;

use hue::api::{Device, DeviceUpdate, RType};

use crate::routes::clip::ApiV2Result;
use crate::routes::V2Reply;
use crate::server::appstate::AppState;

pub async fn put_device(state: &AppState, id: Uuid, put: Value) -> ApiV2Result {
    let rlink = RType::Device.link_to(id);

    let upd: DeviceUpdate = serde_json::from_value(put)?;

    state
        .res
        .lock()
        .await
        .update::<Device>(&id, |obj| *obj += upd)?;

    V2Reply::ok(rlink)
}
