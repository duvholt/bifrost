use crate::frame::{ZclFrame, ZclFrameDirection};

pub fn describe(frame: &ZclFrame, _data: &[u8]) -> Option<String> {
    if frame.manufacturer_specific() {
        return None;
    }

    if frame.flags.direction != ZclFrameDirection::ClientToServer {
        return None;
    }

    match frame.cmd {
        0x00 => Some("MoveToHue".to_string()),
        0x01 => Some("MoveHue".to_string()),
        0x02 => Some("StepHue".to_string()),
        0x03 => Some("MoveToSaturation".to_string()),
        0x04 => Some("MoveSaturation".to_string()),
        0x05 => Some("StepSaturation".to_string()),
        0x06 => Some("MoveToHueAndSaturation".to_string()),
        0x07 => Some("MoveToColor".to_string()),
        0x08 => Some("MoveColor".to_string()),
        0x09 => Some("StepColor".to_string()),
        0x0a => Some("MoveToColorTemp".to_string()),
        0x40 => Some("EnhancedMoveToHue".to_string()),
        0x41 => Some("EnhancedMoveHue".to_string()),
        0x42 => Some("EnhancedStepHue".to_string()),
        0x43 => Some("EnhancedMoveToHueAndSaturation".to_string()),
        0x44 => Some("ColorLoopSet".to_string()),
        0x47 => Some("StopMoveStep".to_string()),
        0x4b => Some("MoveColorTemp".to_string()),
        0x4c => Some("StepColorTemp".to_string()),
        _ => None,
    }
}
