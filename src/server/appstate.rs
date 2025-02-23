use std::collections::HashMap;
use std::fs::{self, File};
use std::sync::Arc;

use camino::Utf8Path;
use chrono::Utc;
use tokio::sync::Mutex;

use crate::config::AppConfig;
use crate::error::ApiResult;
use crate::hue;
use crate::hue::legacy_api::{ApiConfig, ApiShortConfig, Whitelist};
use crate::model::state::{State, StateVersion};
use crate::resource::Resources;
use crate::server::certificate;
use crate::server::updater::VersionUpdater;

#[derive(Clone)]
pub struct AppState {
    conf: Arc<AppConfig>,
    upd: Arc<Mutex<VersionUpdater>>,
    pub res: Arc<Mutex<Resources>>,
}

impl AppState {
    pub async fn from_config(config: AppConfig) -> ApiResult<Self> {
        let certfile = &config.bifrost.cert_file;

        let certpath = Utf8Path::new(certfile);
        if certpath.is_file() {
            certificate::check_certificate(certpath, config.bridge.mac)?;
        } else {
            log::warn!("Missing certificate file [{certfile}], generating..");
            certificate::generate_and_save(certpath, config.bridge.mac)?;
        }

        let mut res;
        let upd = Arc::new(Mutex::new(VersionUpdater::new()));
        let swversion = upd.lock().await.get().await.clone();

        if let Ok(fd) = File::open(&config.bifrost.state_file) {
            log::debug!("Existing state file found, loading..");
            let yaml = serde_yml::from_reader(fd)?;
            let state = match State::version(&yaml)? {
                StateVersion::V0 => {
                    log::info!("Detected state file version 0. Upgrading to new version..");
                    let backup_path = &config.bifrost.state_file.with_extension("v0.bak");
                    fs::rename(&config.bifrost.state_file, backup_path)?;
                    log::info!("  ..saved old state file as {backup_path}");
                    State::from_v0(yaml)?
                }
                StateVersion::V1 => {
                    log::info!("Detected state file version 1. Loading..");
                    State::from_v1(yaml)?
                }
            };
            res = Resources::new(swversion, state);
        } else {
            log::debug!("No state file found, initializing..");
            res = Resources::new(swversion, State::new());
            res.init(&hue::bridge_id(config.bridge.mac))?;
        }

        res.reset_all_streaming()?;

        let conf = Arc::new(config);
        let res = Arc::new(Mutex::new(res));

        Ok(Self { conf, upd, res })
    }

    #[must_use]
    pub fn config(&self) -> Arc<AppConfig> {
        self.conf.clone()
    }

    #[must_use]
    pub fn updater(&self) -> Arc<Mutex<VersionUpdater>> {
        self.upd.clone()
    }

    #[must_use]
    pub async fn api_short_config(&self) -> ApiShortConfig {
        let mac = self.conf.bridge.mac;
        ApiShortConfig::from_mac_and_version(mac, self.upd.lock().await.get().await)
    }

    #[must_use]
    pub async fn api_config(&self, username: String) -> ApiConfig {
        ApiConfig {
            short_config: self.api_short_config().await,
            ipaddress: self.conf.bridge.ipaddress,
            netmask: self.conf.bridge.netmask,
            gateway: self.conf.bridge.gateway,
            timezone: self.conf.bridge.timezone.clone(),
            whitelist: HashMap::from([(
                username,
                Whitelist {
                    create_date: Utc::now(),
                    last_use_date: Utc::now(),
                    name: "User#foo".to_string(),
                },
            )]),
            ..ApiConfig::default()
        }
    }
}
