use std::fmt::Debug;
use std::io::Write;

use byteorder::{WriteBytesExt, BE, LE};
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
pub struct HueEntSegmentConfig {
    pub members: Vec<u16>,
}

#[derive(PackedStruct, Debug, Clone)]
#[packed_struct(size = "2", endian = "lsb")]
pub struct HueEntSegment {
    pub length: u8,
    pub index: u8,
}

#[derive(Debug, Clone)]
pub struct HueEntSegmentLayout {
    pub members: Vec<HueEntSegment>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightRecordMode {
    Segment = 0b00000,
    Device = 0b01011,
}

#[derive(PackedStruct, Clone)]
#[packed_struct(size_bytes = "7", endian = "lsb", bit_numbering = "msb0")]
pub struct HueEntFrameLightRecord {
    /// Zigbee network address of recipient
    #[packed_field(bits = "0..=15")]
    addr: u16,

    /// Field contains brightness (top 11 bits) and mode (bottom 5 bits)
    brightness: u16,

    /// Raw (packed) color value (from [`XY::to_quant()`])
    #[packed_field(bits = "32..=55")]
    raw: [u8; 3],
}

impl HueEntFrameLightRecord {
    #[must_use]
    pub const fn new(addr: u16, brightness: u16, mode: LightRecordMode, raw: [u8; 3]) -> Self {
        Self {
            addr,
            brightness: (brightness << 5) | (mode as u16),
            raw,
        }
    }

    #[must_use]
    pub const fn brightness(&self) -> u16 {
        self.brightness >> 5
    }

    #[must_use]
    pub const fn raw(&self) -> [u8; 3] {
        self.raw
    }
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

const fn check_size_valid(len: usize, header_size: usize, element_size: usize) -> HueResult<()> {
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

impl HueEntSegmentConfig {
    #[must_use]
    pub fn new(map: &[u16]) -> Self {
        Self {
            members: map.to_vec(),
        }
    }

    pub fn parse(data: &[u8]) -> HueResult<Self> {
        check_size_valid(data.len(), 2, 2)?;

        let (hdr, data) = data.split_at(2);

        let count = u16::from_be_bytes([hdr[0], hdr[1]]);

        let members = data
            .chunks_exact(2)
            .take(count as usize)
            .map(|d| u16::from_le_bytes([d[0], d[1]]))
            .collect();

        Ok(Self { members })
    }

    pub fn pack(&self) -> HueResult<Vec<u8>> {
        let mut res = vec![];
        let count = u16::try_from(self.members.len())?;
        res.write_u16::<BE>(count)?;
        for m in &self.members {
            res.write_u16::<LE>(*m)?;
        }

        Ok(res)
    }
}

impl HueEntSegmentLayout {
    #[must_use]
    pub fn new(map: &[HueEntSegment]) -> Self {
        Self {
            members: map.to_vec(),
        }
    }

    pub fn parse(data: &[u8]) -> HueResult<Self> {
        check_size_valid(data.len(), 3, 2)?;

        let (hdr, data) = data.split_at(3);

        let count = hdr[2];

        let members = data
            .chunks_exact(2)
            .take(usize::from(count))
            .map(HueEntSegment::unpack_from_slice)
            .collect::<Result<_, _>>()?;

        Ok(Self { members })
    }

    pub fn pack(&self) -> HueResult<Vec<u8>> {
        let mut res = vec![];
        let count = u16::try_from(self.members.len())?;
        res.write_u16::<LE>(0)?;
        res.write_u16::<LE>(count)?;
        for m in &self.members {
            res.write_all(&m.pack()?)?;
        }

        Ok(res)
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

#[cfg(test)]
mod tests {
    use packed_struct::prelude::*;

    use crate::zigbee::{HueEntFrameLightRecord, LightRecordMode};

    #[test]
    fn light_record() {
        let foo = HueEntFrameLightRecord {
            addr: 0x1122,
            brightness: 0x7FF << 5,
            raw: [0xAA, 0xBB, 0xCC],
        };

        let data = foo.pack().unwrap();

        assert_eq!("2211e0ffaabbcc", hex::encode(data));
    }

    #[test]
    fn light_record_segment() {
        let foo = HueEntFrameLightRecord::new(
            0x1122,
            0x7FF,
            LightRecordMode::Segment,
            [0xAA, 0xBB, 0xCC],
        );

        let data = foo.pack().unwrap();

        assert_eq!("2211e0ffaabbcc", hex::encode(data));
    }

    #[test]
    fn light_record_device() {
        let foo =
            HueEntFrameLightRecord::new(0x1122, 0x7FF, LightRecordMode::Device, [0xAA, 0xBB, 0xCC]);

        let data = foo.pack().unwrap();

        assert_eq!("2211ebffaabbcc", hex::encode(data));
    }
}
