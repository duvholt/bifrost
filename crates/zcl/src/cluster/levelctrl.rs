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
        0x01 => Some("Move".to_string()),
        0x02 => Some("Step".to_string()),
        0x03 => Some("Stop".to_string()),
        0x04 => Some("MoveToLevelWithOnOff".to_string()),
        0x05 => Some("MoveWithOnOff".to_string()),
        0x06 => Some("StepWithOnOff".to_string()),
        0x07 => Some("StopWithOnOff".to_string()),
        _ => None,
    }
}
