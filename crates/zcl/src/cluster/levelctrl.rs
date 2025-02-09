use crate::frame::{ZclFrame, ZclFrameDirection};

pub fn describe(frame: &ZclFrame, _data: &[u8]) -> Option<String> {
    if frame.manufacturer_specific() {
        return None;
    }

    if frame.flags.direction != ZclFrameDirection::ClientToServer {
        return None;
    }

    match frame.cmd {
        0x00 => Some("MoveToLevel".to_string()),
        _ => None,
    }
}
