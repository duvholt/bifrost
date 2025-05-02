use std::collections::BTreeMap;

use hue::zigbee::{EntertainmentZigbeeStream, LightRecordMode};

use crate::backend::z2m::stream::Z2mTarget;

pub struct EntStream {
    pub stream: EntertainmentZigbeeStream,
    pub target: Z2mTarget,
    pub addrs: BTreeMap<String, Vec<u16>>,
    pub modes: Vec<(u16, LightRecordMode)>,
}

impl EntStream {
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
}
