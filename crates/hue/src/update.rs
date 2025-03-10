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
struct UpdateEntries {
    updates: Vec<UpdateEntry>,
}

#[must_use]
pub fn update_url_for_bridge(device_type_id: &str, version: u64) -> String {
    format!("{UPDATE_CHECK_URL}?deviceTypeId={device_type_id}&version={version}")
}

pub async fn fetch_updates(since_version: Option<u64>) -> ApiResult<Vec<UpdateEntry>> {
    let url = update_url_for_bridge(HUE_BRIDGE_V2_MODEL_ID, since_version.unwrap_or_default());
    let response: UpdateEntries = reqwest::get(url).await?.json().await?;
    Ok(response.updates)
}
