use serde::{Deserialize, Serialize};

use crate::clamp::Clamp;
use crate::colorspace::{self, ColorSpace};

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct XY {
    pub x: f64,
    pub y: f64,
}

impl XY {
    const COLOR_SPACE: ColorSpace = colorspace::WIDE;

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
