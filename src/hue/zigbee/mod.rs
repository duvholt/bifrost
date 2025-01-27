use std::io::{Read, Write};

use bitflags::bitflags;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use packed_struct::derive::{PackedStruct, PrimitiveEnum_u8};
use packed_struct::{PackedStruct, PackedStructSlice, PrimitiveEnum};

use crate::error::{ApiError, ApiResult};
use crate::model::flags::TakeFlag;
use crate::model::types::XY;

#[derive(PrimitiveEnum_u8, Debug, Copy, Clone)]
pub enum EffectType {
    NoEffect = 0x00,
    Candle = 0x01,
    Fireplace = 0x02,
    Prism = 0x03,
    Sunrise = 0x09,
    Sparkle = 0x0a,
    Opal = 0x0b,
    Glisten = 0x0c,
    Underwater = 0x0e,
    Cosmos = 0x0f,
    Sunbeam = 0x10,
    Enchant = 0x11,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: u16 {
        const ON_OFF          = 1 <<  0;
        const BRIGHTNESS      = 1 <<  1;
        const COLOR_MIREK     = 1 <<  2;
        const COLOR_XY        = 1 <<  3;
        const UNKNOWN_0       = 1 <<  4;
        const EFFECT_TYPE     = 1 <<  5;
        const GRADIENT_PARAMS = 1 <<  6;
        const EFFECT_SPEED    = 1 <<  7;

        const GRADIENT_COLORS = 1 <<  8;
        const UNUSED1         = 1 <<  9;
        const UNUSED2         = 1 << 10;
        const UNUSED3         = 1 << 11;
        const UNUSED4         = 1 << 12;
        const UNUSED5         = 1 << 13;
        const UNUSED6         = 1 << 14;
        const UNUSED7         = 1 << 15;
    }
}

#[derive(Default, PackedStruct)]
#[packed_struct(endian = "lsb", bit_numbering = "msb0")]
pub struct GradientUpdateHeader {
    /// First 4 bits of first byte: number of gradient light points
    #[packed_field(bits = "0..4")]
    pub nlights: u8,

    /// Last 4 bits of first byte: unknown
    #[packed_field(bits = "4..8")]
    pub resv0: u8,

    /// Second byte: unknown
    pub resv1: u8,

    /// Third and fourth byte: unknown
    pub resv2: u16,
}

#[derive(Debug, PackedStruct)]
#[packed_struct(endian = "lsb")]
struct PackedXY12 {
    #[packed_field(size_bits = "12")]
    x: u16,
    #[packed_field(size_bits = "12")]
    y: u16,
}

pub struct GradientColors {
    pub header: GradientUpdateHeader,
    pub points: Vec<XY>,
}

#[derive(Debug, PackedStruct)]
#[packed_struct(endian = "lsb")]
pub struct GradientParams {
    pub scale: u8,
    pub offset: u8,
}

#[derive(Default)]
pub struct HueZigbeeUpdate {
    pub onoff: Option<u8>,
    pub brightness: Option<u8>,
    pub color_mirek: Option<u16>,
    pub color_xy: Option<XY>,
    pub unk0: Option<u16>,
    pub gradient_colors: Option<GradientColors>,
    pub gradient_params: Option<GradientParams>,
    pub effect_type: Option<EffectType>,
    pub effect_speed: Option<u8>,
}

impl HueZigbeeUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn with_on_off(mut self, on_off: bool) -> Self {
        self.onoff = Some(if on_off { 1 } else { 0 });
        self
    }

    #[must_use]
    pub const fn with_brightness(mut self, brightness: u8) -> Self {
        self.brightness = Some(brightness);
        self
    }

    #[must_use]
    pub const fn with_color_mirek(mut self, mirek: u16) -> Self {
        self.color_mirek = Some(mirek);
        self
    }

    #[must_use]
    pub const fn with_color_xy(mut self, xy: XY) -> Self {
        self.color_xy = Some(xy);
        self
    }

    #[must_use]
    pub const fn with_unknown0(mut self, unk0: u16) -> Self {
        self.unk0 = Some(unk0);
        self
    }

    #[must_use]
    pub fn with_gradient_colors(mut self, colors: GradientColors) -> Self {
        self.gradient_colors = Some(colors);
        self
    }

    #[must_use]
    pub const fn with_gradient_transform(mut self, transform: GradientParams) -> Self {
        self.gradient_params = Some(transform);
        self
    }

    #[must_use]
    pub const fn with_effect_type(mut self, effect_type: EffectType) -> Self {
        self.effect_type = Some(effect_type);
        self
    }

    #[must_use]
    pub const fn with_effect_speed(mut self, effect_speed: u8) -> Self {
        self.effect_speed = Some(effect_speed);
        self
    }
}

#[allow(clippy::cast_possible_truncation)]
impl HueZigbeeUpdate {
    pub fn from_reader(rdr: &mut impl Read) -> ApiResult<Self> {
        let mut hz = Self::default();

        let mut flags = Flags::from_bits(rdr.read_u16::<LittleEndian>()?).unwrap();

        if flags.take(Flags::ON_OFF) {
            hz.onoff = Some(rdr.read_u8()?);
        }

        if flags.take(Flags::BRIGHTNESS) {
            hz.brightness = Some(rdr.read_u8()?);
        }

        if flags.take(Flags::COLOR_MIREK) {
            hz.color_mirek = Some(rdr.read_u16::<LittleEndian>()?);
        }

        if flags.take(Flags::COLOR_XY) {
            hz.color_xy = Some(XY::new(
                f64::from(rdr.read_u16::<LittleEndian>()?) / f64::from(0xFFFF),
                f64::from(rdr.read_u16::<LittleEndian>()?) / f64::from(0xFFFF),
            ));
        }

        if flags.take(Flags::UNKNOWN_0) {
            hz.unk0 = Some(rdr.read_u16::<LittleEndian>()?);
        }

        if flags.take(Flags::EFFECT_TYPE) {
            let data = rdr.read_u8()?;
            hz.effect_type =
                Some(EffectType::from_primitive(data).ok_or(ApiError::HueZigbeeDecodeError)?);
        }

        if flags.take(Flags::GRADIENT_COLORS) {
            let len = rdr.read_u8()?;
            let mut data = vec![0; 4];
            rdr.read_exact(&mut data)?;
            let header = GradientUpdateHeader::unpack_from_slice(&data)?;
            debug_assert!(len == header.nlights * 3 + 4);

            let mut points = vec![];
            for _ in 0..header.nlights {
                let mut point = vec![0; 3];
                rdr.read_exact(&mut point)?;
                let point = PackedXY12::unpack_from_slice(&point)?;
                points.push(XY {
                    x: f64::from(point.x) / f64::from(0xFFF),
                    y: f64::from(point.y) / f64::from(0xFFF),
                });
            }
            hz.gradient_colors = Some(GradientColors { header, points });
        }

        if flags.take(Flags::EFFECT_SPEED) {
            hz.effect_speed = Some(rdr.read_u8()?);
        }

        if flags.take(Flags::GRADIENT_PARAMS) {
            hz.gradient_params = Some(GradientParams {
                scale: rdr.read_u8()?,
                offset: rdr.read_u8()?,
            });
        }

        if flags.is_empty() {
            Ok(hz)
        } else {
            Err(ApiError::HueZigbeeUnknownFlags(flags.bits()))
        }
    }
}
