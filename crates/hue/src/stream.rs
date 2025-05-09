use packed_struct::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{HueError, HueResult};
use crate::xy::XY;

#[derive(PrimitiveEnum_u8, Clone, Debug, Copy, PartialEq, Eq)]
pub enum HueStreamColorMode {
    Rgb = 0x00,
    Xy = 0x01,
}

#[derive(PackedStruct, Clone, Debug)]
#[packed_struct(size = "52", endian = "msb")]
pub struct HueStreamHeader {
    magic: [u8; 9],
    version: u16,
    seqnr: u8,
    x0: u16,
    #[packed_field(size_bytes = "1", ty = "enum")]
    color_mode: HueStreamColorMode,
    x1: u8,
    dest: [u8; 36],
}

#[derive(Clone, Debug)]
pub struct HueStreamPacketHeader {
    pub color_mode: HueStreamColorMode,
    pub area: Uuid,
}

impl HueStreamPacketHeader {
    pub const MAGIC: &[u8] = b"HueStream";
    pub const SIZE: usize = size_of::<<HueStreamHeader as PackedStruct>::ByteArray>();

    pub fn parse(data: &[u8]) -> HueResult<Self> {
        let len = Self::SIZE;
        if data.len() < len {
            return Err(HueError::HueEntertainmentBadHeader);
        }

        let hdr = HueStreamHeader::unpack_from_slice(&data[..len])?;

        if hdr.magic != Self::MAGIC {
            return Err(HueError::HueEntertainmentBadHeader);
        }

        let dest = Uuid::try_parse_ascii(&hdr.dest)?;

        Ok(Self {
            color_mode: hdr.color_mode,
            area: dest,
        })
    }
}

#[derive(Clone, Debug)]
pub struct HueStreamPacket {
    pub color_mode: HueStreamColorMode,
    pub area: Uuid,
    pub lights: HueStreamLights,
}

impl HueStreamPacket {
    pub const HEADER_SIZE: usize = HueStreamPacketHeader::SIZE;

    #[must_use]
    pub const fn size_with_lights(nlights: usize) -> usize {
        Self::HEADER_SIZE + nlights * 7
    }

    pub fn parse(data: &[u8]) -> HueResult<Self> {
        let (header, body) = data.split_at(Self::HEADER_SIZE);
        let hdr = HueStreamPacketHeader::parse(header)?;
        let lights = HueStreamLights::parse(hdr.color_mode, body)?;

        Ok(Self {
            color_mode: hdr.color_mode,
            area: hdr.area,
            lights,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HueStreamLights {
    Rgb(Vec<Rgb16>),
    Xy(Vec<Xy16>),
}

impl HueStreamLights {
    pub fn parse(color_mode: HueStreamColorMode, data: &[u8]) -> HueResult<Self> {
        let res = match color_mode {
            HueStreamColorMode::Rgb => Self::Rgb(
                data.chunks_exact(7)
                    .map(Rgb16::unpack_from_slice)
                    .collect::<Result<_, _>>()?,
            ),
            HueStreamColorMode::Xy => Self::Xy(
                data.chunks_exact(7)
                    .map(Xy16::unpack_from_slice)
                    .collect::<Result<_, _>>()?,
            ),
        };

        Ok(res)
    }
}

#[derive(PackedStruct, Clone, Debug, Copy, Serialize, Deserialize)]
#[packed_struct(size = "7", endian = "msb")]
pub struct Rgb16 {
    pub channel: u8,
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

impl Rgb16 {
    #[must_use]
    pub fn to_xy(&self) -> (XY, f64) {
        XY::from_rgb(
            (self.r / 256) as u8,
            (self.g / 256) as u8,
            (self.b / 256) as u8,
        )
    }
}

#[derive(PackedStruct, Clone, Debug, Copy, Serialize, Deserialize)]
#[packed_struct(size = "7", endian = "msb")]
pub struct Xy16 {
    pub channel: u8,
    pub x: u16,
    pub y: u16,
    pub b: u16,
}

impl Xy16 {
    #[must_use]
    pub fn to_xy(&self) -> (XY, f64) {
        (
            XY::new(
                f64::from(self.x) / f64::from(0xFFFF),
                f64::from(self.y) / f64::from(0xFFFF),
            ),
            f64::from(self.b) / f64::from(0x101),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        stream::{Rgb16, Xy16},
        xy::XY,
    };

    macro_rules! compare_float {
        ($expr:expr, $value:expr, $diff:expr) => {
            let a = $expr;
            let b = $value;
            eprintln!("{a} vs {b:.4}");
            assert!((a - b).abs() < $diff);
        };
    }

    macro_rules! compare {
        ($expr:expr, $value:expr) => {
            compare_float!($expr, $value, 1e-5)
        };
    }

    #[test]
    fn rgb16_to_xy() {
        let rgb16 = Rgb16 {
            channel: 1,
            r: 0xFFFF,
            g: 0xFFFF,
            b: 0xFFFF,
        };

        let (xy, b) = rgb16.to_xy();

        compare!(xy.x, XY::D50_WHITE_POINT.x);
        compare!(xy.y, XY::D50_WHITE_POINT.y);
        compare_float!(b, 255.0, 1e-2);
    }

    #[test]
    fn xy16_to_xy() {
        let xy16 = Xy16 {
            channel: 1,
            x: 0x8000,
            y: 0xFFFF,
            b: 0xFFFF,
        };

        let (xy, b) = xy16.to_xy();

        compare!(xy.x, 0.5);
        compare!(xy.y, 1.0);
        compare!(b, 255.0);
    }
}
