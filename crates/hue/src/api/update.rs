use serde::{Deserialize, Serialize};
use serde_json::Value;

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
}
