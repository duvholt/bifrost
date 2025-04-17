use serde_json::Value;

use hue::api::{ResourceLink, Room, RoomUpdate};

use crate::backend::BackendRequest;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_room(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let mut lock = state.res.lock().await;
    lock.get::<Room>(&rlink)?;

    let mut upd: RoomUpdate = serde_json::from_value(put)?;

    if let Some(metadata) = upd.metadata.take() {
        lock.update(&rlink.rid, |room: &mut Room| {
            room.metadata += metadata;
        })?;
    }

    lock.backend_request(BackendRequest::RoomUpdate(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}
