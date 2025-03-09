// This module is heavily inspired by MIT-licensed code found here:
//
//   https://viereck.ch/hue-xy-rgb/
//
// Original code by Thomas Lochmatter

use std::ops::{Index, IndexMut};

use crate::gamma::GammaCorrection;

#[derive(Clone, Debug)]
pub struct Matrix3(pub [f64; 3 * 3]);

impl Matrix3 {
    #[must_use]
    pub const fn identity() -> Self {
        Self([
            1.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, //
            0.0, 0.0, 1.0, //
        ])
    }

    #[must_use]
    pub fn inverted(&self) -> Option<Self> {
        let mut current = self.clone();
        let mut inverse = Self::identity();

        // Gaussian elimination (part 1)
        for i in 0..3 {
            // Get the diagonal term
            let mut d = current[[i, i]];

            // If it is 0, there must be at least one row with a non-zero element (otherwise, the matrix is not invertible)
            if d == 0.0 {
                let mut r = i + 1;

                while r < 3 && (current[[r, i]]).abs() < 1e-10 {
                    r += 1
                }

                if r == 3 {
                    return None;
                } // i is the rank

                for c in 0..3 {
                    current[[i, c]] += current[[r, c]];
                    inverse[[i, c]] += inverse[[r, c]];
                }

                d = current[[i, i]];
            }

            // Divide the row by the diagonal term
            let inv = 1.0 / d;
            for c in 0..3 {
                current[[i, c]] *= inv;
                inverse[[i, c]] *= inv;
            }

            // Divide all subsequent rows with a non-zero coefficient, and subtract the row
            for r in i + 1..3 {
                let p = current.0[r * 3 + i];
                if p != 0.0 {
                    for c in 0..3 {
                        current[[r, c]] -= current[[i, c]] * p;
                        inverse[[r, c]] -= inverse[[i, c]] * p;
                    }
                }
            }
        }

        // Gaussian elimination (part 2)
        for i in (0..3).rev() {
            for r in 0..i {
                let d = current[[r, i]];
                for c in 0..3 {
                    current[[r, c]] -= current[[i, c]] * d;
                    inverse[[r, c]] -= inverse[[i, c]] * d;
                }
            }
        }

        Some(inverse)
    }

    #[allow(clippy::suboptimal_flops)]
    #[must_use]
    fn mult(&self, d: [f64; 3]) -> [f64; 3] {
        let m = self.0;
        let cx = d[0] * m[0] + d[1] * m[1] + d[2] * m[2];
        let cy = d[0] * m[3] + d[1] * m[4] + d[2] * m[5];
        let cz = d[0] * m[6] + d[1] * m[7] + d[2] * m[8];
        [cx, cy, cz]
    }
}

impl Index<[usize; 2]> for Matrix3 {
    type Output = f64;

    fn index(&self, index: [usize; 2]) -> &Self::Output {
        &self.0[index[0] * 3 + index[1]]
    }
}

impl IndexMut<[usize; 2]> for Matrix3 {
    fn index_mut(&mut self, index: [usize; 2]) -> &mut Self::Output {
        &mut self.0[index[0] * 3 + index[1]]
    }
}

pub struct ColorSpace {
    rgb: Matrix3,
    xyz: Matrix3,
    gamma: GammaCorrection,
}

impl ColorSpace {
    #[must_use]
    pub fn xyz_to_rgb(&self, x: f64, y: f64, z: f64) -> [f64; 3] {
        self.rgb.mult([x, y, z]).map(|q| self.gamma.transform(q))
    }

    #[allow(non_snake_case)]
    #[must_use]
    pub fn xyy_to_rgb(&self, x: f64, y: f64, Y: f64) -> [f64; 3] {
        let z = 1.0 - x - y;
        self.xyz_to_rgb((Y / y) * x, Y, (Y / y) * z)
    }

    #[must_use]
    pub fn rgb_to_xyz(&self, r: f64, g: f64, b: f64) -> [f64; 3] {
        self.xyz.mult([r, g, b].map(|q| self.gamma.inverse(q)))
    }

    #[allow(clippy::many_single_char_names)]
    #[must_use]
    pub fn rgb_to_xyy(&self, r: f64, g: f64, b: f64) -> [f64; 3] {
        let [cx, cy, cz] = self.rgb_to_xyz(r, g, b);

        let x = cx / (cx + cy + cz);
        let y = cy / (cx + cy + cz);
        let brightness = cy;

        [x, y, brightness]
    }

    #[allow(clippy::many_single_char_names)]
    #[must_use]
    pub fn find_maximum_y(&self, x: f64, y: f64) -> f64 {
        let mut bri = 1.0;
        for _ in 0..10 {
            let [r, g, b] = self.xyy_to_rgb(x, y, bri);
            let max = r.max(g).max(b);
            bri /= max;
        }

        bri
    }

    #[allow(non_snake_case)]
    #[must_use]
    pub fn xy_to_rgb_color(&self, x: f64, y: f64, brightness: f64) -> [f64; 3] {
        let max_Y = self.find_maximum_y(x, y);
        self.xyy_to_rgb(x, y, max_Y * brightness / 255.0)
    }
}

/// Wide gamut color space
pub const WIDE: ColorSpace = ColorSpace {
    rgb: Matrix3([
        1.4625, -0.1845, -0.2734, //
        -0.5229, 1.4479, 0.0681, //
        0.0346, -0.0958, 1.2875, //
    ]),
    xyz: Matrix3([
        0.7164, 0.1010, 0.1468, //
        0.2587, 0.7247, 0.0166, //
        0.0000, 0.0512, 0.7740, //
    ]),
    gamma: GammaCorrection::NONE,
};

/// sRGB color space
pub const SRGB: ColorSpace = ColorSpace {
    rgb: Matrix3([
        3.2401, -1.5370, -0.4983, //
        -0.9693, 1.8760, 0.0415, //
        0.0558, -0.2040, 1.0572, //
    ]),
    xyz: Matrix3([
        0.4125, 0.3576, 0.1804, //
        0.2127, 0.7152, 0.0722, //
        0.0193, 0.1192, 0.9503, //
    ]),
    gamma: GammaCorrection::SRGB,
};

/// Adobe RGB color space
pub const ADOBE: ColorSpace = ColorSpace {
    rgb: Matrix3([
        2.0416, -0.5652, -0.3447, //
        -0.9695, 1.8763, 0.0415, //
        0.0135, -0.1184, 1.0154, //
    ]),
    xyz: Matrix3([
        0.5767, 0.1856, 0.1882, //
        0.2974, 0.6273, 0.0753, //
        0.0270, 0.0707, 0.9911, //
    ]),
    gamma: GammaCorrection::NONE,
};

#[cfg(test)]
mod tests {
    use std::iter::zip;

    use crate::colorspace::{ColorSpace, ADOBE, SRGB, WIDE};

    macro_rules! compare {
        ($expr:expr, $value:expr) => {
            let a = $expr;
            let b = $value;
            eprintln!("{a} vs {b:.4}");
            assert!((a - b).abs() < 1e-4);
        };
    }

    fn verify_matrix(cs: &ColorSpace) {
        let xyz = &cs.xyz;
        let rgb = &cs.rgb;

        let xyzi = xyz.inverted().unwrap();
        let rgbi = rgb.inverted().unwrap();

        zip(xyz.0, rgbi.0).for_each(|(a, b)| {
            compare!(a, b);
        });

        zip(rgb.0, xyzi.0).for_each(|(a, b)| {
            compare!(a, b);
        });
    }

    #[test]
    fn iverse_wide() {
        verify_matrix(&WIDE);
    }

    #[test]
    fn iverse_srgb() {
        verify_matrix(&SRGB);
    }

    #[test]
    fn iverse_adobe() {
        verify_matrix(&ADOBE);
    }
}
