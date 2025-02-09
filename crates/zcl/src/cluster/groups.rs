use crate::frame::{ZclFrame, ZclFrameDirection};

pub fn describe(frame: &ZclFrame, _data: &[u8]) -> Option<String> {
    if frame.flags.direction == ZclFrameDirection::ClientToServer {
        match frame.cmd {
            0x00 => Some("Add".to_string()),
            0x02 => Some("GetMembership".to_string()),
            _ => None,
        }
    } else {
        match frame.cmd {
            0x00 => Some("AddResp".to_string()),
            0x02 => Some("GetMembershipResp".to_string()),
            _ => None,
        }
    }
}
