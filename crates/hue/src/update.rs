use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::error::ApiResult;
use crate::hue::{date_format, HUE_BRIDGE_V2_MODEL_ID};

// Full request goes to {UPDATE_CHECK_URL}?deviceTypeId=BSB002&version=1
pub const UPDATE_CHECK_URL: &str = "https://firmware.meethue.com/v1/checkupdate";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEntry {
    #[serde(with = "date_format::update_utc")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "date_format::update_utc")]
    pub updated_at: DateTime<Utc>,
    pub file_size: u64,
    pub md5: String,
    pub binary_url: String,
    pub version: u64,
    pub version_name: String,
    pub release_notes: String,
}

#[derive(Deserialize)]
pub struct UpdateEntries {
    pub updates: Vec<UpdateEntry>,
}

#[must_use]
pub fn update_url_for_bridge(device_type_id: &str, version: u64) -> String {
    format!("{UPDATE_CHECK_URL}?deviceTypeId={device_type_id}&version={version}")
}
