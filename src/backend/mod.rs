pub mod z2m;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;

use hue::api::{GroupedLightUpdate, LightUpdate, ResourceLink, RoomUpdate, Scene, SceneUpdate};
use hue::stream::HueStreamLights;

use crate::error::ApiResult;

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum BackendRequest {
    LightUpdate(ResourceLink, LightUpdate),

    SceneCreate(ResourceLink, u32, Scene),
    SceneUpdate(ResourceLink, SceneUpdate),

    GroupedLightUpdate(ResourceLink, GroupedLightUpdate),

    RoomUpdate(ResourceLink, RoomUpdate),

    Delete(ResourceLink),

    EntertainmentStart(Uuid),
    EntertainmentFrame(HueStreamLights),
    EntertainmentStop(),
}

#[async_trait]
pub trait Backend {
    async fn run_forever(self, chan: Receiver<Arc<BackendRequest>>) -> ApiResult<()>;
}
