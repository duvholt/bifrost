use std::collections::{BTreeSet, HashSet};
use std::time::Duration;

use serde_json::Value;

use bifrost_api::backend::{BackendRequest, request_reply_channel};
use hue::api::{RType, Resource, ResourceLink, Zone, ZoneNew, ZoneUpdate};
use tokio::time::timeout;

use crate::error::ApiError;
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
    let mut lock = state.res.lock().await;

    let zone_new: ZoneNew = serde_json::from_value(post)?;
    let backends: HashSet<_> = zone_new
        .children
        .iter()
        .filter_map(|light_link| {
            lock.aux_get(light_link)
                .map_or(None, |aux| aux.backend.clone())
        })
        .collect();

    if backends.len() > 1 {
        log::error!(
            "Tried adding children from different backends into the same zone {backends:?}"
        );
        return Err(ApiError::MixedBackendChildren);
    }

    let zone_link = if let Some(backend) = backends.into_iter().next() {
        let (tx, rx) = request_reply_channel::<ResourceLink>();
        lock.backend_request(BackendRequest::ZoneCreate {
            backend,
            zone_new,
            link_reply: tx,
        })?;

        drop(lock);

        let Ok(Ok(zone_link)) = timeout(Duration::from_millis(500), rx).await else {
            return Err(ApiError::BackendRequestTimeout);
        };

        zone_link
    } else {
        let link = RType::Zone.random();
        lock.add(
            &link,
            Resource::Zone(Zone {
                children: zone_new.children,
                metadata: zone_new.metadata,
                services: BTreeSet::new(),
            }),
        )?;
        drop(lock);
        link
    };

    V2Reply::ok(zone_link)
}
