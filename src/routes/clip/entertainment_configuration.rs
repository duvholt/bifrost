use bifrost_api::backend::BackendRequest;
use hue::devicedb::gradient_product_data;
use hue::error::HueError;
use serde_json::Value;
use uuid::{Uuid, uuid};

use hue::api::{
    Bridge, Device, Entertainment, EntertainmentConfiguration, EntertainmentConfigurationAction,
    EntertainmentConfigurationChannels, EntertainmentConfigurationLocations,
    EntertainmentConfigurationNew, EntertainmentConfigurationServiceLocations,
    EntertainmentConfigurationStatus, EntertainmentConfigurationStreamMembers,
    EntertainmentConfigurationStreamProxy, EntertainmentConfigurationStreamProxyMode,
    EntertainmentConfigurationStreamProxyUpdate, EntertainmentConfigurationUpdate, Light,
    LightMode, RType, Resource, ResourceLink,
};

use crate::error::ApiResult;
use crate::resource::Resources;
use crate::routes::auth::STANDARD_APPLICATION_ID;
use crate::routes::clip::{ApiV2Result, V2Reply};
use crate::server::appstate::AppState;

pub async fn post_resource(state: &AppState, req: Value) -> ApiV2Result {
    let new: EntertainmentConfigurationNew = serde_json::from_value(req)?;

    let mut lock = state.res.lock().await;

    let locations = EntertainmentConfigurationLocations {
        service_locations: new
            .locations
            .service_locations
            .into_iter()
            .map(Into::into)
            .collect(),
    };

    let channels = make_channels(&lock, &locations.service_locations)?;
    let light_services = make_services(&lock, &locations.service_locations)?;

    let auto_node = find_bridge_entertainment(&lock)?;

    let obj = Resource::EntertainmentConfiguration(EntertainmentConfiguration {
        name: new.metadata.name.clone(),
        configuration_type: new.configuration_type,
        metadata: new.metadata,
        status: EntertainmentConfigurationStatus::Inactive,
        stream_proxy: new.stream_proxy.map_or_else(
            || EntertainmentConfigurationStreamProxy {
                mode: EntertainmentConfigurationStreamProxyMode::Auto,
                node: auto_node,
            },
            |sp| match sp {
                EntertainmentConfigurationStreamProxyUpdate::Auto => {
                    EntertainmentConfigurationStreamProxy {
                        mode: EntertainmentConfigurationStreamProxyMode::Auto,
                        node: auto_node,
                    }
                }
                EntertainmentConfigurationStreamProxyUpdate::Manual { node } => {
                    EntertainmentConfigurationStreamProxy {
                        mode: EntertainmentConfigurationStreamProxyMode::Manual,
                        node,
                    }
                }
            },
        ),
        channels,
        locations,
        light_services,
        active_streamer: None,
    });

    let rlink = ResourceLink::new(Uuid::new_v4(), obj.rtype());
    lock.add(&rlink, obj)?;
    drop(lock);

    V2Reply::ok(rlink)
}

/// The services of an area, in the order of their channels
fn service_order(channels: &[EntertainmentConfigurationChannels]) -> Vec<ResourceLink> {
    let mut channels: Vec<_> = channels.iter().collect();
    channels.sort_by_key(|channel| channel.channel_id);

    let mut order = vec![];
    for member in channels.iter().flat_map(|channel| &channel.members) {
        if !order.contains(&member.service) {
            order.push(member.service);
        }
    }

    order
}

/// Sort `items` in the order given by `current`
///
/// Items whose key is not in `current` go last, in their original order.
fn keep_order<T>(
    mut items: Vec<T>,
    current: &[ResourceLink],
    key: impl Fn(&T) -> ResourceLink,
) -> Vec<T> {
    items.sort_by_key(|item| {
        let key = key(item);
        current
            .iter()
            .position(|link| *link == key)
            .unwrap_or(usize::MAX)
    });

    items
}

fn make_channels(
    lock: &Resources,
    locations: &[EntertainmentConfigurationServiceLocations],
) -> ApiResult<Vec<EntertainmentConfigurationChannels>> {
    let mut channels: Vec<EntertainmentConfigurationChannels> = vec![];

    let mut channel_id = 0;
    for location in locations {
        let Some(pos) = location.positions.first() else {
            continue;
        };
        let ent: &Entertainment = lock.get(&location.service)?;

        let device = lock.get::<Device>(&ent.owner)?;

        let gradient = gradient_product_data(&device.product_data.model_id);

        let positions = gradient.entertainment_channel_positions;
        if let Some(segs) = &ent.segments {
            if segs.segments.len() > 1 {
                for index in 0..segs.segments.len() {
                    channels.push(EntertainmentConfigurationChannels {
                        channel_id,
                        position: positions[index % positions.len()].clone(),
                        members: vec![EntertainmentConfigurationStreamMembers {
                            service: location.service,
                            index: u16::try_from(index)?,
                        }],
                    });
                    channel_id += 1;
                }
                continue;
            }
        }

        channels.push(EntertainmentConfigurationChannels {
            channel_id,
            position: pos.clone(),
            members: vec![EntertainmentConfigurationStreamMembers {
                service: location.service,
                index: 0,
            }],
        });
        channel_id += 1;
    }

    Ok(channels)
}

fn make_services(
    lock: &Resources,
    locations: &[EntertainmentConfigurationServiceLocations],
) -> ApiResult<Vec<ResourceLink>> {
    let mut res = vec![];

    for location in locations {
        let ent: &Entertainment = lock.get(&location.service)?;
        if let Some(ren) = ent.renderer_reference {
            res.push(ren);
        }
    }

    Ok(res)
}

fn find_bridge_entertainment(lock: &Resources) -> ApiResult<ResourceLink> {
    let bridge_id = lock.get_resource_ids_by_type(RType::Bridge)[0];

    let bridge: &Bridge = lock.get_id(bridge_id)?;

    let bridge_dev: &Device = lock.get_id(bridge.owner.rid)?;

    let bridge_ent = bridge_dev
        .services
        .iter()
        .find(|obj| obj.rtype == RType::Entertainment)
        .copied()
        .ok_or(HueError::NotFound(bridge_id))?;

    Ok(bridge_ent)
}

#[must_use]
pub fn active_area(res: &Resources) -> Option<Uuid> {
    res.get_resource_ids_by_type(RType::EntertainmentConfiguration)
        .into_iter()
        .find(|id| {
            res.get_id::<EntertainmentConfiguration>(*id)
                .is_ok_and(|ec| ec.status == EntertainmentConfigurationStatus::Active)
        })
}

pub async fn put_resource_id(state: &AppState, rlink: ResourceLink, put: Value) -> ApiV2Result {
    let upd: EntertainmentConfigurationUpdate = serde_json::from_value(put)?;

    let mut lock = state.res.lock().await;

    // A real bridge streams one area at a time. It accepts a start for another
    // area while one is streaming, or a stop for an area that isn't, and
    // ignores both
    let streaming = active_area(&lock);
    let action = upd.action.clone().filter(|action| match action {
        EntertainmentConfigurationAction::Start => streaming.is_none_or(|id| id == rlink.rid),
        EntertainmentConfigurationAction::Stop => streaming == Some(rlink.rid),
    });

    let mut locations = None;
    let mut channels = vec![];
    let mut light_services = vec![];

    if let Some(locs) = &upd.locations {
        // The channels are numbered in the order of the services, but the
        // Hue app does not send them in a stable order. Keep the numbering of
        // the lights that are already in the area, so that clients that
        // refer to the channels by number (e.g. a layout in HyperHDR) are not
        // broken when the area is edited. New lights are added at the end.
        let current = service_order(&lock.get::<EntertainmentConfiguration>(&rlink)?.channels);

        let service_locations: Vec<EntertainmentConfigurationServiceLocations> = locs
            .service_locations
            .clone()
            .into_iter()
            .map(Into::into)
            .collect();

        let newlocs = EntertainmentConfigurationLocations {
            service_locations: keep_order(service_locations, &current, |loc| loc.service),
        };
        channels = make_channels(&lock, &newlocs.service_locations)?;
        light_services = make_services(&lock, &newlocs.service_locations)?;
        locations = Some(newlocs);
    }

    if let Some(action) = &action {
        let rids: Vec<_> = lock
            .get::<EntertainmentConfiguration>(&rlink)?
            .light_services
            .iter()
            .map(|s| s.rid)
            .collect();

        let mode = match action {
            EntertainmentConfigurationAction::Start => LightMode::Streaming,
            EntertainmentConfigurationAction::Stop => LightMode::Normal,
        };

        for rid in rids {
            lock.update::<Light>(&rid, |light| {
                light.mode = mode;
            })?;
        }
    }

    let bridge_ent = find_bridge_entertainment(&lock)?;

    lock.update::<EntertainmentConfiguration>(&rlink.rid, |ec| {
        if let Some(_locations) = &upd.locations {
            ec.locations = locations.unwrap();
            ec.channels = channels;
            ec.light_services = light_services;
        }

        if let Some(proxy) = &upd.stream_proxy {
            match proxy {
                EntertainmentConfigurationStreamProxyUpdate::Auto => {
                    ec.stream_proxy.mode = EntertainmentConfigurationStreamProxyMode::Auto;
                    ec.stream_proxy.node = bridge_ent;
                }
                EntertainmentConfigurationStreamProxyUpdate::Manual { node } => {
                    ec.stream_proxy.mode = EntertainmentConfigurationStreamProxyMode::Manual;
                    ec.stream_proxy.node = *node;
                }
            }
        }

        if let Some(ctype) = &upd.configuration_type {
            ec.configuration_type = ctype.clone();
        }

        if let Some(md) = &upd.metadata {
            ec.metadata = md.clone();
        }

        if let Some(action) = &action {
            match action {
                EntertainmentConfigurationAction::Start => {
                    ec.active_streamer =
                        Some(RType::AuthV1.link_to(uuid!(STANDARD_APPLICATION_ID)));
                    ec.status = EntertainmentConfigurationStatus::Active;
                }
                EntertainmentConfigurationAction::Stop => {
                    ec.active_streamer = None;
                    ec.status = EntertainmentConfigurationStatus::Inactive;
                }
            }
        }
    })?;

    match action {
        Some(EntertainmentConfigurationAction::Start) => {
            lock.backend_request(BackendRequest::EntertainmentStart(rlink.rid))?;
        }
        Some(EntertainmentConfigurationAction::Stop) => {
            lock.backend_request(BackendRequest::EntertainmentStop())?;
        }
        None => {}
    }

    drop(lock);

    V2Reply::ok(rlink)
}

#[cfg(test)]
mod tests {
    use hue::api::{
        EntertainmentConfigurationChannels, EntertainmentConfigurationStreamMembers, Position,
        RType, ResourceLink,
    };
    use uuid::Uuid;

    use super::{keep_order, service_order};

    fn link(n: u128) -> ResourceLink {
        RType::Entertainment.link_to(Uuid::from_u128(n))
    }

    fn channel(
        channel_id: u32,
        service: ResourceLink,
        index: u16,
    ) -> EntertainmentConfigurationChannels {
        EntertainmentConfigurationChannels {
            channel_id,
            position: Position::default(),
            members: vec![EntertainmentConfigurationStreamMembers { service, index }],
        }
    }

    #[test]
    fn service_order_follows_the_channel_numbers() {
        let (a, b) = (link(1), link(2));

        // channels listed out of order, the services have several channels
        let channels = [
            channel(2, b, 0),
            channel(0, a, 0),
            channel(3, b, 1),
            channel(1, a, 1),
        ];

        assert_eq!(service_order(&channels), [a, b]);
    }

    #[test]
    fn service_order_of_an_empty_area_is_empty() {
        assert_eq!(service_order(&[]), Vec::<ResourceLink>::new());
    }

    #[test]
    fn existing_services_keep_their_place() {
        let (a, b) = (link(1), link(2));

        // the app sends the services in a different order than before
        assert_eq!(keep_order(vec![b, a], &[a, b], |l| *l), [a, b]);
        assert_eq!(keep_order(vec![a, b], &[a, b], |l| *l), [a, b]);
    }

    #[test]
    fn new_services_are_added_at_the_end_in_the_order_they_came() {
        let (a, b, c, d) = (link(1), link(2), link(3), link(4));

        // c and d are new, in the order d, c
        assert_eq!(keep_order(vec![d, b, c, a], &[a, b], |l| *l), [a, b, d, c]);
    }

    #[test]
    fn removed_services_leave_the_others_in_order() {
        let (a, b, c) = (link(1), link(2), link(3));

        assert_eq!(keep_order(vec![c, a], &[a, b, c], |l| *l), [a, c]);
    }

    #[test]
    fn a_new_area_keeps_the_order_it_was_given() {
        let (a, b, c) = (link(1), link(2), link(3));

        assert_eq!(keep_order(vec![c, a, b], &[], |l| *l), [c, a, b]);
    }
}
