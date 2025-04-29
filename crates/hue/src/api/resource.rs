use std::fmt::{self, Debug};
use std::hash::{DefaultHasher, Hash, Hasher};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::Resource;

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RType {
    /// Only used in [`ResourceLink`] references
    AuthV1,
    BehaviorInstance,
    BehaviorScript,
    Bridge,
    BridgeHome,
    Button,
    CameraMotion,
    Contact,
    Device,
    DevicePower,
    DeviceSoftwareUpdate,
    Entertainment,
    EntertainmentConfiguration,
    GeofenceClient,
    Geolocation,
    GroupedLight,
    GroupedLightLevel,
    GroupedMotion,
    Homekit,
    Light,
    LightLevel,
    Matter,
    MatterFabric,
    Motion,
    /// Only used in [`ResourceLink`] references
    PrivateGroup,
    /// Only used in [`ResourceLink`] references
    PublicImage,
    RelativeRotary,
    Room,
    Scene,
    ServiceGroup,
    SmartScene,
    #[serde(rename = "taurus_7455")]
    Taurus,
    Tamper,
    Temperature,
    ZgpConnectivity,
    ZigbeeConnectivity,
    ZigbeeDeviceDiscovery,
    Zone,
}

/// Manually implement Hash, so any future additions/reordering of [`RType`]
/// does not affect output of [`RType::deterministic()`]
impl Hash for RType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // these are all set in stone!
        //
        // never change any of these assignments.
        //
        // use a new unique number for future variants
        let index: u64 = match self {
            Self::AuthV1 => 0,
            Self::BehaviorInstance => 1,
            Self::BehaviorScript => 2,
            Self::Bridge => 3,
            Self::BridgeHome => 4,
            Self::Button => 5,
            Self::Device => 6,
            Self::DevicePower => 7,
            Self::DeviceSoftwareUpdate => 8,
            Self::Entertainment => 9,
            Self::EntertainmentConfiguration => 10,
            Self::GeofenceClient => 11,
            Self::Geolocation => 12,
            Self::GroupedLight => 13,
            Self::GroupedLightLevel => 14,
            Self::GroupedMotion => 15,
            Self::Homekit => 16,
            Self::Light => 17,
            Self::LightLevel => 18,
            Self::Matter => 19,
            Self::Motion => 20,
            Self::PrivateGroup => 21,
            Self::PublicImage => 22,
            Self::RelativeRotary => 23,
            Self::Room => 24,
            Self::Scene => 25,
            Self::SmartScene => 26,
            Self::Taurus => 27,
            Self::Temperature => 28,
            Self::ZigbeeConnectivity => 29,
            Self::ZigbeeDeviceDiscovery => 30,
            Self::Zone => 31,

            /* Added later, so not sorted alphabetically */
            Self::CameraMotion => 32,
            Self::Contact => 33,
            Self::MatterFabric => 34,
            Self::ServiceGroup => 35,
            Self::Tamper => 36,
            Self::ZgpConnectivity => 37,
        };

        index.hash(state);
    }
}

fn hash<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

impl RType {
    #[must_use]
    pub const fn link_to(self, rid: Uuid) -> ResourceLink {
        ResourceLink { rid, rtype: self }
    }

    #[must_use]
    pub fn deterministic(self, data: impl Hash) -> ResourceLink {
        /* hash resource type (i.e., self) */
        let h1 = hash(&self);

        /* hash data */
        let h2 = hash(&data);

        /* use resulting bytes for uuid seed */
        let seed: &[u8] = &[h1.to_le_bytes(), h2.to_le_bytes()].concat();

        let rid = Uuid::new_v5(&Uuid::NAMESPACE_OID, seed);

        self.link_to(rid)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceRecord {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_v1: Option<String>,
    #[serde(flatten)]
    pub obj: Resource,
}

impl ResourceRecord {
    #[must_use]
    pub const fn new(id: Uuid, id_v1: Option<String>, obj: Resource) -> Self {
        Self { id, id_v1, obj }
    }
}

#[derive(Copy, Hash, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceLink {
    pub rid: Uuid,
    pub rtype: RType,
}

impl ResourceLink {
    #[must_use]
    pub const fn new(rid: Uuid, rtype: RType) -> Self {
        Self { rid, rtype }
    }
}

impl Debug for ResourceLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rtype = format!("{:?}", self.rtype).to_lowercase();
        let rid = self.rid;
        write!(f, "{rtype}/{rid}")
    }
}
