use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, oneshot};
use uuid::Uuid;

use hue::api::{
    GroupedLightUpdate, LightUpdate, ResourceLink, RoomNew, RoomUpdate, Scene, SceneUpdate,
    ZigbeeDeviceDiscoveryUpdate, ZoneNew, ZoneUpdate,
};
use hue::stream::HueStreamLightsV2;

use crate::Client;
use crate::config::Z2mServer;
use crate::error::BifrostResult;

pub type RequestReply<T> = Option<Arc<Mutex<Option<oneshot::Sender<T>>>>>;

#[must_use]
pub fn request_reply_channel<T>() -> (RequestReply<T>, oneshot::Receiver<T>) {
    let (tx, rx) = oneshot::channel();
    (Some(Arc::new(Mutex::new(Some(tx)))), rx)
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BackendRequest {
    LightUpdate(ResourceLink, LightUpdate),

    SceneCreate(ResourceLink, u32, Scene),
    SceneUpdate(ResourceLink, SceneUpdate),

    GroupedLightUpdate(ResourceLink, GroupedLightUpdate),

    RoomCreate {
        backend: String,
        room_new: RoomNew,
        #[serde(skip)]
        link_reply: RequestReply<ResourceLink>,
    },
    RoomUpdate(ResourceLink, RoomUpdate),

    ZoneCreate {
        backend: String,
        zone_new: ZoneNew,
        #[serde(skip)]
        link_reply: RequestReply<ResourceLink>,
    },
    ZoneUpdate(ResourceLink, ZoneUpdate),

    Delete {
        link: ResourceLink,
        #[serde(skip)]
        claim: RequestReply<()>,
    },

    EntertainmentStart(Uuid),
    EntertainmentFrame(HueStreamLightsV2),
    EntertainmentStop(),

    ZigbeeDeviceDiscovery(ResourceLink, ZigbeeDeviceDiscoveryUpdate),
}

impl Client {
    pub async fn post_backend(&self, name: &str, backend: Z2mServer) -> BifrostResult<()> {
        self.post(&format!("backend/z2m/{name}"), backend).await
    }
}
