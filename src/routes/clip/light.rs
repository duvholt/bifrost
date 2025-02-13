use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};
use serde_json::Value;
use uuid::Uuid;

use crate::hue::api::{Light, LightUpdate, RType, V2Reply};
use crate::routes::clip::ApiV2Result;
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;
use crate::z2m::request::ClientRequest;
use crate::z2m::update::DeviceUpdate;

async fn put_light(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(put): Json<Value>,
) -> ApiV2Result {
    log::info!("PUT light/{id}");
    log::debug!("json data\n{}", serde_json::to_string_pretty(&put)?);

    let rlink = RType::Light.link_to(id);
    let mut lock = state.res.lock().await;

    let _ = lock.get::<Light>(&rlink)?;

    let upd: LightUpdate = serde_json::from_value(put)?;

    // We cannot recover .mode from backend updates, since these only contain
    // the gradient colors. So we have no choice, but to update the mode
    // here. Otherwise, the information would be lost.
    if let Some(mode) = upd.gradient.as_ref().and_then(|gr| gr.mode) {
        lock.update::<Light>(&rlink.rid, |light| {
            if let Some(gr) = &mut light.gradient {
                gr.mode = mode;
            }
        })?;
    }

    let payload = DeviceUpdate::default()
        .with_state(upd.on.map(|on| on.on))
        .with_brightness(upd.dimming.map(|dim| dim.brightness / 100.0 * 254.0))
        .with_color_temp(upd.color_temperature.map(|ct| ct.mirek))
        .with_color_xy(upd.color.map(|col| col.xy))
        .with_gradient(upd.gradient);

    lock.z2m_request(ClientRequest::light_update(rlink, payload))?;

    drop(lock);

    V2Reply::ok(rlink)
}

async fn get_light(State(state): State<AppState>, Path(id): Path<Uuid>) -> ApiV2Result {
    V2Reply::ok(state.res.lock().await.get_resource(RType::Light, &id)?)
}

pub fn router() -> Router<AppState> {
    Router::new().route("/:id", get(get_light).put(put_light))
}
