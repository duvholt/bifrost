pub trait Clamp {
    fn unit_to_u8_clamped(self) -> u8;
    fn unit_from_u8(value: u8) -> Self;
}

impl Clamp for f32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unit_to_u8_clamped(self) -> u8 {
        (self * 255.0).clamp(0.0, 255.0) as u8
    }

    fn unit_from_u8(value: u8) -> Self {
        Self::from(value) / 255.0
    }
}

impl Clamp for f64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unit_to_u8_clamped(self) -> u8 {
        (self * 255.0).clamp(0.0, 255.0) as u8
    }

    fn unit_from_u8(value: u8) -> Self {
        Self::from(value) / 255.0
    }
}
