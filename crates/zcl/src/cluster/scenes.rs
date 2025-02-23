#![allow(clippy::collapsible_else_if)]

use hue::zigbee::Flags;

use crate::frame::{ZclFrame, ZclFrameDirection};

pub fn describe(frame: &ZclFrame, data: &[u8]) -> Option<String> {
    if frame.manufacturer_specific() {
        if frame.flags.direction == ZclFrameDirection::ClientToServer {
            match frame.cmd {
                0x02 => Some(format!(
                    "SetComposite {:?}",
                    Flags::from_bits((data[3] as u16) | ((data[4] as u16) << 8)).unwrap()
                )),
                _ => None,
            }
        } else {
            match frame.cmd {
                0x02 => Some("SetCompositeOk".to_string()),
                _ => None,
            }
        }
    } else {
        if frame.flags.direction == ZclFrameDirection::ClientToServer {
            match frame.cmd {
                0x02 => Some("Remove".to_string()),
                0x05 => Some("Recall".to_string()),
                0x06 => Some("GetMembership".to_string()),
                _ => None,
            }
        } else {
            match frame.cmd {
                0x06 => Some("GetMembershipResp".to_string()),
                _ => None,
            }
        }
    }
}
