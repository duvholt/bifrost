mod behavior;
mod device;
mod entertainment;
mod entertainment_config;
mod grouped_light;
mod light;
mod resource;
mod room;
mod scene;
mod stream;
mod stubs;
mod update;
mod zigbee_device_discovery;

pub use behavior::{
    BehaviorInstance, BehaviorInstanceConfiguration, BehaviorInstanceMetadata, BehaviorInstanceNew,
    BehaviorInstanceUpdate, BehaviorScript, BehaviorScriptMetadata, WakeupConfiguration,
    WakeupStyle,
};
pub use device::{Device, DeviceArchetype, DeviceProductData, DeviceUpdate, Identify};
pub use entertainment::{Entertainment, EntertainmentSegment, EntertainmentSegments};
pub use entertainment_config::{
    EntertainmentConfiguration, EntertainmentConfigurationAction,
    EntertainmentConfigurationChannels, EntertainmentConfigurationLocations,
    EntertainmentConfigurationLocationsNew, EntertainmentConfigurationLocationsUpdate,
    EntertainmentConfigurationMetadata, EntertainmentConfigurationNew,
    EntertainmentConfigurationServiceLocations, EntertainmentConfigurationServiceLocationsNew,
    EntertainmentConfigurationServiceLocationsUpdate, EntertainmentConfigurationStatus,
    EntertainmentConfigurationStreamMembers, EntertainmentConfigurationStreamProxy,
    EntertainmentConfigurationStreamProxyMode, EntertainmentConfigurationStreamProxyUpdate,
    EntertainmentConfigurationType, EntertainmentConfigurationUpdate, Position,
};
pub use grouped_light::{GroupedLight, GroupedLightDynamicsUpdate, GroupedLightUpdate};
pub use light::{
    ColorGamut, ColorTemperature, ColorTemperatureUpdate, ColorUpdate, Delta, Dimming,
    DimmingUpdate, GamutType, Light, LightAlert, LightColor, LightDynamics, LightDynamicsStatus,
    LightDynamicsUpdate, LightEffect, LightEffectActionUpdate, LightEffectParameters,
    LightEffectStatus, LightEffectValues, LightEffects, LightEffectsV2, LightEffectsV2Update,
    LightFunction, LightGradient, LightGradientMode, LightGradientPoint, LightGradientUpdate,
    LightMetadata, LightMode, LightPowerup, LightPowerupColor, LightPowerupDimming, LightPowerupOn,
    LightPowerupPreset, LightProductData, LightSignal, LightSignaling, LightTimedEffect,
    LightTimedEffects, LightTimedEffectsUpdate, LightUpdate, MirekSchema, On,
};
pub use resource::{RType, ResourceLink, ResourceRecord};
pub use room::{Room, RoomArchetype, RoomMetadata, RoomMetadataUpdate, RoomUpdate};
pub use scene::{
    Scene, SceneAction, SceneActionElement, SceneActive, SceneMetadata, SceneRecall, SceneStatus,
    SceneStatusEnum, SceneUpdate,
};
use serde::ser::SerializeMap;
pub use stream::HueStreamKey;
pub use stubs::{
    Bridge, BridgeHome, Button, ButtonData, ButtonMetadata, ButtonReport, DevicePower,
    DeviceSoftwareUpdate, DollarRef, GeofenceClient, Geolocation, GroupedLightLevel, GroupedMotion,
    Homekit, LightLevel, Matter, Metadata, MetadataUpdate, Motion, PrivateGroup, PublicImage,
    RelativeRotary, SmartScene, Taurus, Temperature, TimeZone, ZigbeeConnectivity,
    ZigbeeConnectivityStatus, Zone,
};
pub use update::Update;
pub use zigbee_device_discovery::{
    ZigbeeDeviceDiscovery, ZigbeeDeviceDiscoveryAction, ZigbeeDeviceDiscoveryInstallCode,
    ZigbeeDeviceDiscoveryStatus, ZigbeeDeviceDiscoveryUpdate, ZigbeeDeviceDiscoveryUpdateAction,
    ZigbeeDeviceDiscoveryUpdateActionType,
};

use std::fmt::Debug;

use serde::{Deserialize, Serialize};
use serde_json::{Value, from_value, json};

use crate::error::{HueError, HueResult};
use crate::legacy_api::ApiLightStateUpdate;

#[derive(Debug, Deserialize, Clone, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Stub;

impl Serialize for Stub {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_map(None)?.end()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Resource {
    AuthV1(ResourceLink),
    BehaviorInstance(BehaviorInstance),
    BehaviorScript(BehaviorScript),
    Bridge(Bridge),
    BridgeHome(BridgeHome),
    Button(Button),
    Device(Device),
    DevicePower(DevicePower),
    DeviceSoftwareUpdate(DeviceSoftwareUpdate),
    Entertainment(Entertainment),
    EntertainmentConfiguration(EntertainmentConfiguration),
    GeofenceClient(GeofenceClient),
    Geolocation(Geolocation),
    GroupedLight(GroupedLight),
    GroupedLightLevel(GroupedLightLevel),
    GroupedMotion(GroupedMotion),
    Homekit(Homekit),
    Light(Light),
    LightLevel(LightLevel),
    Matter(Matter),
    Motion(Motion),
    PrivateGroup(PrivateGroup),
    PublicImage(PublicImage),
    RelativeRotary(RelativeRotary),
    Room(Room),
    Scene(Scene),
    SmartScene(SmartScene),
    #[serde(rename = "taurus_7455")]
    Taurus(Taurus),
    Temperature(Temperature),
    ZigbeeConnectivity(ZigbeeConnectivity),
    ZigbeeDeviceDiscovery(ZigbeeDeviceDiscovery),
    Zone(Zone),

    /* Unmapped variants */
    CameraMotion(Value),
    Contact(Value),
    MatterFabric(Value),
    ServiceGroup(Value),
    Tamper(Value),
    ZgpConnectivity(Value),
}

impl Resource {
    #[must_use]
    pub const fn rtype(&self) -> RType {
        match self {
            Self::AuthV1(_) => RType::AuthV1,
            Self::BehaviorInstance(_) => RType::BehaviorInstance,
            Self::BehaviorScript(_) => RType::BehaviorScript,
            Self::Bridge(_) => RType::Bridge,
            Self::BridgeHome(_) => RType::BridgeHome,
            Self::Button(_) => RType::Button,
            Self::CameraMotion(_) => RType::CameraMotion,
            Self::Contact(_) => RType::Contact,
            Self::Device(_) => RType::Device,
            Self::DevicePower(_) => RType::DevicePower,
            Self::DeviceSoftwareUpdate(_) => RType::DeviceSoftwareUpdate,
            Self::Entertainment(_) => RType::Entertainment,
            Self::EntertainmentConfiguration(_) => RType::EntertainmentConfiguration,
            Self::GeofenceClient(_) => RType::GeofenceClient,
            Self::Geolocation(_) => RType::Geolocation,
            Self::GroupedLight(_) => RType::GroupedLight,
            Self::GroupedLightLevel(_) => RType::GroupedLightLevel,
            Self::GroupedMotion(_) => RType::GroupedMotion,
            Self::Homekit(_) => RType::Homekit,
            Self::Light(_) => RType::Light,
            Self::LightLevel(_) => RType::LightLevel,
            Self::Matter(_) => RType::Matter,
            Self::MatterFabric(_) => RType::MatterFabric,
            Self::Motion(_) => RType::Motion,
            Self::PrivateGroup(_) => RType::PrivateGroup,
            Self::PublicImage(_) => RType::PublicImage,
            Self::RelativeRotary(_) => RType::RelativeRotary,
            Self::Room(_) => RType::Room,
            Self::Scene(_) => RType::Scene,
            Self::ServiceGroup(_) => RType::ServiceGroup,
            Self::SmartScene(_) => RType::SmartScene,
            Self::Tamper(_) => RType::Tamper,
            Self::Taurus(_) => RType::Taurus,
            Self::Temperature(_) => RType::Temperature,
            Self::ZgpConnectivity(_) => RType::ZgpConnectivity,
            Self::ZigbeeConnectivity(_) => RType::ZigbeeConnectivity,
            Self::ZigbeeDeviceDiscovery(_) => RType::ZigbeeDeviceDiscovery,
            Self::Zone(_) => RType::Zone,
        }
    }

    #[allow(clippy::match_same_arms)]
    #[must_use]
    pub const fn owner(&self) -> Option<ResourceLink> {
        match self {
            Self::AuthV1(_) => None,
            Self::BehaviorInstance(_) => None,
            Self::BehaviorScript(_) => None,
            Self::Bridge(obj) => Some(obj.owner),
            Self::BridgeHome(_) => None,
            Self::Button(obj) => Some(obj.owner),
            Self::Device(_) => None,
            Self::DevicePower(obj) => Some(obj.owner),
            Self::DeviceSoftwareUpdate(obj) => Some(obj.owner),
            Self::Entertainment(obj) => Some(obj.owner),
            Self::EntertainmentConfiguration(_) => None,
            Self::GeofenceClient(_) => None,
            Self::Geolocation(_) => None,
            Self::GroupedLight(obj) => Some(obj.owner),
            Self::GroupedLightLevel(obj) => Some(obj.owner),
            Self::GroupedMotion(obj) => Some(obj.owner),
            Self::Homekit(_) => None,
            Self::Light(obj) => Some(obj.owner),
            Self::LightLevel(obj) => Some(obj.owner),
            Self::Matter(_) => None,
            Self::Motion(obj) => Some(obj.owner),
            Self::PrivateGroup(_) => None,
            Self::PublicImage(_) => None,
            Self::RelativeRotary(obj) => Some(obj.owner),
            Self::Room(_) => None,
            Self::Scene(_) => None,
            Self::SmartScene(_) => None,
            Self::Taurus(obj) => Some(obj.owner),
            Self::Temperature(obj) => Some(obj.owner),
            Self::ZigbeeConnectivity(obj) => Some(obj.owner),
            Self::ZigbeeDeviceDiscovery(obj) => Some(obj.owner),
            Self::Zone(_) => None,

            /* Unmapped variants */
            Self::CameraMotion(_) => None,
            Self::Contact(_) => None,
            Self::MatterFabric(_) => None,
            Self::ServiceGroup(_) => None,
            Self::Tamper(_) => None,
            Self::ZgpConnectivity(_) => None,
        }
    }

    pub fn from_value(rtype: RType, obj: Value) -> HueResult<Self> {
        let res = match rtype {
            RType::AuthV1 => Self::AuthV1(from_value(obj)?),
            RType::BehaviorInstance => Self::BehaviorInstance(from_value(obj)?),
            RType::BehaviorScript => Self::BehaviorScript(from_value(obj)?),
            RType::Bridge => Self::Bridge(from_value(obj)?),
            RType::BridgeHome => Self::BridgeHome(from_value(obj)?),
            RType::Button => Self::Button(from_value(obj)?),
            RType::Device => Self::Device(from_value(obj)?),
            RType::DevicePower => Self::DevicePower(from_value(obj)?),
            RType::DeviceSoftwareUpdate => Self::DeviceSoftwareUpdate(from_value(obj)?),
            RType::Entertainment => Self::Entertainment(from_value(obj)?),
            RType::EntertainmentConfiguration => Self::EntertainmentConfiguration(from_value(obj)?),
            RType::GeofenceClient => Self::GeofenceClient(from_value(obj)?),
            RType::Geolocation => Self::Geolocation(from_value(obj)?),
            RType::GroupedLight => Self::GroupedLight(from_value(obj)?),
            RType::GroupedLightLevel => Self::GroupedLightLevel(from_value(obj)?),
            RType::GroupedMotion => Self::GroupedMotion(from_value(obj)?),
            RType::Homekit => Self::Homekit(from_value(obj)?),
            RType::Light => Self::Light(from_value(obj)?),
            RType::LightLevel => Self::LightLevel(from_value(obj)?),
            RType::Matter => Self::Matter(from_value(obj)?),
            RType::Motion => Self::Motion(from_value(obj)?),
            RType::PrivateGroup => Self::PrivateGroup(from_value(obj)?),
            RType::PublicImage => Self::PublicImage(from_value(obj)?),
            RType::RelativeRotary => Self::RelativeRotary(from_value(obj)?),
            RType::Room => Self::Room(from_value(obj)?),
            RType::Scene => Self::Scene(from_value(obj)?),
            RType::SmartScene => Self::SmartScene(from_value(obj)?),
            RType::Taurus => Self::Taurus(from_value(obj)?),
            RType::Temperature => Self::Temperature(from_value(obj)?),
            RType::ZigbeeConnectivity => Self::ZigbeeConnectivity(from_value(obj)?),
            RType::ZigbeeDeviceDiscovery => Self::ZigbeeDeviceDiscovery(from_value(obj)?),
            RType::Zone => Self::Zone(from_value(obj)?),
            RType::CameraMotion => Self::CameraMotion(obj),
            RType::Contact => Self::Contact(obj),
            RType::MatterFabric => Self::MatterFabric(obj),
            RType::ServiceGroup => Self::ServiceGroup(obj),
            RType::Tamper => Self::Tamper(obj),
            RType::ZgpConnectivity => Self::ZgpConnectivity(obj),
        };
        Ok(res)
    }
}

#[macro_export]
macro_rules! resource_conversion_impl {
    ( $name:ident ) => {
        impl<'a> TryFrom<&'a mut Resource> for &'a mut $name {
            type Error = HueError;

            fn try_from(value: &'a mut Resource) -> Result<Self, Self::Error> {
                if let Resource::$name(obj) = value {
                    Ok(obj)
                } else {
                    Err(HueError::WrongType(RType::Light, value.rtype()))
                }
            }
        }

        impl<'a> TryFrom<&'a Resource> for &'a $name {
            type Error = HueError;

            fn try_from(value: &'a Resource) -> Result<Self, Self::Error> {
                if let Resource::$name(obj) = value {
                    Ok(obj)
                } else {
                    Err(HueError::WrongType(RType::Light, value.rtype()))
                }
            }
        }

        impl TryFrom<Resource> for $name {
            type Error = HueError;

            fn try_from(value: Resource) -> Result<Self, Self::Error> {
                if let Resource::$name(obj) = value {
                    Ok(obj)
                } else {
                    Err(HueError::WrongType(RType::Light, value.rtype()))
                }
            }
        }

        impl From<$name> for Resource {
            fn from(value: $name) -> Self {
                Resource::$name(value)
            }
        }
    };
}

// AuthV1 is not a real resource (only used in links)
// resource_conversion_impl!(AuthV1);
resource_conversion_impl!(BehaviorInstance);
resource_conversion_impl!(BehaviorScript);
resource_conversion_impl!(Bridge);
resource_conversion_impl!(BridgeHome);
resource_conversion_impl!(Button);
resource_conversion_impl!(Device);
resource_conversion_impl!(DevicePower);
resource_conversion_impl!(DeviceSoftwareUpdate);
resource_conversion_impl!(Entertainment);
resource_conversion_impl!(EntertainmentConfiguration);
resource_conversion_impl!(GeofenceClient);
resource_conversion_impl!(Geolocation);
resource_conversion_impl!(GroupedLight);
resource_conversion_impl!(GroupedLightLevel);
resource_conversion_impl!(GroupedMotion);
resource_conversion_impl!(Homekit);
resource_conversion_impl!(Light);
resource_conversion_impl!(LightLevel);
resource_conversion_impl!(Matter);
resource_conversion_impl!(Motion);
resource_conversion_impl!(PrivateGroup);
resource_conversion_impl!(PublicImage);
resource_conversion_impl!(RelativeRotary);
resource_conversion_impl!(Room);
resource_conversion_impl!(Scene);
resource_conversion_impl!(SmartScene);
resource_conversion_impl!(Taurus);
resource_conversion_impl!(Temperature);
resource_conversion_impl!(ZigbeeConnectivity);
resource_conversion_impl!(ZigbeeDeviceDiscovery);
resource_conversion_impl!(Zone);

#[derive(Clone, Debug, Serialize)]
pub struct V1Reply<'a> {
    prefix: String,
    success: Vec<(&'a str, Value)>,
}

impl<'a> V1Reply<'a> {
    #[must_use]
    pub const fn new(prefix: String) -> Self {
        Self {
            prefix,
            success: vec![],
        }
    }

    #[must_use]
    pub fn for_light(id: u32, path: &str) -> Self {
        Self::new(format!("/lights/{id}/{path}"))
    }

    #[must_use]
    pub fn for_group_path(id: u32, path: &str) -> Self {
        Self::new(format!("/groups/{id}/{path}"))
    }

    #[must_use]
    pub fn for_group(id: u32) -> Self {
        Self::new(format!("/groups/{id}"))
    }

    pub fn with_light_state_update(self, upd: &ApiLightStateUpdate) -> HueResult<Self> {
        self.add_option("on", upd.on)?
            .add_option("bri", upd.bri)?
            .add_option("xy", upd.xy)?
            .add_option("ct", upd.ct)?
            .add_option("transitiontime", upd.transitiontime)
    }

    pub fn add<T: Serialize>(mut self, name: &'a str, value: T) -> HueResult<Self> {
        self.success.push((name, serde_json::to_value(value)?));
        Ok(self)
    }

    pub fn add_option<T: Serialize>(mut self, name: &'a str, value: Option<T>) -> HueResult<Self> {
        if let Some(val) = value {
            self.success.push((name, serde_json::to_value(val)?));
        }
        Ok(self)
    }

    #[must_use]
    pub fn json(self) -> Value {
        let mut json = vec![];
        let prefix = self.prefix;
        for (name, value) in self.success {
            json.push(json!({"success": {format!("{prefix}/{name}"): value}}));
        }
        json!(json)
    }
}
