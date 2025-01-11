use mac_address::MacAddress;

pub mod api;
pub mod date_format;
pub mod event;
pub mod legacy_api;
pub mod scene_icons;

pub const HUE_BRIDGE_V2_MODEL_ID: &str = "BSB002";

#[must_use]
pub fn best_guess_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "none".to_string())
}

#[must_use]
pub fn bridge_id_raw(mac: MacAddress) -> [u8; 8] {
    let b = mac.bytes();
    [b[0], b[1], b[2], 0xFF, 0xFE, b[3], b[4], b[5]]
}

#[must_use]
#[allow(clippy::format_collect)]
pub fn bridge_id(mac: MacAddress) -> String {
    let bytes = bridge_id_raw(mac);
    bytes
        .into_iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}
