use chrono::{DateTime, Duration, Utc};

use hue::update::{update_url_for_bridge, UpdateEntries, UpdateEntry};
use hue::version::SwVersion;
use hue::HUE_BRIDGE_V2_MODEL_ID;

use crate::error::{ApiError, ApiResult};

pub async fn fetch_updates(since_version: Option<u64>) -> ApiResult<Vec<UpdateEntry>> {
    let url = update_url_for_bridge(HUE_BRIDGE_V2_MODEL_ID, since_version.unwrap_or_default());
    let response: UpdateEntries = reqwest::get(url).await?.json().await?;
    Ok(response.updates)
}

#[derive(Debug)]
pub struct VersionUpdater {
    version: Option<SwVersion>,
    last_fetch: Option<DateTime<Utc>>,
}

impl Default for VersionUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionUpdater {
    const CACHE_TIME: Duration = Duration::hours(24);

    #[must_use]
    pub const fn new() -> Self {
        Self {
            version: None,
            last_fetch: None,
        }
    }

    pub async fn fetch_version(&mut self) -> ApiResult<SwVersion> {
        update::fetch_updates(None)
            .await?
            .into_iter()
            .max_by(|x, y| x.version.cmp(&y.version))
            .map(|max| SwVersion::new(max.version, max.version_name))
            .ok_or(ApiError::NoUpdateInformation)
    }

    pub async fn get(&mut self) -> &SwVersion {
        let expired = self
            .last_fetch
            .map_or(true, |time| (Utc::now() - time) > Self::CACHE_TIME);

        if expired {
            log::debug!("Firmware update information expired. Fetching..");
        }

        if expired || self.version.is_none() {
            let version = match self.fetch_version().await {
                Ok(version) => {
                    log::info!("Detected newest version to be {version:?}");
                    version
                }
                Err(err) => {
                    let version = SwVersion::default();
                    log::error!("Failed to fetch firmware changelog: {err}");
                    log::warn!("Falling back to default version: {version:?}");
                    version
                }
            };

            self.last_fetch = Some(Utc::now());
            self.version = Some(version);
        }

        self.version.as_ref().unwrap()
    }
}
