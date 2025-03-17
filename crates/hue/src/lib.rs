#![doc = include_str!("../../../doc/hue-zigbee-format.md")]

pub mod clamp;
pub mod colorspace;
pub mod error;
pub mod flags;
pub mod gamma;
pub mod stream;
pub mod xy;
pub mod zigbee;

pub const WIDE_GAMUT_MAX_X: f64 = 0.7347;
pub const WIDE_GAMUT_MAX_Y: f64 = 0.8264;
