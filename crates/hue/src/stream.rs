use packed_struct::prelude::*;
use packed_struct::types::bits::ByteArray;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{HueError, HueResult};
use crate::xy::XY;

#[derive(PrimitiveEnum_u8, Clone, Debug, Copy, PartialEq, Eq)]
pub enum HueStreamColorMode {
    Rgb = 0x00,
    Xy = 0x01,
}

#[derive(PrimitiveEnum_u8, Clone, Debug, Copy, PartialEq, Eq)]
pub enum HueStreamVersion {
    V1 = 0x01,
    V2 = 0x02,
}

#[derive(PackedStruct, Clone, Debug)]
#[packed_struct(size = "16", endian = "msb")]
pub struct HueStreamHeader {
    magic: [u8; 9],
    #[packed_field(ty = "enum", size_bytes = "1")]
    version: HueStreamVersion,
    x0: u8,
    seqnr: u8,
    x1: u16,
    #[packed_field(size_bytes = "1", ty = "enum")]
    color_mode: HueStreamColorMode,
    x2: u8,
}

impl HueStreamHeader {
    pub const MAGIC: &[u8] = b"HueStream";
    pub const SIZE: usize = size_of::<<Self as PackedStruct>::ByteArray>();

    pub fn parse(data: &[u8]) -> HueResult<Self> {
        if data.len() < Self::SIZE {
            return Err(HueError::HueEntertainmentBadHeader);
        }

        let hdr = Self::unpack_from_slice(&data[..Self::SIZE])?;

        if hdr.magic != Self::MAGIC {
            return Err(HueError::HueEntertainmentBadHeader);
        }

        Ok(hdr)
    }
}

#[derive(Clone, Debug)]
pub enum HueStreamPacket {
    V1(HueStreamPacketV1),
    V2(HueStreamPacketV2),
}

#[derive(Clone, Debug)]
pub struct HueStreamPacketV1 {
    pub lights: HueStreamLightsV1,
}

impl HueStreamPacketV1 {
    #[must_use]
    pub const fn color_mode(&self) -> HueStreamColorMode {
        match self.lights {
            HueStreamLightsV1::Rgb(_) => HueStreamColorMode::Rgb,
            HueStreamLightsV1::Xy(_) => HueStreamColorMode::Xy,
        }
    }

    #[must_use]
    pub fn light_ids(&self) -> Vec<u32> {
        match &self.lights {
            HueStreamLightsV1::Rgb(rgb) => rgb.iter().map(|light| light.light_id).collect(),
            HueStreamLightsV1::Xy(xy) => xy.iter().map(|light| light.light_id).collect(),
        }
    }
}

impl HueStreamPacketV2 {
    #[must_use]
    pub const fn color_mode(&self) -> HueStreamColorMode {
        match self.lights {
            HueStreamLightsV2::Rgb(_) => HueStreamColorMode::Rgb,
            HueStreamLightsV2::Xy(_) => HueStreamColorMode::Xy,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HueStreamPacketV2 {
    pub area: Uuid,
    pub lights: HueStreamLightsV2,
}

impl HueStreamPacket {
    /// Size of uuid in printed ("dashed") form
    const ASCII_UUID_SIZE: usize = 36;

    pub fn parse(data: &[u8]) -> HueResult<Self> {
        let (header, body) = data.split_at(HueStreamHeader::SIZE);
        let hdr = HueStreamHeader::parse(header)?;
        match hdr.version {
            HueStreamVersion::V1 => {
                let lights = HueStreamLightsV1::parse(hdr.color_mode, body)?;
                Ok(Self::V1(HueStreamPacketV1 { lights }))
            }
            HueStreamVersion::V2 => {
                let (area_bytes, body) = body.split_at(Self::ASCII_UUID_SIZE);
                let area = Uuid::try_parse_ascii(area_bytes)?;
                let lights = HueStreamLightsV2::parse(hdr.color_mode, body)?;
                Ok(Self::V2(HueStreamPacketV2 { area, lights }))
            }
        }
    }

    #[must_use]
    pub const fn color_mode(&self) -> HueStreamColorMode {
        match self {
            Self::V1(v1) => v1.color_mode(),
            Self::V2(v2) => v2.color_mode(),
        }
    }
}

#[derive(PackedStruct, Clone, Debug, Copy, Serialize, Deserialize)]
#[packed_struct(size = "9", endian = "msb")]
pub struct Rgb16V1 {
    #[packed_field(size_bytes = "3")]
    pub light_id: u32,
    #[packed_field(size_bytes = "6")]
    pub rgb: Rgb16,
}

#[derive(PackedStruct, Clone, Debug, Copy, Serialize, Deserialize)]
#[packed_struct(size = "9", endian = "msb")]
pub struct Xy16V1 {
    #[packed_field(size_bytes = "3")]
    pub light_id: u32,
    #[packed_field(size_bytes = "6")]
    pub xy: Xy16,
}

#[derive(PackedStruct, Clone, Debug, Copy, Serialize, Deserialize)]
#[packed_struct(size = "7", endian = "msb")]
pub struct Rgb16V2 {
    pub channel: u8,
    #[packed_field(size_bytes = "6")]
    pub rgb: Rgb16,
}

#[derive(PackedStruct, Clone, Debug, Copy, Serialize, Deserialize)]
#[packed_struct(size = "7", endian = "msb")]
pub struct Xy16V2 {
    pub channel: u8,
    #[packed_field(size_bytes = "6")]
    pub xy: Xy16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HueStreamLightsV1 {
    Rgb(Vec<Rgb16V1>),
    Xy(Vec<Xy16V1>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HueStreamLightsV2 {
    Rgb(Vec<Rgb16V2>),
    Xy(Vec<Xy16V2>),
}

fn parse_list<T: PackedStruct>(data: &[u8]) -> HueResult<Vec<T>> {
    let res = data
        .chunks_exact(T::ByteArray::len())
        .map(T::unpack_from_slice)
        .collect::<Result<_, _>>()?;

    Ok(res)
}

impl HueStreamLightsV1 {
    pub fn parse(color_mode: HueStreamColorMode, data: &[u8]) -> HueResult<Self> {
        match color_mode {
            HueStreamColorMode::Rgb => Ok(Self::Rgb(parse_list(data)?)),
            HueStreamColorMode::Xy => Ok(Self::Xy(parse_list(data)?)),
        }
    }
}

impl HueStreamLightsV2 {
    pub fn parse(color_mode: HueStreamColorMode, data: &[u8]) -> HueResult<Self> {
        match color_mode {
            HueStreamColorMode::Rgb => Ok(Self::Rgb(parse_list(data)?)),
            HueStreamColorMode::Xy => Ok(Self::Xy(parse_list(data)?)),
        }
    }
}

#[derive(PackedStruct, Clone, Debug, Copy, Serialize, Deserialize)]
#[packed_struct(size = "6", endian = "msb")]
pub struct Rgb16 {
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
#[packed_struct(size = "6", endian = "msb")]
pub struct Xy16 {
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
    use crate::stream::{Rgb16, Xy16};
    use crate::xy::XY;
    use crate::{compare, compare_float, compare_xy};

    #[test]
    fn rgb16_to_xy() {
        let rgb16 = Rgb16 {
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
