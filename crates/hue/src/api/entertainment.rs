use serde::{Deserialize, Serialize};

use crate::api::ResourceLink;

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
