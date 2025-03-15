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
