use std::collections::BTreeMap;

use hue::zigbee::{EntertainmentZigbeeStream, LightRecordMode};

use crate::backend::z2m::stream::Z2mTarget;

pub struct EntStream {
    pub stream: EntertainmentZigbeeStream,
    pub target: Z2mTarget,
    pub addrs: BTreeMap<String, Vec<u16>>,
    pub modes: Vec<(u16, LightRecordMode)>,
}
