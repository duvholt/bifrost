use std::collections::BTreeMap;

use serde_json::json;

use hue::stream::HueStreamLights;
use hue::zigbee::{
    EntertainmentZigbeeStream, HueEntFrameLightRecord, LightRecordMode,
    PHILIPS_HUE_ZIGBEE_VENDOR_ID,
};
use z2m::request::Z2mRequest;
use zcl::attr::ZclDataType;

pub struct EntStream {
    pub stream: EntertainmentZigbeeStream,
    pub target: String,
    pub addrs: BTreeMap<String, Vec<u16>>,
    pub modes: Vec<(u16, LightRecordMode)>,
}

impl EntStream {
    #[must_use]
    pub fn new(counter: u32, target: &str, addrs: BTreeMap<String, Vec<u16>>) -> Self {
        let modes = Self::addrs_to_light_modes(&addrs);
        Self {
            stream: EntertainmentZigbeeStream::new(counter),
            target: target.to_string(),
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
        Z2mRequest::Write {
            cluster: EntertainmentZigbeeStream::CLUSTER,
            payload: json!({
                EntertainmentZigbeeStream::CMD_LIGHT_BALANCE.to_string(): {
                    "manufacturerCode": PHILIPS_HUE_ZIGBEE_VENDOR_ID,
                    "type": ZclDataType::ZclU8 as u8,
                    "value": brightness,
                }
            }),
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[must_use]
    pub fn generate_frame(&self, frame: &HueStreamLights) -> Vec<HueEntFrameLightRecord> {
        let mut blks = vec![];
        match frame {
            HueStreamLights::Rgb(rgb) => {
                for light in rgb {
                    let (xy, bright) = light.to_xy();

                    let brightness = (bright / 255.0 * 2047.0).clamp(1.0, 2047.0) as u16;
                    let (chan, mode) = self.modes[light.channel as usize % self.modes.len()];
                    let raw = xy.to_quant();
                    let lrec = HueEntFrameLightRecord::new(chan, brightness, mode, raw);

                    blks.push(lrec);
                }
            }
            HueStreamLights::Xy(xy) => {
                for light in xy {
                    let (xy, bright) = light.to_xy();

                    let brightness = (bright / 255.0 * 2047.0).clamp(1.0, 2047.0) as u16;
                    let (chan, mode) = self.modes[light.channel as usize % self.modes.len()];
                    let raw = xy.to_quant();
                    let lrec = HueEntFrameLightRecord::new(chan, brightness, mode, raw);

                    blks.push(lrec);
                }
            }
        }

        blks
    }
}
