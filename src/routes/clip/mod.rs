pub mod device;
pub mod entertainment_configuration;
pub mod grouped_light;
pub mod light;
pub mod scene;

use entertainment_configuration as ent_conf;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post, put};
use axum::Router;
use hue::api::RType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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

async fn get_all_resources(State(state): State<AppState>) -> impl IntoResponse {
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
) -> impl IntoResponse {
    log::info!("POST: {rtype:?} {}", serde_json::to_string(&req)?);

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
    Path((rtype, id)): Path<(RType, Uuid)>,
) -> ApiV2Result {
    V2Reply::ok(state.res.lock().await.get_resource(rtype, &id)?)
}

async fn put_resource_id(
    State(state): State<AppState>,
    Path((rtype, id)): Path<(RType, Uuid)>,
    Json(put): Json<Value>,
) -> ApiV2Result {
    log::info!("PUT {rtype:?}/{id}");
    log::debug!("json data\n{}", serde_json::to_string_pretty(&put)?);

    match rtype {
        /* Allowed + supported */
        RType::Device => device::put_device(&state, id, put).await,
        RType::EntertainmentConfiguration => ent_conf::put_resource_id(&state, id, put).await,
        RType::GroupedLight => grouped_light::put_grouped_light(&state, id, put).await,
        RType::Light => light::put_light(&state, id, put).await,
        RType::Scene => scene::put_scene(&state, id, put).await,

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
        | RType::Room
        | RType::ServiceGroup
        | RType::SmartScene
        | RType::Temperature
        | RType::ZgpConnectivity
        | RType::ZigbeeConnectivity
        | RType::ZigbeeDeviceDiscovery
        | RType::Zone => {
            /* check that the resource exists, otherwise we should return 404 */
            state.res.lock().await.get_resource(rtype, &id)?;

            let err = ApiError::UpdateNotYetSupported(rtype);
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
            let err = ApiError::UpdateNotAllowed(rtype);
            log::error!("{err}");
            Err(err)
        }
    }
}

async fn delete_resource_id(
    State(state): State<AppState>,
    Path((rtype, id)): Path<(RType, Uuid)>,
) -> ApiV2Result {
    log::info!("DELETE {rtype:?}/{id}");

    match rtype {
        RType::Scene => scene::delete_scene(&state, id).await,

        /* Allowed, but support is missing in Bifrost */
        RType::BehaviorInstance
        | RType::Device
        | RType::EntertainmentConfiguration
        | RType::GeofenceClient
        | RType::MatterFabric
        | RType::Room
        | RType::ServiceGroup
        | RType::SmartScene
        | RType::Zone => {
            /* check that the resource exists, otherwise we should return 404 */
            state.res.lock().await.get_resource(rtype, &id)?;

            let err = ApiError::DeleteNotYetSupported(rtype);
            log::error!("err");
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
            let err = ApiError::DeleteNotAllowed(rtype);
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
        .route("/{rtype}/{id}", get(get_resource_id))
        .route("/{rtype}/{id}", put(put_resource_id))
        .route("/{rtype}/{id}", delete(delete_resource_id))
}
