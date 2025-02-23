use packed_struct::prelude::*;
use packed_struct::types::bits::ByteArray;
use uuid::Uuid;

use crate::error::HueResult;
use crate::xy::XY;

#[derive(PrimitiveEnum_u8, Clone, Debug, Copy)]
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
pub struct HueStreamPacket {
    pub color_mode: HueStreamColorMode,
    pub area: Uuid,
}

impl HueStreamPacket {
    pub fn parse(data: &[u8]) -> HueResult<Self> {
        let len = <HueStreamHeader as PackedStruct>::ByteArray::len();
        let hdr = HueStreamHeader::unpack_from_slice(&data[..len])?;
        debug_assert_eq!(&hdr.magic, b"HueStream");
        Ok(Self {
            color_mode: hdr.color_mode,
            area: Uuid::try_parse_ascii(&hdr.dest)?,
        })
    }
}

#[derive(PackedStruct, Clone, Debug, Copy)]
#[packed_struct(size = "7", endian = "msb")]
pub struct HueStreamLight {
    pub channel: u8,
    pub r: u16,
    pub g: u16,
    pub b: u16,
}

impl HueStreamLight {
    pub fn to_xy(&self) -> (XY, f64) {
        XY::from_rgb(
            (self.r / 256) as u8,
            (self.g / 256) as u8,
            (self.b / 256) as u8,
        )
    }
}
