use serde::{Deserialize, Serialize};

use crate::api::ResourceLink;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfiguration {
    pub name: String,
    pub configuration_type: EntertainmentConfigurationType,
    pub metadata: EntertainmentConfigurationMetadata,
    pub status: EntertainmentConfigurationStatus,
    pub stream_proxy: EntertainmentConfigurationStreamProxy,
    pub locations: EntertainmentConfigurationLocations,
    pub light_services: Vec<ResourceLink>,
    pub channels: Vec<EntertainmentConfigurationChannels>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_streamer: Option<ResourceLink>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntertainmentConfigurationStatus {
    Active,
    Inactive,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationMetadata {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationStreamProxy {
    pub mode: EntertainmentConfigurationStreamProxyMode,
    pub node: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationLocations {
    pub service_locations: Vec<EntertainmentConfigurationServiceLocations>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationServiceLocations {
    pub equalization_factor: f64,
    pub position: Position,
    pub positions: Vec<Position>,
    pub service: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationChannels {
    pub channel_id: u32,
    pub position: Position,
    pub members: Vec<EntertainmentConfigurationStreamMembers>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Position {
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationStreamMembers {
    pub service: ResourceLink,
    pub index: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EntertainmentConfigurationAction {
    Start,
    Stop,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum EntertainmentConfigurationType {
    Screen,
    Monitor,
    Music,
    #[serde(rename = "3dspace")]
    Space3D,
    Other,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationUpdate {
    pub configuration_type: Option<EntertainmentConfigurationType>,
    pub metadata: Option<EntertainmentConfigurationMetadata>,
    pub action: Option<EntertainmentConfigurationAction>,
    pub stream_proxy: Option<EntertainmentConfigurationStreamProxyUpdate>,
    pub locations: Option<EntertainmentConfigurationLocationsUpdate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct EntertainmentConfigurationNew {
    pub configuration_type: EntertainmentConfigurationType,
    pub metadata: EntertainmentConfigurationMetadata,
    pub stream_proxy: Option<EntertainmentConfigurationStreamProxyUpdate>,
    pub locations: EntertainmentConfigurationLocationsNew,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EntertainmentConfigurationStreamProxyMode {
    Auto,
    Manual,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum EntertainmentConfigurationStreamProxyUpdate {
    Auto,
    Manual { node: ResourceLink },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationLocationsUpdate {
    pub service_locations: Vec<EntertainmentConfigurationServiceLocationsUpdate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationLocationsNew {
    pub service_locations: Vec<EntertainmentConfigurationServiceLocationsNew>,
}

impl From<EntertainmentConfigurationServiceLocationsNew>
    for EntertainmentConfigurationServiceLocationsUpdate
{
    fn from(value: EntertainmentConfigurationServiceLocationsNew) -> Self {
        Self {
            equalization_factor: Some(1.0),
            positions: value.positions,
            service: value.service,
        }
    }
}

impl From<EntertainmentConfigurationServiceLocations>
    for EntertainmentConfigurationServiceLocationsUpdate
{
    fn from(value: EntertainmentConfigurationServiceLocations) -> Self {
        Self {
            equalization_factor: Some(value.equalization_factor),
            service: value.service,
            positions: value.positions,
        }
    }
}

impl From<EntertainmentConfigurationServiceLocationsUpdate>
    for EntertainmentConfigurationServiceLocations
{
    fn from(value: EntertainmentConfigurationServiceLocationsUpdate) -> Self {
        Self {
            equalization_factor: value.equalization_factor.unwrap_or(1.0),
            service: value.service,
            position: value.positions.first().cloned().unwrap_or_default(),
            positions: value.positions,
        }
    }
}

impl From<EntertainmentConfigurationServiceLocationsNew>
    for EntertainmentConfigurationServiceLocations
{
    fn from(value: EntertainmentConfigurationServiceLocationsNew) -> Self {
        Self {
            equalization_factor: 1.0,
            service: value.service,
            position: value.positions.first().cloned().unwrap_or_default(),
            positions: value.positions,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationServiceLocationsUpdate {
    pub equalization_factor: Option<f64>,
    pub positions: Vec<Position>,
    pub service: ResourceLink,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntertainmentConfigurationServiceLocationsNew {
    pub positions: Vec<Position>,
    pub service: ResourceLink,
}
