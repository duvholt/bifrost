use std::{
    collections::BTreeSet,
    ops::{AddAssign, Sub},
};

use serde::{Deserialize, Serialize};

use crate::api::{RType, ResourceLink, RoomMetadata, RoomMetadataUpdate};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Zone {
    pub children: BTreeSet<ResourceLink>,
    pub metadata: RoomMetadata,
    #[serde(default)]
    pub services: BTreeSet<ResourceLink>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZoneNew {
    pub children: BTreeSet<ResourceLink>,
    pub metadata: RoomMetadata,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ZoneUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<BTreeSet<ResourceLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RoomMetadataUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<Vec<ResourceLink>>,
}

impl Zone {
    #[must_use]
    pub fn grouped_light_service(&self) -> Option<&ResourceLink> {
        self.services
            .iter()
            .find(|rl| rl.rtype == RType::GroupedLight)
    }
}

impl ZoneUpdate {
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

impl AddAssign<&ZoneUpdate> for Zone {
    fn add_assign(&mut self, rhs: &ZoneUpdate) {
        if let Some(md) = &rhs.metadata {
            self.metadata += md;
        }
        if let Some(children) = &rhs.children {
            self.children.clone_from(children);
        }
    }
}
#[allow(clippy::if_not_else)]
impl Sub<&Zone> for &Zone {
    type Output = ZoneUpdate;

    fn sub(self, rhs: &Zone) -> Self::Output {
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
