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
    #[serde(default)]
    pub search_codes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZigbeeDeviceDiscovery {
    pub owner: ResourceLink,
    pub status: ZigbeeDeviceDiscoveryStatus,

    /* FIXME: Needed to import previous state files which lack this field */
    #[serde(default)]
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
