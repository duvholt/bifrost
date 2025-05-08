use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::api::ResourceLink;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ZigbeeDeviceDiscoveryStatus {
    Active,
    Ready,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ZigbeeDeviceDiscoveryAction {
    pub action_type_values: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search_codes: Vec<String>,
}

impl ZigbeeDeviceDiscoveryAction {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.action_type_values.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZigbeeDeviceDiscovery {
    pub owner: ResourceLink,
    pub status: ZigbeeDeviceDiscoveryStatus,

    #[serde(default, skip_serializing_if = "ZigbeeDeviceDiscoveryAction::is_empty")]
    pub action: ZigbeeDeviceDiscoveryAction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZigbeeDeviceDiscoveryInstallCode {
    pub mac_address: String,
    pub ic: Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ZigbeeDeviceDiscoveryUpdateActionType {
    Search,
    SearchAllowDefaultLinkKey,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZigbeeDeviceDiscoveryUpdateAction {
    pub action_type: ZigbeeDeviceDiscoveryUpdateActionType,
    pub search_codes: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZigbeeDeviceDiscoveryUpdate {
    pub action: ZigbeeDeviceDiscoveryUpdateAction,
    pub add_install_code: Option<ZigbeeDeviceDiscoveryInstallCode>,
}
