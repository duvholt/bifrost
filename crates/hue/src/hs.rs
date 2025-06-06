use serde::{Deserialize, Serialize};

#[derive(Copy, Debug, Serialize, Deserialize, Clone)]
pub struct HS {
    pub hue: f64,
    pub sat: f64,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct RawHS {
    pub hue: u16,
    pub sat: u8,
}

impl From<RawHS> for HS {
    fn from(raw: RawHS) -> Self {
        Self {
            hue: f64::from(raw.hue) / f64::from(0xFFFF),
            sat: f64::from(raw.sat) / f64::from(0xFF),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::hs::{HS, RawHS};
    use crate::{compare, compare_float, compare_hs};

    #[test]
    fn from_rawhs_min() {
        compare_hs!(
            HS::from(RawHS { hue: 0, sat: 0 }),
            HS { hue: 0.0, sat: 0.0 }
        );
    }

    #[test]
    fn from_rawhs_mid() {
        compare_hs!(
            HS::from(RawHS {
                hue: 0xCCCC,
                sat: 0xCC
            }),
            HS { hue: 0.8, sat: 0.8 }
        );
    }

    #[test]
    fn from_rawhs_max() {
        compare_hs!(
            HS::from(RawHS {
                hue: 0xFFFF,
                sat: 0xFF
            }),
            HS { hue: 1.0, sat: 1.0 }
        );
    }
}
