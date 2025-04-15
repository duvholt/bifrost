use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::api::{
    DeviceUpdate, EntertainmentConfigurationUpdate, GroupedLightUpdate, LightUpdate, RType,
    RoomUpdate, SceneUpdate,
};

type BridgeUpdate = Value;
type BridgeHomeUpdate = Value;
type ZigbeeDeviceDiscoveryUpdate = Value;
type BehaviorInstanceUpdate = Value;
type SmartSceneUpdate = Value;
type ZoneUpdate = Value;
type GeolocationUpdate = Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Update {
    /* BehaviorScript(BehaviorScriptUpdate), */
    BehaviorInstance(BehaviorInstanceUpdate),
    Bridge(BridgeUpdate),
    BridgeHome(BridgeHomeUpdate),
    Device(DeviceUpdate),
    /* Entertainment(EntertainmentUpdate), */
    EntertainmentConfiguration(EntertainmentConfigurationUpdate),
    /* GeofenceClient(GeofenceClientUpdate), */
    Geolocation(GeolocationUpdate),
    GroupedLight(GroupedLightUpdate),
    /* Homekit(HomekitUpdate), */
    Light(LightUpdate),
    /* Matter(MatterUpdate), */
    /* PublicImage(PublicImageUpdate), */
    Room(RoomUpdate),
    Scene(SceneUpdate),
    SmartScene(SmartSceneUpdate),
    /* ZigbeeConnectivity(ZigbeeConnectivityUpdate), */
    ZigbeeDeviceDiscovery(ZigbeeDeviceDiscoveryUpdate),
    Zone(ZoneUpdate),
}

impl Update {
    #[must_use]
    pub const fn rtype(&self) -> RType {
        match self {
            Self::BehaviorInstance(_) => RType::BehaviorInstance,
            Self::Bridge(_) => RType::Bridge,
            Self::BridgeHome(_) => RType::BridgeHome,
            Self::Device(_) => RType::Device,
            Self::EntertainmentConfiguration(_) => RType::EntertainmentConfiguration,
            Self::Geolocation(_) => RType::Geolocation,
            Self::GroupedLight(_) => RType::GroupedLight,
            Self::Light(_) => RType::Light,
            Self::Room(_) => RType::Room,
            Self::Scene(_) => RType::Scene,
            Self::SmartScene(_) => RType::SmartScene,
            Self::ZigbeeDeviceDiscovery(_) => RType::ZigbeeDeviceDiscovery,
            Self::Zone(_) => RType::Zone,
        }
    }

    #[must_use]
    pub fn id_v1_scope(&self, id: u32, uuid: &Uuid) -> Option<String> {
        match self {
            Self::BehaviorInstance(_)
            | Self::Bridge(_)
            | Self::BridgeHome(_)
            | Self::Geolocation(_)
            | Self::ZigbeeDeviceDiscovery(_)
            | Self::Zone(_) => None,

            Self::Room(_) | Self::GroupedLight(_) | Self::EntertainmentConfiguration(_) => {
                Some(format!("/groups/{id}"))
            }
            Self::Device(_) => Some(format!("/device/{id}")),
            Self::Light(_) => Some(format!("/lights/{id}")),
            Self::Scene(_) | Self::SmartScene(_) => Some(format!("/scenes/{uuid}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRecord {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_v1: Option<String>,
    #[serde(flatten)]
    pub upd: Update,
}

impl UpdateRecord {
    #[must_use]
    pub fn new(uuid: &Uuid, id_v1: Option<u32>, upd: Update) -> Self {
        Self {
            id: *uuid,
            id_v1: id_v1.and_then(|id| upd.id_v1_scope(id, uuid)),
            upd,
        }
    }
}
