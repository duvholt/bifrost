use serde::{Deserialize, Serialize};
use uuid::Uuid;

use hue::api::{GroupedLightUpdate, LightUpdate, ResourceLink, RoomUpdate, Scene, SceneUpdate};
use hue::stream::HueStreamLightsV2;

use crate::Client;
use crate::config::Z2mServer;
use crate::error::BifrostResult;

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BackendRequest {
    LightUpdate(ResourceLink, LightUpdate),

    SceneCreate(ResourceLink, u32, Scene),
    SceneUpdate(ResourceLink, SceneUpdate),

    GroupedLightUpdate(ResourceLink, GroupedLightUpdate),

    RoomUpdate(ResourceLink, RoomUpdate),

    Delete(ResourceLink),

    EntertainmentStart(Uuid),
    EntertainmentFrame(HueStreamLightsV2),
    EntertainmentStop(),
}

impl Client {
    pub async fn post_backend(&self, name: &str, backend: Z2mServer) -> BifrostResult<()> {
        self.post(&format!("backend/z2m/{name}"), backend).await
    }
}
