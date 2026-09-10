use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value;

use bifrost_api::backend::{BackendRequest, request_reply_channel};
use hue::api::{BridgeHome, Device, RType, Resource, ResourceLink, Room, RoomNew, RoomUpdate};
use tokio::time::timeout;

use crate::error::ApiError;
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
    let mut lock = state.res.lock().await;

    let room_new: RoomNew = serde_json::from_value(post)?;
    let backend = room_new.children.iter().find_map(|link| {
        let device = lock.get::<Device>(link).ok()?;
        let light_link = device.light_service()?;
        lock.aux_get(light_link)
            .map_or(None, |aux| aux.backend.clone())
    });

    let room_link = if let Some(backend) = backend {
        let (tx, rx) = request_reply_channel::<ResourceLink>();
        lock.backend_request(BackendRequest::RoomCreate {
            backend,
            room_new,
            link_reply: tx,
        })?;

        drop(lock);

        let Ok(Ok(room_link)) = timeout(Duration::from_millis(500), rx).await else {
            return Err(ApiError::BackendRequestTimeout);
        };
        room_link
    } else {
        let link = RType::Room.random();
        lock.add(
            &link,
            Resource::Room(Room {
                children: room_new.children,
                metadata: room_new.metadata,
                services: BTreeSet::new(),
            }),
        )?;
        for id in &lock.get_resource_ids_by_type(RType::BridgeHome) {
            lock.update(id, |bh: &mut BridgeHome| {
                bh.children.insert(link);
            })?;
        }
        drop(lock);
        link
    };

    V2Reply::ok(room_link)
}
