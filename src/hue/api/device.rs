use std::collections::BTreeSet;
use std::ops::{AddAssign, Sub};

use serde::{Deserialize, Serialize};

use crate::hue::api::{Metadata, MetadataUpdate, RType, ResourceLink, Stub};
use crate::hue::version::SwVersion;
use crate::hue::HUE_BRIDGE_V2_MODEL_ID;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Device {
    pub product_data: DeviceProductData,
    pub metadata: Metadata,
    pub services: BTreeSet<ResourceLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usertest: Option<UserTest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identify: Option<Stub>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DeviceUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MetadataUpdate>,
}

impl Device {
    #[must_use]
    pub fn light_service(&self) -> Option<&ResourceLink> {
        self.services.iter().find(|rl| rl.rtype == RType::Light)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Identify {}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct UserTest {
    status: String,
    usertest: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceProductData {
    pub model_id: String,
    pub manufacturer_name: String,
    pub product_name: String,
    pub product_archetype: DeviceArchetype,
    pub certified: bool,
    pub software_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_platform_type: Option<String>,
}

impl DeviceProductData {
    pub const SIGNIFY_MANUFACTURER_NAME: &'static str = "Signify Netherlands B.V.";

    #[must_use]
    pub fn hue_bridge_v2(version: &SwVersion) -> Self {
        Self {
            certified: true,
            manufacturer_name: Self::SIGNIFY_MANUFACTURER_NAME.to_string(),
            model_id: HUE_BRIDGE_V2_MODEL_ID.to_string(),
            product_archetype: DeviceArchetype::BridgeV2,
            product_name: "Hue Bridge".to_string(),
            software_version: version.get_software_version(),
            hardware_platform_type: None,
        }
    }
}

impl DeviceUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_metadata(self, metadata: Metadata) -> Self {
        Self {
            metadata: Some(MetadataUpdate {
                archetype: Some(metadata.archetype),
                name: Some(metadata.name),
            }),
        }
    }
}

impl AddAssign<DeviceUpdate> for Device {
    fn add_assign(&mut self, upd: DeviceUpdate) {
        if let Some(md) = upd.metadata {
            if let Some(name) = md.name {
                self.metadata.name = name;
            }
            if let Some(archetype) = md.archetype {
                self.metadata.archetype = archetype;
            }
        }
    }
}

#[allow(clippy::if_not_else)]
impl Sub<&Device> for &Device {
    type Output = DeviceUpdate;

    fn sub(self, rhs: &Device) -> Self::Output {
        let mut upd = Self::Output::default();

        if self.metadata != rhs.metadata {
            upd.metadata = Some(MetadataUpdate {
                name: if self.metadata.name != rhs.metadata.name {
                    Some(rhs.metadata.name.clone())
                } else {
                    None
                },
                archetype: if self.metadata.archetype != rhs.metadata.archetype {
                    Some(rhs.metadata.archetype.clone())
                } else {
                    None
                },
            });
        }

        upd
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceArchetype {
    BridgeV2,
    #[default]
    UnknownArchetype,
    ClassicBulb,
    SultanBulb,
    FloodBulb,
    SpotBulb,
    CandleBulb,
    LusterBulb,
    PendantRound,
    PendantLong,
    CeilingRound,
    CeilingSquare,
    FloorShade,
    FloorLantern,
    TableShade,
    RecessedCeiling,
    RecessedFloor,
    SingleSpot,
    DoubleSpot,
    TableWash,
    WallLantern,
    WallShade,
    FlexibleLamp,
    GroundSpot,
    WallSpot,
    Plug,
    HueGo,
    HueLightstrip,
    HueIris,
    HueBloom,
    Bollard,
    WallWasher,
    HuePlay,
    VintageBulb,
    VintageCandleBulb,
    EllipseBulb,
    TriangleBulb,
    SmallGlobeBulb,
    LargeGlobeBulb,
    EdisonBulb,
    ChristmasTree,
    StringLight,
    HueCentris,
    HueLightstripTv,
    HueLightstripPc,
    HueTube,
    HueSigne,
    PendantSpot,
    CeilingHorizontal,
    CeilingTube,

    #[serde(untagged)]
    Other(String),
}
