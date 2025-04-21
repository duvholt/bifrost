use std::{collections::HashMap, net::Ipv4Addr};

use camino::{Utf8Path, Utf8PathBuf};
use config::{Config, ConfigError};
use mac_address::MacAddress;
use serde::{Deserialize, Serialize};
use url::Url;

use hue::api::RoomArchetype;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub name: String,
    pub mac: MacAddress,
    pub ipaddress: Ipv4Addr,
    pub http_port: u16,
    pub https_port: u16,
    pub entm_port: u16,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub timezone: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BifrostConfig {
    pub state_file: Utf8PathBuf,
    pub cert_file: Utf8PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Z2mConfig {
    #[serde(flatten)]
    pub servers: HashMap<String, Z2mServer>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Z2mServer {
    pub url: Url,
    pub group_prefix: Option<String>,
    pub disable_tls_verify: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RoomConfig {
    pub name: Option<String>,
    pub icon: Option<RoomArchetype>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub bridge: BridgeConfig,
    pub z2m: Z2mConfig,
    pub bifrost: BifrostConfig,
    #[serde(default)]
    pub rooms: HashMap<String, RoomConfig>,
}

impl Z2mServer {
    #[must_use]
    pub fn get_url(&self) -> Url {
        let mut url = self.url.clone();
        // z2m version 1.x allows both / and /api as endpoints for the
        // websocket, but version 2.x only allows /api. By adding /api (if
        // missing), we ensure compatibility with both versions.
        if !url.path().ends_with("/api") {
            if let Ok(mut path) = url.path_segments_mut() {
                path.push("api");
            }
        }

        // z2m version 2.x requires an auth token on the websocket. If one is
        // not specified in the z2m configuration, the literal string
        // `your-secret-token` is used!
        //
        // To be compatible, we mirror this behavior here. If "token" is set
        // manually by the user, we do nothing.
        if !url.query_pairs().any(|(key, _)| key == "token") {
            url.query_pairs_mut()
                .append_pair("token", "your-secret-token");
        }

        url
    }

    #[must_use]
    #[allow(clippy::option_if_let_else)]
    fn sanitize_url(url: &str) -> String {
        match url.find("token=") {
            Some(offset) => {
                let token = &url[offset + "token=".len()..];
                if token == "your-secret-token" {
                    // this is the standard "blank" token, it's safe to show
                    url.to_string()
                } else {
                    // this is an actual secret token, blank it out with a
                    // standard-length placeholder.
                    format!("{}token={}", &url[..offset], "<<REDACTED>>")
                }
            }
            None => url.to_string(),
        }
    }

    #[must_use]
    pub fn get_sanitized_url(&self) -> String {
        Self::sanitize_url(self.get_url().as_str())
    }
}

pub fn parse(filename: &Utf8Path) -> Result<AppConfig, ConfigError> {
    let settings = Config::builder()
        .set_default("bifrost.state_file", "state.yaml")?
        .set_default("bifrost.cert_file", "cert.pem")?
        .set_default("bridge.http_port", 80)?
        .set_default("bridge.https_port", 443)?
        .set_default("bridge.entm_port", 2100)?
        .add_source(config::File::with_name(filename.as_str()))
        .build()?;

    settings.try_deserialize()
}
