use std::fmt::Debug;
use std::io::Read;

use byteorder::{BigEndian as BE, ReadBytesExt};
use packed_struct::prelude::*;

use crate::error::ZclResult;

#[derive(PrimitiveEnum_u8, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ZclFrameType {
    ProfileWide = 0x00,
    ClusterSpecific = 0x01,
}

#[derive(PrimitiveEnum_u8, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ZclFrameDirection {
    ClientToServer = 0x00,
    ServerToClient = 0x01,
}

#[derive(PackedStruct, Clone, Copy)]
#[packed_struct(size_bytes = "1", bit_numbering = "lsb0")]
pub struct ZclFrameFlags {
    #[packed_field(bits = "0..2", ty = "enum")]
    pub frame_type: ZclFrameType,

    #[packed_field(bits = "2")]
    pub manufacturer_specific: bool,

    #[packed_field(bits = "3", ty = "enum")]
    pub direction: ZclFrameDirection,

    #[packed_field(bits = "4")]
    pub disable_default_response: bool,
}

impl Debug for ZclFrameFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ft = match self.frame_type {
            ZclFrameType::ProfileWide => "PW",
            ZclFrameType::ClusterSpecific => "CS",
        };
        let dir = match self.direction {
            ZclFrameDirection::ClientToServer => "C2S",
            ZclFrameDirection::ServerToClient => "S2C",
        };
        write!(f, "[ ")?;
        write!(f, "ft:{ft}, ")?;
        write!(f, "ms:{}, ", u8::from(self.manufacturer_specific))?;
        write!(f, "dir:{dir}, ")?;
        write!(f, "ddr:{}", u8::from(self.disable_default_response))?;
        write!(f, " ]")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ZclFrame {
    pub flags: ZclFrameFlags,
    pub mfcode: Option<u16>,
    pub seqnr: u8,
    pub cmd: u8,
}

impl ZclFrame {
    pub fn parse(data: &mut impl Read) -> ZclResult<Self> {
        let flags = ZclFrameFlags::unpack(&[data.read_u8()?])?;

        let mfcode = if flags.manufacturer_specific {
            Some(data.read_u16::<BE>()?)
        } else {
            None
        };

        let seqnr = data.read_u8()?;
        let cmd = data.read_u8()?;

        Ok(Self {
            flags,
            mfcode,
            seqnr,
            cmd,
        })
    }

    pub fn cluster_specific(&self) -> bool {
        self.flags.frame_type == ZclFrameType::ClusterSpecific
    }

    pub fn manufacturer_specific(&self) -> bool {
        self.flags.manufacturer_specific
    }
}
