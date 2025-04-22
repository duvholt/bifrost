use axum::extract::{Path, State};

use serde_json::Value;
use uuid::Uuid;

use hue::api::{Device, DeviceUpdate, RType};

use crate::routes::clip::ApiV2Result;
use crate::routes::extractor::Json;
use crate::routes::V2Reply;
use crate::server::appstate::AppState;

pub async fn put_device(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(put): Json<Value>,
) -> ApiV2Result {
    log::info!("PUT device/{id}");
    log::debug!("json data\n{}", serde_json::to_string_pretty(&put)?);

    let rlink = RType::Device.link_to(id);

    let upd: DeviceUpdate = serde_json::from_value(put)?;

    state
        .res
        .lock()
        .await
        .update::<Device>(&id, |obj| *obj += upd)?;

    V2Reply::ok(rlink)
}
