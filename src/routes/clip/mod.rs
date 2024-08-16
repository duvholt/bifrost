pub mod scene;

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use hyper::StatusCode;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::hue::api::{GroupedLightUpdate, LightUpdate, RType, Resource, ResourceLink, V2Reply};
use crate::state::AppState;
use crate::z2m::request::ClientRequest;
use crate::z2m::update::DeviceUpdate;

type ApiV2Result = ApiResult<Json<V2Reply<Value>>>;

impl<T: Serialize> V2Reply<T> {
    #[allow(clippy::unnecessary_wraps)]
    fn ok(obj: T) -> ApiV2Result {
        Ok(Json(V2Reply {
            data: vec![serde_json::to_value(obj)?],
            errors: vec![],
        }))
    }

    #[allow(clippy::unnecessary_wraps)]
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

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error_msg = format!("{self}");
        log::error!("Request failed: {error_msg}");
        let res = Json(V2Reply::<Value> {
            data: vec![],
            errors: vec![error_msg],
        });

        let status = match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Full(_) => StatusCode::INSUFFICIENT_STORAGE,
            Self::WrongType(_, _) => StatusCode::NOT_ACCEPTABLE,
            Self::DeleteDenied(_) => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status, res).into_response()
    }
}

async fn get_root(State(state): State<AppState>) -> impl IntoResponse {
    Json(V2Reply {
        data: state.res.lock().await.get_resources(),
        errors: vec![],
    })
}

async fn get_resource(State(state): State<AppState>, Path(rtype): Path<RType>) -> ApiV2Result {
    V2Reply::list(state.res.lock().await.get_resources_by_type(rtype))
}

async fn post_resource(
    State(state): State<AppState>,
    Path(rtype): Path<RType>,
    Json(req): Json<Value>,
) -> impl IntoResponse {
    log::info!("POST: {rtype:?} {}", serde_json::to_string(&req)?);

    let obj = Resource::from_value(rtype, req)?;

    let mut lock = state.res.lock().await;

    let rlink = ResourceLink::new(Uuid::new_v4(), obj.rtype());
    lock.add(&rlink, obj)?;
    drop(lock);

    V2Reply::ok(rlink)
}

#[allow(clippy::option_if_let_else)]
async fn get_resource_id(
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

    let rlink = rtype.link_to(id);
    let lock = state.res.lock().await;
    let res = lock.get_resource(rtype, &id);

    match res?.obj {
        Resource::Light(_) => {
            let upd: LightUpdate = serde_json::from_value(put)?;

            let payload = DeviceUpdate::default()
                .with_state(upd.on.map(|on| on.on))
                .with_brightness(upd.dimming.map(|dim| dim.brightness / 100.0 * 255.0))
                .with_color_temp(upd.color_temperature.map(|ct| ct.mirek))
                .with_color_xy(upd.color.map(|col| col.xy));

            lock.z2m_request(ClientRequest::light_update(rlink, payload))?;
        }

        Resource::GroupedLight(_) => {
            log::info!("PUT {rtype:?}/{id}: updating");

            let upd: GroupedLightUpdate = serde_json::from_value(put)?;

            let payload = DeviceUpdate::default()
                .with_state(upd.on.map(|on| on.on))
                .with_brightness(upd.dimming.map(|dim| dim.brightness / 100.0 * 255.0))
                .with_color_temp(upd.color_temperature.map(|ct| ct.mirek))
                .with_color_xy(upd.color.map(|col| col.xy));

            lock.z2m_request(ClientRequest::group_update(rlink, payload))?;
        }
        resource => {
            log::warn!("PUT {rtype:?}/{id}: state update not supported: {resource:?}");
        }
    }

    drop(lock);

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
        .nest("/scene", scene::router())
        .route("/", get(get_root))
        .route("/:resource", get(get_resource))
        .route("/:resource", post(post_resource))
        .route("/:resource/:id", get(get_resource_id))
        .route("/:resource/:id", put(put_resource_id))
        .route("/:resource/:id", delete(delete_resource_id))
}