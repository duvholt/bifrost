use std::fmt::Debug;

use packed_struct::prelude::*;

use crate::xy::XY;

use crate::error::{HueError, HueResult};

#[derive(PackedStruct, Debug, Clone, Copy)]
#[packed_struct(size = "6", endian = "lsb")]
pub struct HueEntStop {
    pub x0: u8,
    pub x1: u8,
    pub counter: u32,
}

#[derive(Debug, Clone)]
pub struct HueEntStart {
    pub count: u16,
    pub members: Vec<u16>,
}

#[derive(PackedStruct, Debug, Clone)]
#[packed_struct(size = "6", endian = "lsb")]
pub struct HueEntFrameHeader {
    pub counter: u32,
    pub x0: u16,
}

#[derive(Debug, Clone)]
pub struct HueEntFrame {
    pub counter: u32,
    pub x0: u16,
    pub blks: Vec<HueEntFrameLight>,
}

#[derive(PackedStruct, Clone)]
#[packed_struct(size = "5", endian = "lsb")]
pub struct HueEntFrameLight {
    pub addr: u16,
    pub b: u16,
    pub raw: [u8; 3],
}

impl Debug for HueEntFrameLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let xy = XY::from_quant(self.raw);

        write!(
            f,
            "<{:04x}> ({:.3?},{:.3?})@{:04x?}",
            self.addr, xy.x, xy.y, self.b
        )
    }
}

impl HueEntStart {
    pub fn parse(data: &[u8]) -> HueResult<Self> {
        if data.len() < 2 {
            return Err(HueError::PackedStructError(PackingError::InvalidValue));
        }
        let (hdr, mut data) = data.split_at(2);
        let count = u16::from_be_bytes([hdr[0], hdr[1]]);
        if (count as usize * 2) != data.len() {
            return Err(HueError::PackedStructError(PackingError::InvalidValue));
        }

        let mut members = vec![];
        while !data.is_empty() {
            members.push(u16::from_le_bytes([data[0], data[1]]));
            data = &data[2..];
        }

        debug_assert!(data.is_empty());

        Ok(Self { count, members })
    }
}

impl HueEntFrame {
    pub fn parse(data: &[u8]) -> HueResult<Self> {
        if data.len() < (8 + 5) {
            return Err(HueError::PackedStructError(PackingError::InvalidValue));
        }
        let (hdr, mut data) = data.split_at(6);
        let hdr = HueEntFrameHeader::unpack_from_slice(hdr)?;

        let mut blks: Vec<HueEntFrameLight> = vec![];

        while !data.is_empty() {
            blks.push(HueEntFrameLight::unpack_from_slice(&data[..7])?);
            data = &data[7..];
        }

        Ok(Self {
            counter: hdr.counter,
            x0: hdr.x0,
            blks,
        })
    }
}
