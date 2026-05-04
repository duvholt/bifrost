use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::api::{RType, ResourceLink};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BridgeHome {
    pub children: BTreeSet<ResourceLink>,
    pub services: BTreeSet<ResourceLink>,
}

impl BridgeHome {
    #[must_use]
    pub fn grouped_light_service(&self) -> Option<&ResourceLink> {
        self.services
            .iter()
            .find(|rl| rl.rtype == RType::GroupedLight)
    }
}
