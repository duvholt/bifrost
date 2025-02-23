use serde_json::json;

use crate::z2m::api::RawMessage;

pub const PHILIPS_HUE_ZIGBEE_VENDOR_ID: u16 = 0x100B;

/// Custom, experimental extension for `Zigbee2MQTT`, which allows entirely
/// free-form zigbee messages to be sent.
///
/// NOTE: The generated z2m payload only works on patches z2m instances.
///
/// A REGULAR INSTALL OF Z2M WILL NOT WORK.
#[must_use]
pub fn hue_zclcommand(topic: &str, cluster: &str, command: u8, data: &[u8]) -> RawMessage {
    RawMessage {
        topic: topic.to_string(),
        payload: json!({
            "zclcommand2": {
                "cluster": cluster,
                "command": command,
                "payload": {
                    "data": data,
                },
                "options": {
                    "manufacturerCode": PHILIPS_HUE_ZIGBEE_VENDOR_ID,
                    "disableDefaultResponse": true,
                    "direction": 0,
                    "srcEndpoint": 64,
                    "timeout": 100.0
                }
            }
        }),
    }
}
