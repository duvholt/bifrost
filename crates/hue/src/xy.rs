use serde::{Deserialize, Serialize};

use crate::clamp::Clamp;
use crate::colorspace::{self, ColorSpace};
use crate::{WIDE_GAMUT_MAX_X, WIDE_GAMUT_MAX_Y};

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct XY {
    pub x: f64,
    pub y: f64,
}

impl XY {
    pub const COLOR_SPACE: ColorSpace = colorspace::WIDE;

    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub const D65_WHITE_POINT: Self = Self {
        x: 0.31271,
        y: 0.32902,
    };

    #[allow(clippy::many_single_char_names)]
    #[must_use]
    pub fn from_rgb(red: u8, green: u8, blue: u8) -> (Self, f64) {
        let [r, g, b] = [red, green, blue].map(Clamp::unit_from_u8);

        let [x, y, bright] = Self::COLOR_SPACE.rgb_to_xyy(r, g, b);

        (Self { x, y }, bright)
    }

    #[must_use]
    pub fn to_rgb(&self, brightness: f64) -> [u8; 3] {
        Self::COLOR_SPACE
            .xy_to_rgb_color(self.x, self.y, brightness)
            .map(Clamp::unit_to_u8_clamped)
    }
}

impl XY {
    pub fn from_quant(data: [u8; 3]) -> Self {
        let x0 = u16::from(data[0]) | (u16::from(data[1] & 0x0F) << 8);
        let y0 = (u16::from(data[2]) << 4) | (u16::from(data[1] >> 4));

        let x = f64::from(x0) * WIDE_GAMUT_MAX_X / f64::from(0xFFF);
        let y = f64::from(y0) * WIDE_GAMUT_MAX_Y / f64::from(0xFFF);

        Self { x, y }
    }

    pub fn to_quant(&self) -> [u8; 3] {
        let x = (self.x * ((f64::from(0xFFF) / WIDE_GAMUT_MAX_X) + (0.5 / 4095.))) as u16;
        let y = (self.y * ((f64::from(0xFFF) / WIDE_GAMUT_MAX_Y) + (0.5 / 4095.))) as u16;
        debug_assert!(x < 0x1000);
        debug_assert!(y < 0x1000);

        [
            (x & 0xFF) as u8,
            (((x >> 8) & 0x0F) | ((y & 0x0F) << 4)) as u8,
            ((y >> 4) & 0xFF) as u8,
        ]
    }
}

impl From<[f64; 2]> for XY {
    fn from(value: [f64; 2]) -> Self {
        Self {
            x: value[0],
            y: value[1],
        }
    }
}

impl From<XY> for [f64; 2] {
    fn from(value: XY) -> Self {
        [value.x, value.y]
    }
}
