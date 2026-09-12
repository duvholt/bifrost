use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::Value;

use bifrost_api::backend::{BackendRequest, request_reply_channel};
use hue::api::{RType, Resource, ResourceLink, Zone, ZoneNew, ZoneUpdate};
use tokio::time::timeout;

use crate::error::{ApiError, ApiResult};
use crate::resource::Resources;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn put_zone(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let mut lock = state.res.lock().await;
    lock.get::<Zone>(&rlink)?;

    let mut upd: ZoneUpdate = serde_json::from_value(put)?;
    let zone_backend = lock.zone_backend(&rlink)?;

    let children_backend = match &upd.children {
        Some(children) => backend_for_children(&lock, children)?,
        None => None,
    };

    if let (Some(zone_backend), Some(children_backend)) = (&zone_backend, &children_backend)
        && zone_backend != children_backend
    {
        return Err(ApiError::MixedBackendChildren);
    }

    if let Some(metadata) = upd.metadata.take() {
        lock.update(&rlink.rid, |zone: &mut Zone| {
            zone.metadata += &metadata;
        })?;
    }

    if zone_backend.is_none()
        && let Some(backend) = children_backend
        && let Some(children) = upd.children.take()
    {
        let zone = lock.get::<Zone>(&rlink)?;
        let zone_new = ZoneNew {
            children,
            metadata: zone.metadata.clone(),
        };
        lock.backend_request(BackendRequest::ZoneCreate {
            backend,
            zone_new,
            existing_link: Some(rlink),
            link_reply: None,
        })?;
    } else {
        lock.backend_request(BackendRequest::ZoneUpdate(rlink, upd))?;
    }

    drop(lock);

    V2Reply::ok(rlink)
}

pub async fn post_zone(state: &AppState, post: Value) -> ApiV2Result {
    let mut lock = state.res.lock().await;

    let zone_new: ZoneNew = serde_json::from_value(post)?;
    let backend = backend_for_children(&lock, &zone_new.children)?;

    let zone_link = if let Some(backend) = backend {
        let (tx, rx) = request_reply_channel::<ResourceLink>();
        lock.backend_request(BackendRequest::ZoneCreate {
            backend,
            zone_new,
            existing_link: None,
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

fn backend_for_children(
    res: &Resources,
    children: &BTreeSet<ResourceLink>,
) -> ApiResult<Option<String>> {
    let mut backend = None;
    for link in children {
        let child_backend = res
            .light_backend(link)?
            .ok_or(ApiError::BackendNotFound(link.rid))?;

        match &backend {
            None => backend = Some(child_backend),
            Some(expected) if expected == &child_backend => {}
            Some(expected) => {
                log::error!(
                    "Tried adding children from different backends into the same zone {expected} {child_backend}"
                );
                return Err(ApiError::MixedBackendChildren);
            }
        }
    }
    Ok(backend)
}
