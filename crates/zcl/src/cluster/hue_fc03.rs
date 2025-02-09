use hue::zigbee::Flags;

use crate::error::ZclResult;
use crate::frame::ZclFrame;

pub fn describe(frame: &ZclFrame, data: &[u8]) -> ZclResult<Option<String>> {
    if !frame.cluster_specific() {
        return Ok(None);
    }

    match frame.cmd {
        0x00 => {
            let zflags = Flags::from_bits((data[0] as u16) | (data[1] as u16) << 8).unwrap();
            Ok(Some(format!("{:?} {}", zflags, hex::encode(&data[2..]))))
        }
        _ => Ok(None),
    }
}
