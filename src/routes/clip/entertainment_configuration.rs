use bifrost_api::backend::BackendRequest;
use hue::devicedb::{gradient_product_data, product_data};
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
    LightMode, Position, RType, Resource, ResourceLink,
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

/// Positions of the `count` segments of a light, spread evenly between the
/// first and the last of the `points` given by the Hue app
///
/// Returns `None` if the existing positions should be used instead: when the
/// light has segment positions of its own, or when the app gave less than two
/// points.
fn segment_line(
    points: &[Position],
    count: usize,
    has_own_gradient_data: bool,
) -> Option<Vec<Position>> {
    match (points.first(), points.last()) {
        (Some(start), Some(end)) if points.len() >= 2 && !has_own_gradient_data => {
            Some(Position::spread(start, end, count))
        }
        _ => None,
    }
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
        let has_own_gradient_data =
            product_data(&device.product_data.model_id).is_some_and(|pd| pd.gradient.is_some());

        let positions = gradient.entertainment_channel_positions;
        if let Some(segs) = &ent.segments {
            if segs.segments.len() > 1 {
                // For lights that we have no segment positions for (e.g. the
                // Hue Signe), the Hue app tells us where the first and the
                // last segment are, with a start and an end point. Spread the
                // segments evenly between them. Flipping the light in the app
                // swaps the two points, which reverses the order of the
                // segments.
                let line = segment_line(
                    &location.positions,
                    segs.segments.len(),
                    has_own_gradient_data,
                );

                for index in 0..segs.segments.len() {
                    let position = line.as_ref().map_or_else(
                        || positions[index % positions.len()].clone(),
                        |line| line[index].clone(),
                    );

                    channels.push(EntertainmentConfigurationChannels {
                        channel_id,
                        position,
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
        let newlocs = EntertainmentConfigurationLocations {
            service_locations: locs
                .service_locations
                .clone()
                .into_iter()
                .map(Into::into)
                .collect(),
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
    use hue::api::Position;

    use super::segment_line;

    fn assert_z(line: &[Position], want: &[f64]) {
        assert_eq!(line.len(), want.len());
        for (p, w) in line.iter().zip(want) {
            assert!((p.z - w).abs() < 1e-9, "{} != {w}", p.z);
        }
    }

    #[test]
    fn segments_are_spread_between_the_two_points() {
        let points = [Position::new(0.0, 0.0, -1.0), Position::new(0.0, 0.0, 1.0)];

        let line = segment_line(&points, 5, false).expect("two points spread the segments");

        assert_z(&line, &[-1.0, -0.5, 0.0, 0.5, 1.0]);
    }

    #[test]
    fn flipping_the_light_reverses_the_segments() {
        let points = [Position::new(0.0, 0.0, -1.0), Position::new(0.0, 0.0, 1.0)];
        let flipped = [points[1].clone(), points[0].clone()];

        let forward = segment_line(&points, 7, false).unwrap();
        let backward = segment_line(&flipped, 7, false).unwrap();

        let mut reversed: Vec<f64> = backward.iter().map(|p| p.z).collect();
        reversed.reverse();
        assert_z(&forward, &reversed);
    }

    #[test]
    fn the_first_and_the_last_point_are_used() {
        // more than two points: only the ends count
        let points = [
            Position::new(0.0, 0.0, -1.0),
            Position::new(5.0, 5.0, 5.0),
            Position::new(0.0, 0.0, 1.0),
        ];

        let line = segment_line(&points, 3, false).unwrap();

        assert_z(&line, &[-1.0, 0.0, 1.0]);
    }

    #[test]
    fn lights_with_their_own_segment_data_keep_their_positions() {
        let points = [Position::new(0.0, 0.0, -1.0), Position::new(0.0, 0.0, 1.0)];

        assert!(segment_line(&points, 5, true).is_none());
    }

    #[test]
    fn a_single_point_keeps_the_existing_positions() {
        let points = [Position::new(0.0, 0.0, 0.0)];

        assert!(segment_line(&points, 5, false).is_none());
        assert!(segment_line(&[], 5, false).is_none());
    }
}
