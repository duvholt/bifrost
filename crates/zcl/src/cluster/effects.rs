use crate::frame::{ZclFrame, ZclFrameDirection};

pub fn describe(frame: &ZclFrame, _data: &[u8]) -> Option<String> {
    if frame.flags.direction == ZclFrameDirection::ClientToServer {
        match frame.cmd {
            0x40 => Some("Trigger".to_string()),
            _ => None,
        }
    } else {
        None
    }
}
