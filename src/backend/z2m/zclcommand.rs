use serde_json::json;

use crate::z2m::api::RawMessage;

pub const PHILIPS_HUE_ZIGBEE_VENDOR_ID: u16 = 0x100B;

/// Use the low-level endpoint for `Zigbee2MQTT`, which allows free-form zigbee
/// messages to be sent.
///
/// NOTE: The generated z2m payload only works on z2m instances with
/// Zigbee-Herdsman-Converter version 23.0.0 or greater.
///
/// OLDER VERSIONS WILL NOT WORK.
#[must_use]
pub fn hue_zclcommand(topic: &str, cluster: &str, command: u8, data: &[u8]) -> RawMessage {
    RawMessage {
        topic: topic.to_string(),
        payload: json!({
            "zclcommand": {
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
