use std::collections::BTreeSet;

use serde_json::Value;

use bifrost_api::backend::BackendRequest;
use hue::api::{RType, ResourceLink, Zone, ZoneNew, ZoneUpdate};

use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_zone(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let mut lock = state.res.lock().await;
    lock.get::<Zone>(&rlink)?;

    let mut upd: ZoneUpdate = serde_json::from_value(put)?;

    if let Some(metadata) = upd.metadata.take() {
        lock.update(&rlink.rid, |zone: &mut Zone| {
            zone.metadata += &metadata;
        })?;
    }

    lock.backend_request(BackendRequest::ZoneUpdate(rlink, upd))?;

    drop(lock);

    V2Reply::ok(rlink)
}

pub async fn post_zone(state: &AppState, post: Value) -> ApiV2Result {
    let lock = state.res.lock().await;

    let new: ZoneNew = serde_json::from_value(post)?;
    let zone = Zone {
        children: new.children,
        metadata: new.metadata,
        services: BTreeSet::new(),
    };

    let group_id = lock.get_next_group_id()?;
    let rlink = RType::Zone.deterministic(group_id);
    lock.backend_request(BackendRequest::ZoneCreate(rlink, group_id, zone))?;

    drop(lock);

    V2Reply::ok(rlink)
}
