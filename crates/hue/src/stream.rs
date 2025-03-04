use packed_struct::prelude::*;
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

#[derive(Clone, Debug)]
pub enum HueStreamLights {
    Rgb(Vec<Rgb16>),
    Xy(Vec<Xy16>),
}

impl HueStreamLights {
    pub fn parse(color_mode: HueStreamColorMode, data: &[u8]) -> HueResult<Self> {
        let res = match color_mode {
            HueStreamColorMode::Rgb => HueStreamLights::Rgb(
                data.chunks_exact(7)
                    .map(Rgb16::unpack_from_slice)
                    .collect::<Result<_, _>>()?,
            ),
            HueStreamColorMode::Xy => HueStreamLights::Xy(
                data.chunks_exact(7)
                    .map(Xy16::unpack_from_slice)
                    .collect::<Result<_, _>>()?,
            ),
        };

        Ok(res)
    }
}

#[derive(PackedStruct, Clone, Debug, Copy)]
#[packed_struct(size = "7", endian = "msb")]
pub struct Rgb16 {
    pub channel: u8,
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

impl Rgb16 {
    pub fn to_xy(&self) -> (XY, f64) {
        XY::from_rgb(
            (self.r / 256) as u8,
            (self.g / 256) as u8,
            (self.b / 256) as u8,
        )
    }
}

#[derive(PackedStruct, Clone, Debug, Copy)]
#[packed_struct(size = "7", endian = "msb")]
pub struct Xy16 {
    pub channel: u8,
    pub x: u16,
    pub y: u16,
    pub b: u16,
}

impl Xy16 {}
