#![doc = include_str!("../../../doc/hue-zigbee-format.md")]

pub mod api;
pub mod clamp;
pub mod colorspace;
pub mod colortemp;
pub mod date_format;
pub mod devicedb;
pub mod diff;
pub mod error;
pub mod flags;
pub mod gamma;
pub mod hs;
pub mod legacy_api;
pub mod scene_icons;
pub mod stream;
pub mod update;
pub mod version;
pub mod xy;
pub mod zigbee;

#[cfg(feature = "event")]
pub mod event;

#[cfg(feature = "mac")]
use mac_address::MacAddress;

pub const WIDE_GAMUT_MAX_X: f64 = 0.7347;
pub const WIDE_GAMUT_MAX_Y: f64 = 0.8264;

pub const HUE_BRIDGE_V2_MODEL_ID: &str = "BSB002";
pub const HUE_BRIDGE_V2_DEFAULT_SWVERSION: u64 = 1_970_084_010;
pub const HUE_BRIDGE_V2_DEFAULT_APIVERSION: &str = "1.70.0";

#[must_use]
pub fn best_guess_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "none".to_string())
}

#[cfg(feature = "mac")]
#[must_use]
pub fn bridge_id_raw(mac: MacAddress) -> [u8; 8] {
    let b = mac.bytes();
    [b[0], b[1], b[2], 0xFF, 0xFE, b[3], b[4], b[5]]
}

#[cfg(feature = "mac")]
#[must_use]
pub fn bridge_id(mac: MacAddress) -> String {
    hex::encode(bridge_id_raw(mac))
}

#[cfg(test)]
mod tests {
    use crate::version::SwVersion;
    use crate::{HUE_BRIDGE_V2_DEFAULT_APIVERSION, HUE_BRIDGE_V2_DEFAULT_SWVERSION};

    /// verify that `HUE_BRIDGE_V2_DEFAULT_SWVERSION` and
    /// `HUE_BRIDGE_V2_DEFAULT_APIVERSION` are synchronized
    #[test]
    fn test_default_version_match() {
        let ver = SwVersion::new(HUE_BRIDGE_V2_DEFAULT_SWVERSION, String::new());
        assert_eq!(
            HUE_BRIDGE_V2_DEFAULT_APIVERSION,
            ver.get_legacy_apiversion()
        );
    }
}
