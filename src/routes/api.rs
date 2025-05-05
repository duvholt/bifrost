use std::collections::{BTreeMap, HashMap};

use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use bytes::Bytes;
use chrono::Utc;
use log::{info, warn};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::MutexGuard;
use uuid::Uuid;

use bifrost_api::backend::BackendRequest;
use hue::api::{
    Device, Entertainment, EntertainmentConfiguration, EntertainmentConfigurationMetadata,
    EntertainmentConfigurationNew, EntertainmentConfigurationServiceLocationsNew,
    EntertainmentConfigurationStatus, EntertainmentConfigurationType, GroupedLight,
    GroupedLightUpdate, Light, LightUpdate, Position, RType, Resource, ResourceLink, Room, Scene,
    SceneActive, SceneStatus, SceneUpdate, V1Reply,
};
use hue::error::{HueApiV1Error, HueError, HueResult};
use hue::legacy_api::{
    ApiGroup, ApiGroupAction, ApiGroupActionUpdate, ApiGroupClass, ApiGroupNew, ApiGroupState,
    ApiGroupType, ApiGroupUpdate2, ApiLight, ApiLightStateUpdate, ApiResourceType, ApiScene,
    ApiSceneAppData, ApiSceneType, ApiSceneVersion, ApiSensor, ApiUserConfig, Capabilities,
    HueApiResult, NewUser, NewUserReply,
};

use crate::error::{ApiError, ApiResult};
use crate::resource::Resources;
use crate::routes::auth::STANDARD_CLIENT_KEY;
use crate::routes::extractor::Json;
use crate::routes::{ApiV1Error, ApiV1Result};
use crate::server::appstate::AppState;

async fn get_api_config(State(state): State<AppState>) -> Json<impl Serialize> {
    Json(state.api_short_config().await)
}

async fn post_api(bytes: Bytes) -> ApiV1Result<Json<impl Serialize>> {
    info!("post: {bytes:?}");
    let json: NewUser = serde_json::from_slice(&bytes)?;

    let res = NewUserReply {
        clientkey: if json.generateclientkey {
            Some(STANDARD_CLIENT_KEY.to_hex())
        } else {
            None
        },
        username: Uuid::new_v4().as_simple().to_string(),
    };
    Ok(Json(vec![HueApiResult::Success(res)]))
}

fn get_lights(res: &MutexGuard<Resources>) -> ApiResult<HashMap<String, ApiLight>> {
    let mut lights = HashMap::new();

    for rr in res.get_resources_by_type(RType::Light) {
        let light: Light = rr.obj.try_into()?;
        let dev = res.get::<Device>(&light.owner)?;
        lights.insert(
            res.get_id_v1(rr.id)?,
            ApiLight::from_dev_and_light(&rr.id, dev, &light),
        );
    }

    Ok(lights)
}

fn get_groups(res: &MutexGuard<Resources>, group_0: bool) -> ApiResult<HashMap<String, ApiGroup>> {
    let mut rooms = HashMap::new();

    if group_0 {
        rooms.insert("0".into(), ApiGroup::make_group_0());
    }

    for rr in res.get_resources_by_type(RType::Room) {
        let room: Room = rr.obj.try_into()?;
        let uuid = room
            .services
            .iter()
            .find(|rl| rl.rtype == RType::GroupedLight)
            .ok_or(HueError::NotFound(rr.id))?;

        let glight = res.get::<GroupedLight>(uuid)?;
        let lights: Vec<String> = room
            .children
            .iter()
            .filter_map(|rl| res.get(rl).ok())
            .filter_map(Device::light_service)
            .filter_map(|rl| res.get_id_v1(rl.rid).ok())
            .collect();

        rooms.insert(
            res.get_id_v1(rr.id)?,
            ApiGroup::from_lights_and_room(glight, lights, room),
        );
    }

    for rr in res.get_resources_by_type(RType::EntertainmentConfiguration) {
        let entconf: EntertainmentConfiguration = rr.obj.try_into()?;

        let mut locations = BTreeMap::<String, Vec<f64>>::new();

        for sl in &entconf.locations.service_locations {
            let ent = res.get::<Entertainment>(&sl.service)?;
            let dev = res.get::<Device>(&ent.owner)?;
            let light_link = dev
                .light_service()
                .ok_or(HueError::NotFound(ent.owner.rid))?;

            let idx = res.get_id_v1_index(light_link.rid)?;
            locations.insert(
                idx.to_string(),
                vec![sl.position.x, sl.position.y, sl.position.z],
            );
        }

        let class = match entconf.configuration_type {
            EntertainmentConfigurationType::Screen => ApiGroupClass::TV,
            EntertainmentConfigurationType::Monitor => ApiGroupClass::Computer,
            EntertainmentConfigurationType::Music => ApiGroupClass::Music,
            // FIXME: what does Space3D map to?
            EntertainmentConfigurationType::Space3D | EntertainmentConfigurationType::Other => {
                ApiGroupClass::Other
            }
        };

        rooms.insert(
            res.get_id_v1(rr.id)?,
            ApiGroup {
                name: entconf.metadata.name.clone(),
                lights: locations.keys().cloned().collect(),
                locations: json!(locations),
                action: ApiGroupAction::default(),
                class,
                group_type: ApiGroupType::Entertainment,
                recycle: false,
                sensors: vec![],
                state: ApiGroupState::default(),
                stream: json!({
                    "active": false,
                    "owner": Value::Null,
                    "proxymode": "auto",
                    "proxynode": "/bridge"
                }),
            },
        );
    }

    Ok(rooms)
}

pub fn get_scene(res: &Resources, owner: String, scene: &Scene) -> ApiV1Result<ApiScene> {
    let lights = scene
        .actions
        .iter()
        .map(|sae| res.get_id_v1(sae.target.rid))
        .collect::<HueResult<_>>()?;

    let lightstates = scene
        .actions
        .iter()
        .map(|sae| {
            Ok((
                res.get_id_v1(sae.target.rid)?,
                ApiLightStateUpdate::from(sae.action.clone()),
            ))
        })
        .collect::<ApiV1Result<_>>()?;

    let room_id = res.get_id_v1_index(scene.group.rid)?;

    Ok(ApiScene {
        name: scene.metadata.name.clone(),
        scene_type: ApiSceneType::GroupScene,
        lights,
        lightstates,
        owner,
        recycle: false,
        locked: false,
        /* Some clients (e.g. Hue Essentials) require .appdata */
        appdata: ApiSceneAppData {
            data: Some(format!("xxxxx_r{room_id}")),
            version: Some(1),
        },
        picture: String::new(),
        lastupdated: Utc::now(),
        version: ApiSceneVersion::V2 as u32,
        image: scene.metadata.image.map(|rl| rl.rid),
        group: Some(room_id.to_string()),
    })
}

fn get_scenes(owner: &str, res: &MutexGuard<Resources>) -> ApiV1Result<HashMap<String, ApiScene>> {
    let mut scenes = HashMap::new();

    for rr in res.get_resources_by_type(RType::Scene) {
        let scene = &rr.obj.try_into()?;

        scenes.insert(
            res.get_id_v1(rr.id)?,
            get_scene(res, owner.to_string(), scene)?,
        );
    }

    Ok(scenes)
}

#[allow(clippy::zero_sized_map_values)]
async fn get_api_user(
    state: State<AppState>,
    Path(username): Path<String>,
) -> ApiV1Result<Json<impl Serialize>> {
    let lock = state.res.lock().await;

    Ok(Json(ApiUserConfig {
        config: state.api_config(username.clone()).await?,
        groups: get_groups(&lock, false)?,
        lights: get_lights(&lock)?,
        resourcelinks: HashMap::new(),
        rules: HashMap::new(),
        scenes: get_scenes(&username, &lock)?,
        schedules: HashMap::new(),
        sensors: HashMap::from([(1, ApiSensor::builtin_daylight_sensor())]),
    }))
}

async fn get_api_user_resource(
    State(state): State<AppState>,
    Path((username, artype)): Path<(String, ApiResourceType)>,
) -> ApiV1Result<Json<Value>> {
    let lock = &state.res.lock().await;
    match artype {
        ApiResourceType::Config => Ok(Json(json!(state.api_config(username).await?))),
        ApiResourceType::Lights => Ok(Json(json!(get_lights(lock)?))),
        ApiResourceType::Groups => Ok(Json(json!(get_groups(lock, false)?))),
        ApiResourceType::Scenes => Ok(Json(json!(get_scenes(&username, lock)?))),
        ApiResourceType::Resourcelinks
        | ApiResourceType::Rules
        | ApiResourceType::Schedules
        | ApiResourceType::Sensors => Ok(Json(json!({}))),
        ApiResourceType::Capabilities => Ok(Json(json!(Capabilities::new()))),
    }
}

async fn post_api_user_resource(
    Path((_username, resource)): Path<(String, ApiResourceType)>,
    Json(req): Json<Value>,
) -> ApiV1Result<Json<Value>> {
    warn!("POST v1 user resource unsupported");
    warn!("Request: {req:?}");
    Err(ApiV1Error::V1CreateUnsupported(resource))
}

async fn put_api_user_resource(
    Path((_username, _resource)): Path<(String, String)>,
    Json(req): Json<Value>,
) -> ApiV1Result<Json<impl Serialize>> {
    warn!("PUT v1 user resource {req:?}");
    //Json(format!("user {username} resource {resource}"))
    Ok(Json(vec![HueApiResult::Success(req)]))
}

#[allow(clippy::significant_drop_tightening)]
async fn get_api_user_resource_id(
    State(state): State<AppState>,
    Path((username, resource, id)): Path<(String, ApiResourceType, u32)>,
) -> ApiV1Result<Json<impl Serialize>> {
    log::debug!("GET v1 username={username} resource={resource:?} id={id}");
    let result = match resource {
        ApiResourceType::Lights => {
            let lock = state.res.lock().await;
            let uuid = lock.from_id_v1(id)?;
            let link = ResourceLink::new(uuid, RType::Light);
            let light = lock.get::<Light>(&link)?;
            let dev = lock.get::<Device>(&light.owner)?;

            json!(ApiLight::from_dev_and_light(&uuid, dev, light))
        }
        ApiResourceType::Scenes => {
            let lock = state.res.lock().await;
            let uuid = lock.from_id_v1(id)?;
            let link = ResourceLink::new(uuid, RType::Scene);
            let scene = lock.get::<Scene>(&link)?;

            json!(get_scene(&lock, username, scene)?)
        }
        ApiResourceType::Groups => {
            let lock = state.res.lock().await;
            let groups = get_groups(&lock, true)?;
            let group = groups
                .get(&id.to_string())
                .ok_or(HueError::V1NotFound(id))?;

            json!(group)
        }
        _ => Err(HueError::V1NotFound(id))?,
    };

    Ok(Json(result))
}

#[allow(clippy::significant_drop_tightening, clippy::single_match)]
async fn put_api_user_resource_id(
    State(state): State<AppState>,
    Path((username, artype, id)): Path<(String, ApiResourceType, u32)>,
    Json(req): Json<Value>,
) -> ApiV1Result<Json<Value>> {
    log::debug!("PUT v1 username={username} resource={artype:?} id={id}");
    log::debug!("JSON: {req:?}");
    match artype {
        ApiResourceType::Groups => {
            let upd: ApiGroupUpdate2 = serde_json::from_value(req)?;
            let mut lock = state.res.lock().await;

            let uuid = lock.from_id_v1(id)?;

            match lock.get_resource_by_id(&uuid)?.obj {
                Resource::EntertainmentConfiguration(_ec) => {
                    lock.update(&uuid, |ec: &mut EntertainmentConfiguration| {
                        ec.status = if upd.stream.active {
                            EntertainmentConfigurationStatus::Active
                        } else {
                            EntertainmentConfigurationStatus::Inactive
                        };
                    })?;
                }
                _ => {}
            }

            Ok(Json(V1Reply::for_group(id).json()))
        }
        ApiResourceType::Config
        | ApiResourceType::Lights
        | ApiResourceType::Resourcelinks
        | ApiResourceType::Rules
        | ApiResourceType::Scenes
        | ApiResourceType::Schedules
        | ApiResourceType::Sensors
        | ApiResourceType::Capabilities => Err(ApiV1Error::V1CreateUnsupported(artype)),
    }
}

async fn put_api_user_resource_id_path(
    State(state): State<AppState>,
    Path((_username, artype, id, path)): Path<(String, ApiResourceType, u32, String)>,
    Json(req): Json<Value>,
) -> ApiV1Result<Json<Value>> {
    match artype {
        ApiResourceType::Lights => {
            log::debug!("req: {}", serde_json::to_string_pretty(&req)?);
            if path != "state" {
                return Err(HueError::V1NotFound(id))?;
            }

            let lock = state.res.lock().await;
            let uuid = lock.from_id_v1(id)?;
            let link = ResourceLink::new(uuid, RType::Light);
            let updv1: ApiLightStateUpdate = serde_json::from_value(req)?;

            let upd = LightUpdate::from(&updv1);

            lock.backend_request(BackendRequest::LightUpdate(link, upd))?;
            drop(lock);

            let reply = V1Reply::for_light(id, &path).with_light_state_update(&updv1)?;

            Ok(Json(reply.json()))
        }

        /* handle groups, exceot for group 0 ("all groups") */
        ApiResourceType::Groups if id != 0 => {
            if path != "action" {
                return Err(HueError::V1NotFound(id))?;
            }

            let lock = state.res.lock().await;

            let uuid = lock.from_id_v1(id)?;
            let link = ResourceLink::new(uuid, RType::Room);

            let room: &Room = lock.get(&link)?;
            let glight = room.grouped_light_service().unwrap();

            let updv1: ApiGroupActionUpdate = serde_json::from_value(req)?;

            let reply = match updv1 {
                ApiGroupActionUpdate::LightUpdate(upd) => {
                    let updv2 = GroupedLightUpdate::from(&upd);

                    lock.backend_request(BackendRequest::GroupedLightUpdate(*glight, updv2))?;
                    drop(lock);

                    V1Reply::for_group_path(id, &path).with_light_state_update(&upd)?
                }
                ApiGroupActionUpdate::GroupUpdate(upd) => {
                    let scene_id = upd.scene.parse().map_err(ApiError::ParseIntError)?;
                    let scene_uuid = lock.from_id_v1(scene_id)?;
                    let rlink = RType::Scene.link_to(scene_uuid);
                    let updv2 = SceneUpdate::new().with_recall_action(Some(SceneStatus {
                        active: SceneActive::Static,
                        last_recall: None,
                    }));
                    lock.backend_request(BackendRequest::SceneUpdate(rlink, updv2))?;
                    drop(lock);

                    V1Reply::for_group_path(id, &path).add("scene", upd.scene)?
                }
            };

            Ok(Json(reply.json()))
        }

        /* handle group 0 ("all groups") */
        ApiResourceType::Groups => {
            if path != "action" {
                return Err(HueError::V1NotFound(id))?;
            }

            let lock = state.res.lock().await;

            let updv1: ApiGroupActionUpdate = serde_json::from_value(req)?;

            let reply = match updv1 {
                ApiGroupActionUpdate::LightUpdate(upd) => {
                    let updv2 = GroupedLightUpdate::from(&upd);

                    for res in lock.get_resources_by_type(RType::GroupedLight) {
                        let link = RType::GroupedLight.link_to(res.id);
                        let req = BackendRequest::GroupedLightUpdate(link, updv2.clone());
                        lock.backend_request(req)?;
                    }

                    drop(lock);

                    V1Reply::for_group_path(id, &path).with_light_state_update(&upd)?
                }
                ApiGroupActionUpdate::GroupUpdate(_api_group_update) => {
                    return Err(HueError::V1NotFound(id))?;
                }
            };

            Ok(Json(reply.json()))
        }

        ApiResourceType::Config
        | ApiResourceType::Resourcelinks
        | ApiResourceType::Rules
        | ApiResourceType::Scenes
        | ApiResourceType::Schedules
        | ApiResourceType::Sensors
        | ApiResourceType::Capabilities => Err(ApiV1Error::V1CreateUnsupported(artype)),
    }
}

/// This generates a workaround necessary for iConnectHue (iPhone app)
///
/// For some reason, iConnectHue has been observed to try the endpoint GET /api/newUser,
/// even though this does not seem to ever have been a valid hue endpoint.
///
/// 2025-01-24: This response has been confirmed to work by Alexa and Peter Miller on discord.
pub async fn workaround_iconnect_hue() -> ApiV1Result<()> {
    Err(HueApiV1Error::UnauthorizedUser)?
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(post_api))
        .route("/config", get(get_api_config))
        .route("/nouser/config", get(get_api_config))
        .route("/newUser", get(workaround_iconnect_hue))
        .route("/{user}", get(get_api_user))
        .route("/{user}/{rtype}", get(get_api_user_resource))
        .route("/{user}/{rtype}", post(post_api_user_resource))
        .route("/{user}/{rtype}", put(put_api_user_resource))
        .route("/{user}/{rtype}/{id}", get(get_api_user_resource_id))
        .route("/{user}/{rtype}/{id}", put(put_api_user_resource_id))
        .route(
            "/{user}/{rtype}/{id}/{key}",
            put(put_api_user_resource_id_path),
        )
}
