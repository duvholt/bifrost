pub mod device;
pub mod entertainment_configuration;
pub mod grouped_light;
pub mod light;
pub mod room;
pub mod scene;
pub mod zigbee_device_discovery;

use bifrost_api::backend::BackendRequest;
use entertainment_configuration as ent_conf;

use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{delete, get, post, put};
use hue::api::{RType, ResourceLink};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct V2Error {
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct V2Reply<T> {
    pub data: Vec<T>,
    pub errors: Vec<V2Error>,
}

type ApiV2Result = ApiResult<Json<V2Reply<Value>>>;

impl<T: Serialize> V2Reply<T> {
    fn ok(obj: T) -> ApiV2Result {
        Ok(Json(V2Reply {
            data: vec![serde_json::to_value(obj)?],
            errors: vec![],
        }))
    }

    fn list(data: Vec<T>) -> ApiV2Result {
        Ok(Json(V2Reply {
            data: data
                .into_iter()
                .map(|e| serde_json::to_value(e))
                .collect::<Result<_, _>>()?,
            errors: vec![],
        }))
    }
}

async fn get_all_resources(State(state): State<AppState>) -> ApiV2Result {
    let lock = state.res.lock().await;
    let res = lock.get_resources();
    drop(lock);
    V2Reply::list(res)
}

pub async fn get_resource(State(state): State<AppState>, Path(rtype): Path<RType>) -> ApiV2Result {
    let lock = state.res.lock().await;
    let res = lock.get_resources_by_type(rtype);
    drop(lock);
    V2Reply::list(res)
}

async fn post_resource(
    State(state): State<AppState>,
    Path(rtype): Path<RType>,
    Json(req): Json<Value>,
) -> ApiV2Result {
    log::info!("POST {rtype:?}");
    log::debug!("Json data:\n{}", serde_json::to_string_pretty(&req)?);

    match rtype {
        RType::EntertainmentConfiguration => ent_conf::post_resource(&state, req).await,
        RType::Scene => scene::post_scene(&state, req).await,

        /* Not supported yet by Bifrost */
        RType::BehaviorInstance
        | RType::GeofenceClient
        | RType::Room
        | RType::ServiceGroup
        | RType::SmartScene
        | RType::Zone => {
            let err = ApiError::CreateNotYetSupported(rtype);
            log::warn!("{err}");
            Err(err)
        }

        /* Not allowed by protocol */
        RType::AuthV1
        | RType::BehaviorScript
        | RType::Bridge
        | RType::BridgeHome
        | RType::Button
        | RType::CameraMotion
        | RType::Contact
        | RType::Device
        | RType::DevicePower
        | RType::DeviceSoftwareUpdate
        | RType::Entertainment
        | RType::Geolocation
        | RType::GroupedLight
        | RType::GroupedLightLevel
        | RType::GroupedMotion
        | RType::Homekit
        | RType::Light
        | RType::LightLevel
        | RType::Matter
        | RType::MatterFabric
        | RType::Motion
        | RType::PrivateGroup
        | RType::PublicImage
        | RType::RelativeRotary
        | RType::Taurus
        | RType::Tamper
        | RType::Temperature
        | RType::ZgpConnectivity
        | RType::ZigbeeConnectivity
        | RType::ZigbeeDeviceDiscovery => {
            let err = ApiError::CreateNotAllowed(rtype);
            log::error!("{err}");
            Err(err)
        }
    }
}

pub async fn get_resource_id(
    State(state): State<AppState>,
    Path(rlink): Path<ResourceLink>,
) -> ApiV2Result {
    V2Reply::ok(state.res.lock().await.get_resource(&rlink)?)
}

async fn put_resource_id(
    State(state): State<AppState>,
    Path(rlink): Path<ResourceLink>,
    Json(put): Json<Value>,
) -> ApiV2Result {
    log::info!("PUT {rlink:?}");
    log::debug!("Json data:\n{}", serde_json::to_string_pretty(&put)?);

    match rlink.rtype {
        /* Allowed + supported */
        RType::Device => device::put_device(&state, rlink, put).await,
        RType::EntertainmentConfiguration => ent_conf::put_resource_id(&state, rlink, put).await,
        RType::GroupedLight => grouped_light::put_grouped_light(&state, rlink, put).await,
        RType::Light => light::put_light(&state, rlink, put).await,
        RType::Scene => scene::put_scene(&state, rlink, put).await,
        RType::Room => room::put_room(&state, rlink, put).await,
        RType::ZigbeeDeviceDiscovery => {
            zigbee_device_discovery::put_zigbee_device_discovery(&state, rlink, put).await
        }

        /* Allowed, but support is missing in Bifrost */
        RType::BehaviorInstance
        | RType::Bridge
        | RType::Button
        | RType::CameraMotion
        | RType::Contact
        | RType::DevicePower
        | RType::DeviceSoftwareUpdate
        | RType::Entertainment
        | RType::GeofenceClient
        | RType::Geolocation
        | RType::GroupedLightLevel
        | RType::GroupedMotion
        | RType::Homekit
        | RType::LightLevel
        | RType::Matter
        | RType::Motion
        | RType::RelativeRotary
        | RType::ServiceGroup
        | RType::SmartScene
        | RType::Temperature
        | RType::ZgpConnectivity
        | RType::ZigbeeConnectivity
        | RType::Zone => {
            /* check that the resource exists, otherwise we should return 404 */
            state.res.lock().await.get_resource(&rlink)?;

            let err = ApiError::UpdateNotYetSupported(rlink.rtype);
            log::warn!("{err}");
            Err(err)
        }

        /* Not allowed by protocol */
        RType::AuthV1
        | RType::BehaviorScript
        | RType::BridgeHome
        | RType::MatterFabric
        | RType::PrivateGroup
        | RType::PublicImage
        | RType::Taurus
        | RType::Tamper => {
            let err = ApiError::UpdateNotAllowed(rlink.rtype);
            log::error!("{err}");
            Err(err)
        }
    }
}

async fn delete_resource_id(
    State(state): State<AppState>,
    Path(rlink): Path<ResourceLink>,
) -> ApiV2Result {
    log::info!("DELETE {rlink:?}");

    match rlink.rtype {
        /* Allowed (send request to backend) */
        RType::BehaviorInstance
        | RType::Device
        | RType::EntertainmentConfiguration
        | RType::GeofenceClient
        | RType::MatterFabric
        | RType::Room
        | RType::Scene
        | RType::ServiceGroup
        | RType::SmartScene
        | RType::Zone => {
            let lock = state.res.lock().await;

            /* check that the resource exists, otherwise we should return 404 */
            lock.get_resource(&rlink)?;

            /* request deletion from backend */
            lock.backend_request(BackendRequest::Delete(rlink))?;

            drop(lock);

            V2Reply::ok(rlink)
        }

        /* Not allowed by protocol */
        RType::AuthV1
        | RType::BehaviorScript
        | RType::Bridge
        | RType::BridgeHome
        | RType::Button
        | RType::CameraMotion
        | RType::Contact
        | RType::DevicePower
        | RType::DeviceSoftwareUpdate
        | RType::Entertainment
        | RType::Geolocation
        | RType::GroupedLight
        | RType::GroupedLightLevel
        | RType::GroupedMotion
        | RType::Homekit
        | RType::Light
        | RType::LightLevel
        | RType::Matter
        | RType::Motion
        | RType::PrivateGroup
        | RType::PublicImage
        | RType::RelativeRotary
        | RType::Tamper
        | RType::Taurus
        | RType::Temperature
        | RType::ZgpConnectivity
        | RType::ZigbeeConnectivity
        | RType::ZigbeeDeviceDiscovery => {
            let err = ApiError::DeleteNotAllowed(rlink.rtype);
            log::error!("{err}");
            Err(err)
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_all_resources))
        .route("/{rtype}", get(get_resource))
        .route("/{rtype}", post(post_resource))
        .route("/{rtype}/{rid}", get(get_resource_id))
        .route("/{rtype}/{rid}", put(put_resource_id))
        .route("/{rtype}/{rid}", delete(delete_resource_id))
}
