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
    pub smoothing: u16,
}

#[derive(Debug, Clone)]
pub struct HueEntFrame {
    pub counter: u32,
    pub smoothing: u16,
    pub blks: Vec<HueEntFrameLightRecord>,
}

#[derive(PackedStruct, Clone)]
#[packed_struct(size_bytes = "7", endian = "lsb", bit_numbering = "msb0")]
pub struct HueEntFrameLightRecord {
    #[packed_field(bits = "0..=15")]
    pub addr: u16,
    #[packed_field(bits = "16..=27")]
    pub brightness: u16,
    #[packed_field(bits = "32..=55")]
    pub raw: [u8; 3],
}

impl Debug for HueEntFrameLightRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let xy = XY::from_quant(self.raw);

        write!(
            f,
            "<{:04x}> ({:.3?},{:.3?})@{:04x?}",
            self.addr, xy.x, xy.y, self.brightness
        )
    }
}

fn check_size_valid(len: usize, header_size: usize, element_size: usize) -> HueResult<()> {
    // Must have bytes enough for the header
    if len < header_size {
        return Err(HueError::HueZigbeeDecodeError);
    }

    // Must have a whole number of elements
    if (len - header_size) % element_size != 0 {
        return Err(HueError::HueZigbeeDecodeError);
    }

    Ok(())
}

impl HueEntStart {
    pub fn parse(data: &[u8]) -> HueResult<Self> {
        check_size_valid(data.len(), 2, 2)?;

        let (hdr, data) = data.split_at(2);
        let count = u16::from_be_bytes([hdr[0], hdr[1]]);

        let members = data
            .chunks_exact(2)
            .map(|d| u16::from_le_bytes([d[0], d[1]]))
            .collect();

        Ok(Self { count, members })
    }
}

impl HueEntFrame {
    pub fn parse(data: &[u8]) -> HueResult<Self> {
        if data.len() < 6 {
            return Err(HueError::HueZigbeeDecodeError);
        }

        let (hdr, data) = data.split_at(6);
        let hdr = HueEntFrameHeader::unpack_from_slice(hdr)?;

        let blks = data
            .chunks_exact(7)
            .map(HueEntFrameLightRecord::unpack_from_slice)
            .collect::<Result<_, _>>()?;

        Ok(Self {
            counter: hdr.counter,
            smoothing: hdr.smoothing,
            blks,
        })
    }

    pub fn pack(&self) -> HueResult<Vec<u8>> {
        let hdr = HueEntFrameHeader {
            counter: self.counter,
            smoothing: self.smoothing,
        };

        let mut res = hdr.pack_to_vec()?;

        for blk in &self.blks {
            res.extend(&blk.pack()?);
        }

        Ok(res)
    }
}
