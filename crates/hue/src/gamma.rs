// This module is heavily inspired by MIT-licensed code found here:
//
//   https://viereck.ch/hue-xy-rgb/
//
// Original code by Thomas Lochmatter

pub struct GammaCorrection {
    gamma: f64,
    transition: f64,
    slope: f64,
    offset: f64,
}

impl GammaCorrection {
    #[must_use]
    pub const fn new(gamma: f64, transition: f64, slope: f64, offset: f64) -> Self {
        Self {
            gamma,
            transition,
            slope,
            offset,
        }
    }

    #[must_use]
    pub fn transform(&self, value: f64) -> f64 {
        if value <= self.transition {
            self.slope * value
        } else {
            (1.0 + self.offset).mul_add(value.powf(self.gamma), -self.offset)
        }
    }

    #[must_use]
    pub fn inverse(&self, value: f64) -> f64 {
        if value <= self.transform(self.transition) {
            value / self.slope
        } else {
            ((value + self.offset) / (1.0 + self.offset)).powf(1.0 / self.gamma)
        }
    }
}

impl Default for GammaCorrection {
    fn default() -> Self {
        Self::NONE
    }
}

impl GammaCorrection {
    /// Identity mapping ("f(x) -> x"), i.e. no gamma correction
    pub const NONE: Self = Self {
        gamma: 1.0,
        transition: 0.0,
        slope: 1.0,
        offset: 0.0,
    };

    /// Standard gamma correction for sRGB color space
    pub const SRGB: Self = Self::new(0.42, 0.003_130_8, 12.92, 0.055);
}
