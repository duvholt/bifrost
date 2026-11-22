use std::fmt::{self, Debug};
use std::hash::{Hash, Hasher};

use serde::{Deserialize, Serialize};
use siphasher::sip::SipHasher13;
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
    Clip,
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
            Self::Clip => 38,
        };

        index.hash(state);
    }
}

fn hash<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut s = SipHasher13::new();
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

    #[must_use]
    pub fn random(self) -> ResourceLink {
        self.link_to(Uuid::new_v4())
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

    #[must_use]
    pub const fn link(&self) -> ResourceLink {
        ResourceLink::new(self.id, self.obj.rtype())
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
        // we need serde(rename_all = "snake_case") translation
        let rtype = serde_json::to_string(&self.rtype).unwrap();
        let rid = self.rid;
        write!(f, "{}/{rid}", rtype.trim_matches('"'))
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::api::{
        Bridge, BridgeHome, Button, Device, Entertainment, GroupedLight, Light, RType, Room, Scene,
        Taurus, ZigbeeConnectivity, ZigbeeDeviceDiscovery, Zone,
    };

    #[test]
    fn bridge_resource_links_preserve_stable_ids() {
        let bridge = uuid!("2fd1b4c0-ebf6-534b-9bde-fca6b1d7f84d");
        let home = uuid!("e1e7f958-59ab-5eec-b4c8-54d08798055b");
        assert_eq!(
            Bridge::resource_link("112233fffe445566"),
            RType::Bridge.link_to(bridge)
        );
        assert_eq!(
            BridgeHome::resource_link("112233fffe445566"),
            RType::BridgeHome.link_to(home)
        );
        assert_eq!(
            Device::bridge_resource_link(bridge),
            RType::Device.link_to(uuid!("7a16c640-6be0-583f-a3f3-5c5d941bada5"))
        );
        assert_eq!(
            Entertainment::bridge_resource_link(bridge),
            RType::Entertainment.link_to(uuid!("75069e57-3629-55b5-9d38-ee7333a034ee"))
        );
        assert_eq!(
            ZigbeeDeviceDiscovery::resource_link(bridge),
            RType::ZigbeeDeviceDiscovery.link_to(uuid!("030642bd-5d16-5809-a2ad-cc537922e376"))
        );
        assert_eq!(
            ZigbeeConnectivity::bridge_resource_link(bridge),
            RType::ZigbeeConnectivity.link_to(uuid!("6ef36268-e134-5378-a0a0-218ba71dbf0e"))
        );
        assert_eq!(
            GroupedLight::bridge_home_resource_link(home),
            RType::GroupedLight.link_to(uuid!("ced15305-322e-5306-a292-a7cb66f7dc8a"))
        );
    }

    #[test]
    fn zigbee_resource_links_preserve_stable_ids() {
        let address = 0x0200_0000_0000_0001;
        assert_eq!(
            Device::zigbee_resource_link(address),
            RType::Device.link_to(uuid!("d17a66ce-6d22-5694-8aaa-a41fb736cc1a"))
        );
        assert_eq!(
            Light::resource_link(address),
            RType::Light.link_to(uuid!("bc09377d-6daf-57b5-8ca9-7e0b3a11de90"))
        );
        assert_eq!(
            Entertainment::zigbee_resource_link(address),
            RType::Entertainment.link_to(uuid!("bb4650d2-3777-5153-be4d-1fa578abcd41"))
        );
        assert_eq!(
            Taurus::resource_link(address),
            RType::Taurus.link_to(uuid!("250b35ab-ac8d-50fb-b4a2-08dd489b28a7"))
        );
        assert_eq!(
            ZigbeeConnectivity::zigbee_resource_link(address),
            RType::ZigbeeConnectivity.link_to(uuid!("16bea22e-a1db-5c06-89df-ac7caa767213"))
        );
        assert_eq!(
            Button::resource_link(address, "on"),
            RType::Button.link_to(uuid!("4096c255-a79e-5365-884d-f427840148da"))
        );
    }

    #[test]
    fn group_resource_links_preserve_stable_ids() {
        let grouped_light = uuid!("86191eb2-b8b4-5e5a-a6be-8656531e84f0");
        let room = uuid!("cd560bfe-d970-5a89-82a8-77ec0f2a4e27");
        assert_eq!(
            GroupedLight::resource_link("test", 2),
            RType::GroupedLight.link_to(grouped_light)
        );
        assert_eq!(
            Room::resource_link(grouped_light),
            RType::Room.link_to(room)
        );
        assert_eq!(
            Zone::resource_link(grouped_light),
            RType::Zone.link_to(uuid!("2184ffaf-f16d-5866-9712-727fa80a9761"))
        );
        assert_eq!(
            Scene::resource_link(room, 1),
            RType::Scene.link_to(uuid!("cb335f3b-2e5f-5a24-abcb-be6397fe30b5"))
        );
    }

    #[test]
    fn legacy_room_resource_links_preserve_ids() {
        let room = uuid!("f53c2dfd-0e0d-5e61-a6b1-94b4510d0ada");
        assert_eq!(
            Room::legacy_resource_link("kitchen"),
            RType::Room.link_to(room)
        );
        assert_eq!(
            GroupedLight::legacy_resource_link(room, 2),
            RType::GroupedLight.link_to(uuid!("0236cf62-ed50-5ed3-b638-c93c970fd410"))
        );
    }

    #[test]
    fn rlink_hash_uses_input() {
        let a = RType::Room.deterministic("foo");
        let b = RType::Room.deterministic("bar");

        // these must be different - otherwise we forgot to use input
        assert_ne!(a, b);
    }

    #[test]
    fn rlink_hash_uses_rtype() {
        let a = RType::Room.deterministic("foo");
        let b = RType::Scene.deterministic("foo");

        // these must be different - otherwise we forgot to use type
        assert_ne!(a, b);
    }

    macro_rules! assert_hash {
        ($rtype:path, $uuid:expr) => {
            assert_eq!($rtype.deterministic("foo").rid, uuid!($uuid));
        };
    }

    #[test]
    fn rlink_hash_deterministic() {
        assert_hash!(RType::AuthV1, "9c9dc594-12c4-5db8-bc01-3bd26c09cf0f");
        assert_hash!(RType::Device, "fa83ad4c-fbd8-519c-b543-d7aaf2041c75");
        assert_hash!(RType::Light, "020d5289-53f8-5051-ac97-7ea60043223e");
        assert_hash!(RType::Room, "03585677-7f50-5379-b7a6-8c4d70d63c67");
        assert_hash!(RType::GroupedLight, "b2126c4a-16e3-59f4-b11f-4c674c9130f5");
        assert_hash!(RType::Scene, "02808610-c1ec-5774-8eaf-453b83cf1981");
        assert_hash!(RType::Zone, "1cc85d96-7bb6-5e75-938c-df4207136480");
    }
}
