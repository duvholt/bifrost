use serde::{Deserialize, Serialize};

use crate::clamp::Clamp;
use crate::colorspace::{self, ColorSpace};
use crate::hs::HS;
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

    pub const D50_WHITE_POINT: Self = Self {
        x: 0.34567,
        y: 0.35850,
    };

    pub const D65_WHITE_POINT: Self = Self {
        x: 0.31271,
        y: 0.32902,
    };

    #[must_use]
    pub fn from_rgb(red: u8, green: u8, blue: u8) -> (Self, f64) {
        let [r, g, b] = [red, green, blue].map(Clamp::unit_from_u8);
        Self::from_rgb_unit(r, g, b)
    }

    #[allow(clippy::many_single_char_names)]
    #[must_use]
    pub fn from_rgb_unit(r: f64, g: f64, b: f64) -> (Self, f64) {
        let [x, y, b] = Self::COLOR_SPACE.rgb_to_xyy(r, g, b);

        let max_y = Self::COLOR_SPACE.find_maximum_y(x, y);

        if max_y > f64::EPSILON {
            (Self { x, y }, b / max_y * 255.0)
        } else {
            (Self::D65_WHITE_POINT, 0.0)
        }
    }

    #[must_use]
    pub fn from_hs(hs: HS) -> (Self, f64) {
        let lightness: f64 = 0.5;
        Self::from_hsl(hs, lightness)
    }

    #[must_use]
    pub fn from_hsl(hs: HS, lightness: f64) -> (Self, f64) {
        let [r, g, b] = Self::rgb_from_hsl(hs, lightness);
        Self::from_rgb_unit(r, g, b)
    }

    #[must_use]
    pub fn rgb_from_hsl(hs: HS, lightness: f64) -> [f64; 3] {
        let c = (1.0 - (2.0f64.mul_add(lightness, -1.0)).abs()) * hs.sat;
        let h = hs.hue * 6.0;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let m = lightness - c / 2.0;

        if h < 1.0 {
            [m + c, m + x, m]
        } else if h < 2.0 {
            [m + x, m + c, m]
        } else if h < 3.0 {
            [m, m + c, m + x]
        } else if h < 4.0 {
            [m, m + x, m + c]
        } else if h < 5.0 {
            [m + x, m, m + c]
        } else {
            [m + c, m + 0.0, m + x]
        }
    }

    #[must_use]
    pub fn to_rgb(&self, brightness: f64) -> [u8; 3] {
        Self::COLOR_SPACE
            .xy_to_rgb_color(self.x, self.y, brightness)
            .map(Clamp::unit_to_u8_clamped)
    }
}

impl XY {
    #[must_use]
    pub fn from_quant(data: [u8; 3]) -> Self {
        let x0 = u16::from(data[0]) | (u16::from(data[1] & 0x0F) << 8);
        let y0 = (u16::from(data[2]) << 4) | (u16::from(data[1] >> 4));

        let x = f64::from(x0) * WIDE_GAMUT_MAX_X / f64::from(0xFFF);
        let y = f64::from(y0) * WIDE_GAMUT_MAX_Y / f64::from(0xFFF);

        Self { x, y }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[must_use]
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

#[cfg(test)]
mod tests {
    use crate::hs::HS;
    use crate::xy::XY;

    macro_rules! compare {
        ($expr:expr, $value:expr) => {
            let a = $expr;
            let b = $value;
            eprintln!("{a} vs {b:.4}");
            assert!((a - b).abs() < 1e-4);
        };
    }

    macro_rules! compare_rgb {
        ($a:expr, $b:expr) => {{
            eprintln!("Comparing r");
            compare!($a[0], $b[0]);
            eprintln!("Comparing g");
            compare!($a[1], $b[1]);
            eprintln!("Comparing b");
            compare!($a[2], $b[2]);
        }};
    }

    macro_rules! compare_hsl_rgb {
        ($h:expr, $s:expr, $rgb:expr) => {{
            let sat = $s;
            compare_rgb!(XY::rgb_from_hsl(HS { hue: $h, sat }, 0.5), $rgb);
        }};
    }

    #[test]
    fn rgb_from_hsl() {
        const ONE: f64 = 1.0;
        let sat = 1.0;

        compare_hsl_rgb!(0.0 / 3.0, sat, [ONE, 0.0, 0.0]); // red
        compare_hsl_rgb!(0.5 / 3.0, sat, [ONE, ONE, 0.0]); // red-green
        compare_hsl_rgb!(1.0 / 3.0, sat, [0.0, ONE, 0.0]); // green
        compare_hsl_rgb!(1.5 / 3.0, sat, [0.0, ONE, ONE]); // green-blue
        compare_hsl_rgb!(2.0 / 3.0, sat, [0.0, 0.0, ONE]); // blue
        compare_hsl_rgb!(2.5 / 3.0, sat, [ONE, 0.0, ONE]); // blue-red
        compare_hsl_rgb!(3.0 / 3.0, sat, [ONE, 0.0, 0.0]); // red (wrapped around)
    }
}
