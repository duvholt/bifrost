use bifrost_api::backend::BackendRequest;
use serde_json::Value;

use hue::api::{Device, DeviceUpdate, LightUpdate, ResourceLink};

use crate::routes::V2Reply;
use crate::routes::clip::ApiV2Result;
use crate::server::appstate::AppState;

pub async fn put_device(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let upd: DeviceUpdate = serde_json::from_value(put)?;

    let mut lock = state.res.lock().await;

    if let Some(identify) = &upd.identify {
        let dev: &Device = lock.get(&rlink)?;
        if let Some(light) = dev.light_service() {
            let upd = LightUpdate::new().with_identify(Some(*identify));
            lock.backend_request(BackendRequest::LightUpdate(*light, upd))?;
        }
    }

    lock.update::<Device>(&rlink.rid, |obj| *obj += upd)?;
    drop(lock);

    V2Reply::ok(rlink)
}
