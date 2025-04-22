pub mod device;
pub mod entertainment;
pub mod entertainment_configuration;
pub mod generic;
pub mod grouped_light;
pub mod light;
pub mod scene;

use axum::Router;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ApiResult;
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

#[allow(clippy::unnecessary_wraps)]
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

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/scene", scene::router())
        .nest("/light", light::router())
        .nest("/device", device::router())
        .nest("/grouped_light", grouped_light::router())
        .nest(
            "/entertainment_configuration",
            entertainment_configuration::router(),
        )
        .nest("/entertainment/", entertainment::router())
        .merge(generic::router())
}
