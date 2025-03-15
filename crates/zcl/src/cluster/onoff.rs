use crate::frame::{ZclFrame, ZclFrameDirection};

pub fn describe(frame: &ZclFrame, _data: &[u8]) -> Option<String> {
    if frame.manufacturer_specific() {
        return None;
    }

    if frame.flags.direction != ZclFrameDirection::ClientToServer {
        return None;
    }

    match frame.cmd {
        0x00 => Some("Off".to_string()),
        0x01 => Some("On".to_string()),
        0x40 => Some("OffWithEffect".to_string()),
        _ => None,
    }
}
