use hue::zigbee::ZigbeeMessage;
use serde_json::{Value, json};

/// Use the low-level endpoint for `Zigbee2MQTT`, which allows free-form zigbee
/// messages to be sent.
///
/// NOTE: The generated z2m payload only works on z2m instances with
/// Zigbee-Herdsman-Converter version 23.0.0 or greater.
///
/// This is the case for z2m version 2.1.1 and newer.
///
/// Older versions WILL NOT WORK.
#[must_use]
pub fn hue_zclcommand(msg: &ZigbeeMessage) -> Value {
    json!({
        "zclcommand": {
            "cluster": msg.cluster,
            "command": msg.command,
            "payload": {
                "data": msg.data,
            },
            "frametype": msg.frametype,
            "options": {
                "manufacturerCode": msg.mfc,
                "disableDefaultResponse": msg.ddr,
                "direction": 0,
                "timeout": 100.0,
            },
        }
    })
}
