use std::collections::BTreeMap;

use hue::zigbee::{EntertainmentZigbeeStream, LightRecordMode, PHILIPS_HUE_ZIGBEE_VENDOR_ID};
use serde_json::json;
use z2m::request::Z2mRequest;

use crate::backend::z2m::stream::Z2mTarget;

pub struct EntStream {
    pub stream: EntertainmentZigbeeStream,
    pub target: Z2mTarget,
    pub addrs: BTreeMap<String, Vec<u16>>,
    pub modes: Vec<(u16, LightRecordMode)>,
}

impl EntStream {
    #[must_use]
    pub fn new(counter: u32, target: &str, addrs: BTreeMap<String, Vec<u16>>) -> Self {
        let modes = Self::addrs_to_light_modes(&addrs);
        Self {
            stream: EntertainmentZigbeeStream::new(counter),
            target: Z2mTarget::new(target),
            addrs,
            modes,
        }
    }

    #[must_use]
    pub fn addrs_to_light_modes(addrs: &BTreeMap<String, Vec<u16>>) -> Vec<(u16, LightRecordMode)> {
        let mut modes = vec![];

        for segments in addrs.values() {
            let mode = if segments.len() <= 1 {
                LightRecordMode::Device
            } else {
                LightRecordMode::Segment
            };

            for seg in segments {
                modes.push((*seg, mode));
            }
        }

        modes
    }

    #[must_use]
    pub fn z2m_set_entertainment_brightness(brightness: u8) -> Z2mRequest<'static> {
        Z2mRequest::RawWrite(json!({
            "cluster": EntertainmentZigbeeStream::CLUSTER,
            "payload": {
                "5": {
                    "manufacturerCode": PHILIPS_HUE_ZIGBEE_VENDOR_ID,
                    "type": 32,
                    "value": brightness,
                }
            }
        }))
    }
}
