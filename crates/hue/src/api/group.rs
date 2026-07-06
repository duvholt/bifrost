use std::ops::{AddAssign, Sub};

use serde::{Deserialize, Serialize};

use crate::api::ResourceLink;

pub trait Group {
    fn grouped_light_service(&self) -> Option<&ResourceLink>;

    fn children(&self) -> impl Iterator<Item = &ResourceLink>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GroupMetadata {
    pub name: String,
    pub archetype: GroupArchetype,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GroupMetadataUpdate {
    pub name: Option<String>,
    pub archetype: Option<GroupArchetype>,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GroupArchetype {
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

impl GroupMetadata {
    #[must_use]
    pub fn new(archetype: GroupArchetype, name: &str) -> Self {
        Self {
            archetype,
            name: name.to_string(),
        }
    }
}

impl AddAssign<&GroupMetadataUpdate> for GroupMetadata {
    fn add_assign(&mut self, upd: &GroupMetadataUpdate) {
        if let Some(name) = &upd.name {
            self.name.clone_from(name);
        }
        if let Some(archetype) = &upd.archetype {
            self.archetype = *archetype;
        }
    }
}

#[allow(clippy::if_not_else)]
impl Sub<&GroupMetadata> for &GroupMetadata {
    type Output = GroupMetadataUpdate;

    fn sub(self, rhs: &GroupMetadata) -> Self::Output {
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
