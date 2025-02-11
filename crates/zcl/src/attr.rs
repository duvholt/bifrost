use std::io::Read;
use std::{fmt::Debug, io::Cursor};

use byteorder::{ReadBytesExt, BE, LE};
use packed_struct::prelude::*;

use crate::error::{ZclError, ZclResult};

#[derive(PrimitiveEnum_u8, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ZclProfileCommand {
    ReadAttribute = 0x00,
    ReadAttributeRsp = 0x01,
    WriteAttribute = 0x02,
    WriteAttributeRsp = 0x03,
}

#[derive(PrimitiveEnum_u8, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ZclCommand {
    ReadAttrib = 0x00,
    ReadAttribResp = 0x01,
    WriteAttrib = 0x02,
    WriteAttribUndiv = 0x03,
    WriteAttribResp = 0x04,
    WriteAttribNoResp = 0x05,
    ConfigReport = 0x06,
    ConfigReportResp = 0x07,
    ReadReportCfg = 0x08,
    ReadReportCfgResp = 0x09,
    ReportAttrib = 0x0a,
    DefaultResp = 0x0b,
    DiscAttrib = 0x0c,
    DiscAttribResp = 0x0d,
    ReadAttribStruct = 0x0e,
    WriteAttribStruct = 0x0f,
    WriteAttribStructResp = 0x10,
    DiscoverCommandsReceived = 0x11,
    DiscoverCommandsReceivedRes = 0x12,
    DiscoverCommandsGenerated = 0x13,
    DiscoverCommandsGeneratedRes = 0x14,
    DiscoverAttrExt = 0x15,
    DiscoverAttrExtRes = 0x16,
}

#[derive(PrimitiveEnum_u8, Debug, Clone, Copy, Eq, PartialEq)]
pub enum ZclDataType {
    /** Null data type */
    Null = 0x00,

    /** 8-bit value data type */
    Zcl8bit = 0x08,

    /** 16-bit value data type */
    Zcl16bit = 0x09,

    /** 32-bit value data type */
    Zcl32bit = 0x0b,

    /** Boolean data type */
    ZclBool = 0x10,

    /** 8-bit bitmap data type */
    Zcl8bitmap = 0x18,

    /** 16-bit bitmap data type */
    Zcl16bitmap = 0x19,

    /** 32-bit bitmap data type */
    Zcl32bitmap = 0x1b,

    /** Unsigned 8-bit value data type */
    ZclU8 = 0x20,

    /** Unsigned 16-bit value data type */
    ZclU16 = 0x21,

    /** Unsigned 32-bit value data type */
    ZclU32 = 0x23,

    /** Unsigned 16-bit value data type */
    ZclI16 = 0x29,

    /** Unsigned 8-bit value data type */
    ZclE8 = 0x30,

    /** Byte array data type */
    ZclBytearray = 0x41,

    /** Charactery string (array) data type */
    ZclCharstring = 0x42,

    /** IEEE address (U64) type */
    ZclIeeeaddr = 0xf0,

    /** Invalid data type */
    ZclInvalid = 0xff,
}

#[derive(Debug, Clone)]
pub struct ZclReadAttr {
    pub attr: Vec<u16>,
}

impl ZclReadAttr {
    pub fn parse(data: &[u8]) -> ZclResult<Self> {
        if data.len() % 2 != 0 {
            return Err(ZclError::PackedStructError(PackingError::InvalidValue));
        }

        let mut attr = vec![];

        data.chunks(2)
            .for_each(|v| attr.push(u16::from_le_bytes([v[0], v[1]])));

        Ok(Self { attr })
    }
}

#[derive(Clone)]
pub enum ZclAttrValue {
    Null,
    X8(i8),
    X16(i16),
    X32(i32),
    Bool(bool),
    B8(u8),
    B16(u16),
    B32(u32),
    U8(u8),
    U16(u16),
    U32(u32),
    I16(i16),
    E8(u8),
    Bytes(Vec<u8>),
    String(String),
    IeeeAddr(Vec<u8>),
    Unsupported,
}

impl Debug for ZclAttrValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "Null"),
            Self::X8(val) => write!(f, "x8:{}", val),
            Self::X16(val) => write!(f, "x16:{}", val),
            Self::X32(val) => write!(f, "x32:{}", val),
            Self::Bool(val) => write!(f, "bool:{}", val),
            Self::B8(val)  => write!(f, "b8:{:02X}", val),
            Self::B16(val) => write!(f, "b16:{:04X}", val),
            Self::B32(val) => write!(f, "b32:{:08X}", val),
            Self::U8(val) => write!(f, "u8:{:02X}", val),
            Self::U16(val) => write!(f, "u16:{:04X}", val),
            Self::U32(val) => write!(f, "u32:{:08X}", val),
            Self::I16(val) => write!(f, "i16:{:04X}", val),
            Self::E8(val) => write!(f, "e8:{:02X}", val),
            Self::Bytes(val) => write!(f, "hex:{}", hex::encode(val)),
            Self::String(val) => write!(f, "str:{}", &val),
            Self::IeeeAddr(val) => write!(f, "ieeeaddr {}", hex::encode(val)),
            Self::Unsupported => write!(f, "Unsupported"),
        }
    }
}

impl Debug for ZclAttr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04x}:{:?}", self.key, self.value)
    }
}

#[derive(Clone)]
pub struct ZclAttr {
    pub key: u16,
    pub value: ZclAttrValue,
}

impl ZclAttr {
    fn from_reader(rdr: &mut impl Read, check_status: bool) -> ZclResult<Self> {
        let key = rdr.read_u16::<LE>()?;

        if check_status {
            let status = rdr.read_u8()?;
            if status != 0 {
                return Ok(Self {
                    key,
                    value: ZclAttrValue::Unsupported,
                });
            }
        }

        let dtype = ZclDataType::from_primitive(rdr.read_u8()?)
            .ok_or(ZclError::PackedStructError(PackingError::InvalidValue))?;

        let value = match dtype {
            ZclDataType::Null => ZclAttrValue::Null,
            ZclDataType::Zcl8bit => ZclAttrValue::X8(rdr.read_i8()?),
            ZclDataType::Zcl16bit => ZclAttrValue::X16(rdr.read_i16::<LE>()?),
            ZclDataType::Zcl32bit => ZclAttrValue::X32(rdr.read_i32::<LE>()?),
            ZclDataType::ZclBool => ZclAttrValue::Bool(rdr.read_u8()? != 0),
            ZclDataType::Zcl8bitmap => ZclAttrValue::B8(rdr.read_u8()?),
            ZclDataType::Zcl16bitmap => ZclAttrValue::B16(rdr.read_u16::<LE>()?),
            ZclDataType::Zcl32bitmap => ZclAttrValue::B32(rdr.read_u32::<LE>()?),
            ZclDataType::ZclU8 => ZclAttrValue::U8(rdr.read_u8()?),
            ZclDataType::ZclU16 => ZclAttrValue::U16(rdr.read_u16::<LE>()?),
            ZclDataType::ZclU32 => ZclAttrValue::U32(rdr.read_u32::<LE>()?),
            ZclDataType::ZclI16 => ZclAttrValue::I16(rdr.read_i16::<LE>()?),
            ZclDataType::ZclE8 => ZclAttrValue::E8(rdr.read_u8()?),
            ZclDataType::ZclBytearray => {
                let len = rdr.read_u8()?;
                let mut buf = vec![0; len as usize];
                rdr.read_exact(&mut buf)?;
                ZclAttrValue::Bytes(buf)
            }
            ZclDataType::ZclCharstring => {
                let len = rdr.read_u8()?;
                let mut buf = vec![0; len as usize];
                rdr.read_exact(&mut buf)?;
                ZclAttrValue::String(String::from_utf8(buf)?)
            }
            ZclDataType::ZclIeeeaddr => todo!(),
            ZclDataType::ZclInvalid => todo!(),
        };

        Ok(Self { key, value })
    }

    pub fn readattr_from_reader(rdr: &mut impl Read) -> ZclResult<Self> {
        Self::from_reader(rdr, true)
    }

    pub fn writeattr_from_reader(rdr: &mut impl Read) -> ZclResult<Self> {
        Self::from_reader(rdr, false)
    }
}

#[derive(Debug, Clone)]
pub struct ZclReadAttrResp {
    pub attr: Vec<ZclAttr>,
}

impl ZclReadAttrResp {
    pub fn parse(data: &[u8]) -> ZclResult<Self> {
        let mut attr = vec![];

        let mut cur = Cursor::new(data);
        while (cur.position() as usize) < data.len() {
            attr.push(ZclAttr::readattr_from_reader(&mut cur)?);
        }

        Ok(Self { attr })
    }
}

#[derive(Debug, Clone)]
pub struct ZclWriteAttr {
    pub attr: Vec<ZclAttr>,
}

impl ZclWriteAttr {
    pub fn parse(data: &[u8]) -> ZclResult<Self> {
        let mut attr = vec![];

        let mut cur = Cursor::new(data);
        while (cur.position() as usize) < data.len() {
            attr.push(ZclAttr::writeattr_from_reader(&mut cur)?);
        }

        Ok(Self { attr })
    }
}

#[derive(Debug, Clone)]
pub struct ZclReportAttr {
    pub attr: Vec<ZclAttr>,
}

impl ZclReportAttr {
    pub fn parse(data: &[u8]) -> ZclResult<Self> {
        let mut attr = vec![];

        let mut cur = Cursor::new(data);
        while (cur.position() as usize) < data.len() {
            attr.push(ZclAttr::writeattr_from_reader(&mut cur)?);
        }

        Ok(Self { attr })
    }
}

#[derive(Debug, Clone)]
pub struct ZclDefaultResp {
    pub cmd: u8,
    pub stat: u8,
}

impl ZclDefaultResp {
    pub fn parse(data: &[u8]) -> ZclResult<Self> {
        Ok(Self {
            cmd: data[0],
            stat: data[1],
        })
    }
}

#[derive(Debug, Clone)]
pub struct ZclWriteAttrResp {
    pub attr: Vec<u8>,
}

impl ZclWriteAttrResp {
    pub fn parse(data: &[u8]) -> ZclResult<Self> {
        Ok(Self {
            attr: data.to_vec(),
        })
    }
}
