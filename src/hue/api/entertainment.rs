use serde::{Deserialize, Serialize};

use crate::hue::api::ResourceLink;

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
pub struct EntertainmentConfiguration {
    pub name: String,
    pub configuration_type: String,
    pub metadata: EntertainmentConfigurationMetadata,
    pub status: String,
    pub stream_proxy: EntertainmentConfigurationStreamProxy,
    pub locations: EntertainmentConfigurationLocations,
    pub light_services: Vec<ResourceLink>,
    pub channels: Vec<EntertainmentConfigurationChannels>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_streamer: Option<ResourceLink>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationMetadata {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationStreamProxy {
    pub mode: String,
    pub node: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationLocations {
    pub service_locations: Vec<EntertainmentConfigurationServiceLocations>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationServiceLocations {
    pub equalization_factor: f64,
    pub position: EntertainmentConfigurationPosition,
    pub positions: Vec<EntertainmentConfigurationPosition>,
    pub service: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationChannels {
    pub channel_id: u32,
    pub position: EntertainmentConfigurationPosition,
    pub members: Vec<EntertainmentConfigurationStreamMembers>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EntertainmentConfigurationPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationStreamMembers {
    pub service: ResourceLink,
    pub index: u32,
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
