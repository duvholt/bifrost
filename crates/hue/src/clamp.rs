pub trait Clamp {
    fn unit_to_u8_clamped(self) -> u8;
    fn unit_to_u8_clamped_light(self) -> u8;
    fn unit_from_u8(value: u8) -> Self;
}

impl Clamp for f32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unit_to_u8_clamped(self) -> u8 {
        (self * 255.0).round().clamp(0.0, 255.0) as u8
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unit_to_u8_clamped_light(self) -> u8 {
        self.mul_add(253.0, 1.0).round().clamp(1.0, 254.0) as u8
    }

    fn unit_from_u8(value: u8) -> Self {
        Self::from(value) / 255.0
    }
}

impl Clamp for f64 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unit_to_u8_clamped(self) -> u8 {
        (self * 255.0).round().clamp(0.0, 255.0) as u8
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn unit_to_u8_clamped_light(self) -> u8 {
        self.mul_add(253.0, 1.0).round().clamp(1.0, 254.0) as u8
    }

    fn unit_from_u8(value: u8) -> Self {
        Self::from(value) / 255.0
    }
}

#[cfg(test)]
mod tests {
    use crate::clamp::Clamp;
    use crate::{compare, compare_float};

    #[test]
    fn f32_unit_to_u8_clamped() {
        assert_eq!((-1.0f32).unit_to_u8_clamped(), 0x00);
        assert_eq!(0.0f32.unit_to_u8_clamped(), 0x00);
        assert_eq!(0.5f32.unit_to_u8_clamped(), 0x80);
        assert_eq!(1.0f32.unit_to_u8_clamped(), 0xFF);
        assert_eq!(2.0f32.unit_to_u8_clamped(), 0xFF);
    }

    #[test]
    fn f64_unit_to_u8_clamped() {
        assert_eq!((-1.0f64).unit_to_u8_clamped(), 0x00);
        assert_eq!(0.0f64.unit_to_u8_clamped(), 0x00);
        assert_eq!(0.5f64.unit_to_u8_clamped(), 0x80);
        assert_eq!(1.0f64.unit_to_u8_clamped(), 0xFF);
        assert_eq!(2.0f64.unit_to_u8_clamped(), 0xFF);
    }

    #[test]
    fn f32_unit_to_u8_clamped_light() {
        assert_eq!((-1.0f32).unit_to_u8_clamped_light(), 0x01);
        assert_eq!(0.0f32.unit_to_u8_clamped_light(), 0x01);
        assert_eq!(0.5f32.unit_to_u8_clamped_light(), 0x80);
        assert_eq!(1.0f32.unit_to_u8_clamped_light(), 0xFE);
        assert_eq!(2.0f32.unit_to_u8_clamped_light(), 0xFE);
    }

    #[test]
    fn f64_unit_to_u8_clamped_light() {
        assert_eq!((-1.0f64).unit_to_u8_clamped_light(), 0x01);
        assert_eq!(0.0f64.unit_to_u8_clamped_light(), 0x01);
        assert_eq!(0.5f64.unit_to_u8_clamped_light(), 0x80);
        assert_eq!(1.0f64.unit_to_u8_clamped_light(), 0xFE);
        assert_eq!(2.0f64.unit_to_u8_clamped_light(), 0xFE);
    }

    #[test]
    fn f32_unit_from_u8() {
        compare!(f32::unit_from_u8(0x00), 0.0 / 255.0);
        compare!(f32::unit_from_u8(0x01), 1.0 / 255.0);
        compare!(f32::unit_from_u8(0x02), 2.0 / 255.0);
        compare!(f32::unit_from_u8(0xFE), 254.0 / 255.0);
        compare!(f32::unit_from_u8(0xFF), 255.0 / 255.0);
    }

    #[test]
    fn f64_unit_from_u8() {
        compare!(f64::unit_from_u8(0x00), 0.0 / 255.0);
        compare!(f64::unit_from_u8(0x01), 1.0 / 255.0);
        compare!(f64::unit_from_u8(0x02), 2.0 / 255.0);
        compare!(f64::unit_from_u8(0xFE), 254.0 / 255.0);
        compare!(f64::unit_from_u8(0xFF), 255.0 / 255.0);
    }
}
