use std::borrow::BorrowMut;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::Router;
use serde_json::Value;
use uuid::{uuid, Uuid};

use crate::error::{ApiError, ApiResult};
use crate::hue::api::{
    Bridge, Device, Entertainment, EntertainmentConfiguration, EntertainmentConfigurationAction,
    EntertainmentConfigurationChannels, EntertainmentConfigurationLocations,
    EntertainmentConfigurationNew, EntertainmentConfigurationPosition,
    EntertainmentConfigurationServiceLocations, EntertainmentConfigurationStatus,
    EntertainmentConfigurationStreamMembers, EntertainmentConfigurationStreamProxy,
    EntertainmentConfigurationStreamProxyMode, EntertainmentConfigurationStreamProxyUpdate,
    EntertainmentConfigurationUpdate, Light, LightMode, RType, Resource, ResourceLink, V2Reply,
};
use crate::resource::Resources;
use crate::routes::auth::STANDARD_APPLICATION_ID;
use crate::routes::clip::{generic, ApiV2Result};
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

pub async fn get_resource(state: State<AppState>) -> ApiV2Result {
    super::generic::get_resource(state, Path(RType::EntertainmentConfiguration)).await
}

async fn post_resource(State(state): State<AppState>, Json(req): Json<Value>) -> impl IntoResponse {
    log::info!(
        "POST: entertainment_configuration2 {}",
        serde_json::to_string(&req)?
    );

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

    let channels = make_channels(locations.service_locations[0].service);
    let light_services = make_services(lock.borrow_mut(), &locations.service_locations)?;

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

async fn get_resource_id(state: State<AppState>, Path(id): Path<Uuid>) -> ApiV2Result {
    generic::get_resource_id(state, Path((RType::EntertainmentConfiguration, id))).await
}

fn make_channels(service: ResourceLink) -> Vec<EntertainmentConfigurationChannels> {
    // FIXME: These are hard-coded values fitting for an LCX005 gradient light chain
    let positions = [
        EntertainmentConfigurationPosition {
            x: -0.4,
            y: 0.8,
            z: -0.4,
        },
        EntertainmentConfigurationPosition {
            x: -0.4,
            y: 0.8,
            z: 0.4,
        },
        EntertainmentConfigurationPosition {
            x: -0.21999,
            y: 0.8,
            z: 0.4,
        },
        EntertainmentConfigurationPosition {
            x: 0.0,
            y: 0.8,
            z: 0.4,
        },
        EntertainmentConfigurationPosition {
            x: 0.21999,
            y: 0.8,
            z: 0.4,
        },
        EntertainmentConfigurationPosition {
            x: 0.4,
            y: 0.8,
            z: 0.4,
        },
        EntertainmentConfigurationPosition {
            x: 0.4,
            y: 0.8,
            z: -0.4,
        },
    ];

    let mut channels: Vec<EntertainmentConfigurationChannels> = vec![];

    for x in 0..7 {
        channels.push(EntertainmentConfigurationChannels {
            channel_id: x,
            position: positions[x as usize].clone(),
            members: vec![EntertainmentConfigurationStreamMembers { service, index: x }],
        });
    }

    channels
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
        .ok_or(ApiError::NotFound(bridge_id))?;

    Ok(bridge_ent)
}

async fn put_resource_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(put): Json<Value>,
) -> ApiV2Result {
    let rtype = RType::EntertainmentConfiguration;

    log::info!("PUT {rtype:?}/{id}");
    log::debug!("json data\n{}", serde_json::to_string_pretty(&put)?);

    let upd: EntertainmentConfigurationUpdate = serde_json::from_value(put)?;

    let mut lock = state.res.lock().await;

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
        channels = make_channels(locs.service_locations[0].service);
        light_services = make_services(lock.borrow_mut(), &newlocs.service_locations)?;
        locations = Some(newlocs);
    }

    if let Some(action) = &upd.action {
        let ent: &EntertainmentConfiguration =
            lock.get(&RType::EntertainmentConfiguration.link_to(id))?;
        let svc = ent.light_services.clone();

        lock.update::<Light>(&svc[0].rid, |light| {
            light.mode = match action {
                EntertainmentConfigurationAction::Start => LightMode::Streaming,
                EntertainmentConfigurationAction::Stop => LightMode::Normal,
            }
        })?;
    }

    let bridge_ent = find_bridge_entertainment(&lock)?;

    lock.update::<EntertainmentConfiguration>(&id, |ec| {
        if let Some(_locations) = upd.locations {
            ec.locations = locations.unwrap();
            ec.channels = channels;
            ec.light_services = light_services;
        }
        if let Some(proxy) = upd.stream_proxy {
            match proxy {
                EntertainmentConfigurationStreamProxyUpdate::Auto => {
                    ec.stream_proxy.mode = EntertainmentConfigurationStreamProxyMode::Auto;
                    ec.stream_proxy.node = bridge_ent;
                }
                EntertainmentConfigurationStreamProxyUpdate::Manual { node } => {
                    ec.stream_proxy.mode = EntertainmentConfigurationStreamProxyMode::Manual;
                    ec.stream_proxy.node = node;
                }
            }
        }

        if let Some(ctype) = upd.configuration_type {
            ec.configuration_type = ctype;
        }

        if let Some(md) = upd.metadata {
            ec.metadata = md;
        }

        if let Some(action) = upd.action {
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
            };
        }
    })?;

    drop(lock);

    let rlink = ResourceLink::new(id, rtype);

    V2Reply::ok(rlink)
}

async fn delete_resource_id(
    State(state): State<AppState>,
    Path((rtype, id)): Path<(RType, Uuid)>,
) -> ApiV2Result {
    log::info!("DELETE {rtype:?}/{id}");

    state.res.lock().await.get_resource(rtype, &id)?;

    Err(ApiError::DeleteDenied(id))?
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_resource))
        .route("/", post(post_resource))
        .route("/{id}", get(get_resource_id))
        .route("/{id}", put(put_resource_id))
        .route("/{id}", delete(delete_resource_id))
}
