pub mod z2m;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;

use crate::error::ApiResult;
use crate::hue::api::{GroupedLightUpdate, LightUpdate, ResourceLink, Scene, SceneUpdate};

#[derive(Clone, Debug)]
pub enum BackendRequest {
    LightUpdate(ResourceLink, LightUpdate),

    SceneCreate(ResourceLink, u32, Scene),
    SceneUpdate(ResourceLink, SceneUpdate),

    GroupedLightUpdate(ResourceLink, GroupedLightUpdate),

    Delete(ResourceLink),
}

#[async_trait]
pub trait Backend {
    async fn run_forever(self, chan: Receiver<Arc<BackendRequest>>) -> ApiResult<()>;
}
