use serde::{Deserialize, Serialize};

use crate::api::{RType, ResourceLink};

impl Entertainment {
    #[must_use]
    pub fn zigbee_resource_link(ieee_address: u64) -> ResourceLink {
        RType::Entertainment.deterministic(ieee_address)
    }

    #[must_use]
    pub fn bridge_resource_link(bridge: uuid::Uuid) -> ResourceLink {
        RType::Entertainment.deterministic(bridge.as_bytes())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Entertainment {
    pub equalizer: bool,
    pub owner: ResourceLink,
    pub proxy: bool,
    pub renderer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_streams: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer_reference: Option<ResourceLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<EntertainmentSegments>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentSegments {
    pub configurable: bool,
    pub max_segments: u32,
    pub segments: Vec<EntertainmentSegment>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentSegment {
    pub length: u32,
    pub start: u32,
}
