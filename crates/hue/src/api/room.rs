use std::collections::BTreeSet;
use std::ops::{AddAssign, Sub};

use serde::{Deserialize, Serialize};

use crate::api::{RType, ResourceLink};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RoomMetadata {
    pub name: String,
    pub archetype: RoomArchetype,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RoomMetadataUpdate {
    pub name: Option<String>,
    pub archetype: Option<RoomArchetype>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Room {
    pub children: BTreeSet<ResourceLink>,
    pub metadata: RoomMetadata,
    #[serde(default)]
    pub services: BTreeSet<ResourceLink>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RoomUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<BTreeSet<ResourceLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RoomMetadataUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<ResourceLink>>,
}

impl Room {
    #[must_use]
    pub fn grouped_light_service(&self) -> Option<&ResourceLink> {
        self.services
            .iter()
            .find(|rl| rl.rtype == RType::GroupedLight)
    }
}

impl RoomUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_metadata(self, metadata: RoomMetadata) -> Self {
        Self {
            metadata: Some(RoomMetadataUpdate {
                name: Some(metadata.name),
                archetype: Some(metadata.archetype),
            }),
            ..self
        }
    }

    #[must_use]
    pub fn with_children(self, children: BTreeSet<ResourceLink>) -> Self {
        Self {
            children: Some(children),
            ..self
        }
    }
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoomArchetype {
    LivingRoom,
    Kitchen,
    Dining,
    Bedroom,
    KidsBedroom,
    Bathroom,
    Nursery,
    Office,
    GuestRoom,

    Toilet,
    Staircase,
    Hallway,
    LaundryRoom,
    Storage,
    Closet,
    Garage,
    Other,

    Gym,
    Lounge,
    Tv,
    Computer,
    Recreation,
    /// Gaming Room
    ManCave,
    Music,
    /// Library
    Reading,
    Studio,

    /// Backyard
    Garden,
    /// Patio
    Terrace,
    Balcony,
    Driveway,
    Carport,
    FrontDoor,
    Porch,
    Barbecue,
    Pool,

    Downstairs,
    Upstairs,
    TopFloor,
    Attic,
    Home,

}

impl RoomMetadata {
    #[must_use]
    pub fn new(archetype: RoomArchetype, name: &str) -> Self {
        Self {
            archetype,
            name: name.to_string(),
        }
    }
}

impl AddAssign<RoomUpdate> for Room {
    fn add_assign(&mut self, rhs: RoomUpdate) {
        if let Some(md) = rhs.metadata {
            self.metadata += md;
        }
        if let Some(children) = rhs.children {
            self.children = children;
        }
    }
}

impl AddAssign<RoomMetadataUpdate> for RoomMetadata {
    fn add_assign(&mut self, upd: RoomMetadataUpdate) {
        if let Some(name) = upd.name {
            self.name = name;
        }
        if let Some(archetype) = upd.archetype {
            self.archetype = archetype;
        }
    }
}

#[allow(clippy::if_not_else)]
impl Sub<&RoomMetadata> for &RoomMetadata {
    type Output = RoomMetadataUpdate;

    fn sub(self, rhs: &RoomMetadata) -> Self::Output {
        let mut upd = Self::Output::default();

        if self != rhs {
            if self.name != rhs.name {
                upd.name = Some(rhs.name.clone());
            }
            if self.archetype != rhs.archetype {
                upd.archetype = Some(rhs.archetype);
            }
        }

        upd
    }
}

#[allow(clippy::if_not_else)]
impl Sub<&Room> for &Room {
    type Output = RoomUpdate;

    fn sub(self, rhs: &Room) -> Self::Output {
        let mut upd = Self::Output::default();

        if self != rhs {
            if self.children != rhs.children {
                upd.children = Some(rhs.children.clone());
            }
            if self.metadata != rhs.metadata {
                upd.metadata = Some(&self.metadata - &rhs.metadata);
            }
        }

        upd
    }
}
