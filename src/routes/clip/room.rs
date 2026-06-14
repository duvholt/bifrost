use std::collections::BTreeSet;

use serde_json::Value;

use bifrost_api::backend::BackendRequest;
use hue::api::{RType, ResourceLink, Room, RoomNew, RoomUpdate};

use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_room(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let mut lock = state.res.lock().await;
    lock.get::<Room>(&rlink)?;

    let mut upd: RoomUpdate = serde_json::from_value(put)?;

    if let Some(metadata) = upd.metadata.take() {
        lock.update(&rlink.rid, |room: &mut Room| {
            room.metadata += &metadata;
        })?;
    }

    lock.backend_request(BackendRequest::RoomUpdate(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}

pub async fn post_room(state: &AppState, post: Value) -> ApiV2Result {
    let lock = state.res.lock().await;

    let new: RoomNew = serde_json::from_value(post)?;
    let room = Room {
        children: new.children,
        metadata: new.metadata,
        services: BTreeSet::new(),
    };
    let rlink = RType::Room.deterministic(&room.metadata.name);
    lock.backend_request(BackendRequest::RoomCreate(rlink, room))?;

    drop(lock);

    V2Reply::ok(rlink)
}
