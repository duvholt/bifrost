use std::collections::BTreeMap;

use axum::Router;
use axum::extract::{Path, State};
use axum::routing::{get, put};
use uuid::Uuid;

use bifrost_api::service::{Service, ServiceList};
use svc::traits::ServiceState;

use crate::routes::bifrost::BifrostApiResult;
use crate::routes::extractor::Json;
use crate::server::appstate::AppState;

async fn service_list(state: &AppState) -> BifrostApiResult<ServiceList> {
    let mut svm = state.manager();

    let mut services = BTreeMap::new();
    for (id, name) in svm.list().await? {
        let state = svm.status(id).await?;

        let service = Service { id, name, state };
        services.insert(id, service);
    }

    Ok(ServiceList { services })
}

async fn get_services(State(state): State<AppState>) -> BifrostApiResult<Json<ServiceList>> {
    Ok(Json(service_list(&state).await?))
}

async fn put_service(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(service_state): Json<ServiceState>,
) -> BifrostApiResult<Json<()>> {
    let mut mgr = state.manager();

    match service_state {
        ServiceState::Registered
        | ServiceState::Configured
        | ServiceState::Starting
        | ServiceState::Stopping
        | ServiceState::Failed => {}
        ServiceState::Running => mgr.start(id).await?,
        ServiceState::Stopped => mgr.stop(id).await?,
    }

    Ok(Json(()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_services))
        .route("/{id}", put(put_service))
}
