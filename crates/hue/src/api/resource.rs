use std::fmt::{self, Debug};
use std::hash::{DefaultHasher, Hash, Hasher};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::Resource;

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
