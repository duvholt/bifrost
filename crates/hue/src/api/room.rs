use std::collections::BTreeSet;
use std::ops::{AddAssign, Sub};

use serde::{Deserialize, Serialize};

use crate::api::{Group, GroupMetadata, GroupMetadataUpdate, RType, ResourceLink};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Room {
    pub children: BTreeSet<ResourceLink>,
    pub metadata: GroupMetadata,
    #[serde(default)]
    pub services: BTreeSet<ResourceLink>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoomNew {
    pub children: BTreeSet<ResourceLink>,
    pub metadata: GroupMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RoomUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<BTreeSet<ResourceLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<GroupMetadataUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<ResourceLink>>,
}

impl Group for Room {
    fn grouped_light_service(&self) -> Option<&ResourceLink> {
        self.services
            .iter()
            .find(|rl| rl.rtype == RType::GroupedLight)
    }

    fn children(&self) -> impl Iterator<Item = &ResourceLink> {
        self.children.iter()
    }
}

impl RoomUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_metadata(self, metadata: GroupMetadata) -> Self {
        Self {
            metadata: Some(GroupMetadataUpdate {
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

impl AddAssign<&RoomUpdate> for Room {
    fn add_assign(&mut self, rhs: &RoomUpdate) {
        if let Some(md) = &rhs.metadata {
            self.metadata += md;
        }
        if let Some(children) = &rhs.children {
            self.children.clone_from(children);
        }
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
