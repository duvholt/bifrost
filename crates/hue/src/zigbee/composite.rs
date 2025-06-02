use std::io::{Cursor, Read, Write};

use bitflags::bitflags;
use byteorder::{LittleEndian as LE, ReadBytesExt, WriteBytesExt};
use packed_struct::derive::{PackedStruct, PrimitiveEnum_u8};
use packed_struct::{PackedStruct, PackedStructSlice, PrimitiveEnum};

use crate::api::{LightEffect, LightGradientMode, LightTimedEffect};
use crate::effect_duration::EffectDuration;
use crate::error::{HueError, HueResult};
use crate::flags::TakeFlag;
use crate::xy::XY;

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
    Sunset = 0x0d,
    Underwater = 0x0e,
    Cosmos = 0x0f,
    Sunbeam = 0x10,
    Enchant = 0x11,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<LightEffect> for EffectType {
    fn from(value: LightEffect) -> Self {
        match value {
            LightEffect::NoEffect => Self::NoEffect,
            LightEffect::Prism => Self::Prism,
            LightEffect::Opal => Self::Opal,
            LightEffect::Glisten => Self::Glisten,
            LightEffect::Sparkle => Self::Sparkle,
            LightEffect::Fire => Self::Fireplace,
            LightEffect::Candle => Self::Candle,
            LightEffect::Underwater => Self::Underwater,
            LightEffect::Cosmos => Self::Cosmos,
            LightEffect::Sunbeam => Self::Sunbeam,
            LightEffect::Enchant => Self::Enchant,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<LightTimedEffect> for EffectType {
    fn from(value: LightTimedEffect) -> Self {
        match value {
            LightTimedEffect::NoEffect => Self::NoEffect,
            LightTimedEffect::Sunrise => Self::Sunrise,
            LightTimedEffect::Sunset => Self::Sunset,
        }
    }
}

#[derive(PrimitiveEnum_u8, Debug, Copy, Clone, PartialEq, Eq)]
pub enum GradientStyle {
    Linear = 0x00,
    Scattered = 0x02,
    Mirrored = 0x04,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl From<LightGradientMode> for GradientStyle {
    fn from(value: LightGradientMode) -> Self {
        match value {
            LightGradientMode::InterpolatedPalette => Self::Linear,
            LightGradientMode::InterpolatedPaletteMirrored => Self::Mirrored,
            LightGradientMode::RandomPixelated => Self::Scattered,
        }
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: u16 {
        const ON_OFF          = 1 <<  0;
        const BRIGHTNESS      = 1 <<  1;
        const COLOR_MIREK     = 1 <<  2;
        const COLOR_XY        = 1 <<  3;
        const FADE_SPEED      = 1 <<  4;
        const EFFECT_TYPE     = 1 <<  5;
        const GRADIENT_PARAMS = 1 <<  6;
        const EFFECT_SPEED    = 1 <<  7;
        const GRADIENT_COLORS = 1 <<  8;
        const UNUSED_9        = 1 <<  9;
        const UNUSED_A        = 1 << 10;
        const UNUSED_B        = 1 << 11;
        const UNUSED_C        = 1 << 12;
        const UNUSED_D        = 1 << 13;
        const UNUSED_E        = 1 << 14;
        const UNUSED_F        = 1 << 15;
    }
}

#[derive(PackedStruct)]
#[packed_struct(endian = "lsb", bit_numbering = "msb0")]
pub struct GradientUpdateHeader {
    /// First 4 bits of first byte: number of gradient light points
    #[packed_field(bits = "0..4")]
    pub nlights: u8,

    /// Last 4 bits of first byte: MUST BE 0
    #[packed_field(bits = "4..8")]
    pub resv0: u8,

    /// Second byte: gradient style
    #[packed_field(bytes = "1", ty = "enum")]
    pub style: GradientStyle,

    /// Third and fourth byte: seems unused
    #[packed_field(bytes = "2..=3")]
    pub resv2: u16,
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

impl Default for GradientParams {
    fn default() -> Self {
        Self::new()
    }
}

impl GradientParams {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scale: 0x08,
            offset: 0x00,
        }
    }
}

#[derive(Default)]
pub struct HueZigbeeUpdate {
    pub onoff: Option<u8>,
    pub brightness: Option<u8>,
    pub color_mirek: Option<u16>,
    pub color_xy: Option<XY>,
    pub fade_speed: Option<u16>,
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
    pub const fn is_empty(&self) -> bool {
        self.onoff.is_none()
            && self.brightness.is_none()
            && self.color_mirek.is_none()
            && self.color_xy.is_none()
            && self.fade_speed.is_none()
            && self.gradient_colors.is_none()
            && self.gradient_params.is_none()
            && self.effect_type.is_none()
            && self.effect_speed.is_none()
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
    pub const fn with_fade_speed(mut self, speed: u16) -> Self {
        self.fade_speed = Some(speed);
        self
    }

    pub fn with_gradient_colors(
        mut self,
        style: GradientStyle,
        points: Vec<XY>,
    ) -> HueResult<Self> {
        self.gradient_colors = Some(GradientColors {
            header: GradientUpdateHeader {
                nlights: u8::try_from(points.len())?,
                resv0: 0,
                style,
                resv2: 0,
            },
            points,
        });
        Ok(self)
    }

    #[must_use]
    pub const fn with_gradient_params(mut self, transform: GradientParams) -> Self {
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

    #[must_use]
    pub const fn with_effect_duration(self, EffectDuration(effect_speed): EffectDuration) -> Self {
        self.with_effect_speed(effect_speed)
    }
}

#[allow(clippy::cast_possible_truncation)]
impl HueZigbeeUpdate {
    pub fn from_reader(rdr: &mut impl Read) -> HueResult<Self> {
        let mut hz = Self::default();

        let mut flags = Flags::from_bits(rdr.read_u16::<LE>()?).unwrap();

        if flags.take(Flags::ON_OFF) {
            hz.onoff = Some(rdr.read_u8()?);
        }

        if flags.take(Flags::BRIGHTNESS) {
            hz.brightness = Some(rdr.read_u8()?);
        }

        if flags.take(Flags::COLOR_MIREK) {
            hz.color_mirek = Some(rdr.read_u16::<LE>()?);
        }

        if flags.take(Flags::COLOR_XY) {
            hz.color_xy = Some(XY::new(
                f64::from(rdr.read_u16::<LE>()?) / f64::from(0xFFFF),
                f64::from(rdr.read_u16::<LE>()?) / f64::from(0xFFFF),
            ));
        }

        if flags.take(Flags::FADE_SPEED) {
            hz.fade_speed = Some(rdr.read_u16::<LE>()?);
        }

        if flags.take(Flags::EFFECT_TYPE) {
            let data = rdr.read_u8()?;
            hz.effect_type =
                Some(EffectType::from_primitive(data).ok_or(HueError::HueZigbeeDecodeError)?);
        }

        if flags.take(Flags::GRADIENT_COLORS) {
            let len = rdr.read_u8()?;
            let mut data = vec![0; 4];
            rdr.read_exact(&mut data)?;
            let header = GradientUpdateHeader::unpack_from_slice(&data)?;
            debug_assert!(len == header.nlights * 3 + 4);

            let mut points = vec![];
            for _ in 0..header.nlights {
                let mut bytes = [0u8; 3];
                rdr.read_exact(&mut bytes)?;
                points.push(XY::from_quant(bytes));
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
            Err(HueError::HueZigbeeUnknownFlags(flags.bits()))
        }
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
impl HueZigbeeUpdate {
    pub fn to_vec(&self) -> HueResult<Vec<u8>> {
        let mut cur = Cursor::new(vec![]);
        self.serialize(&mut cur)?;
        Ok(cur.into_inner())
    }

    pub fn serialize(&self, wtr: &mut impl Write) -> HueResult<()> {
        #[allow(clippy::ref_option)]
        fn opt_to_flag<T>(flags: &mut Flags, opt: &Option<T>, flag: Flags) {
            if opt.is_some() {
                flags.insert(flag);
            }
        }

        let mut flags = Flags::empty();
        opt_to_flag(&mut flags, &self.onoff, Flags::ON_OFF);
        opt_to_flag(&mut flags, &self.brightness, Flags::BRIGHTNESS);
        opt_to_flag(&mut flags, &self.color_mirek, Flags::COLOR_MIREK);
        opt_to_flag(&mut flags, &self.color_xy, Flags::COLOR_XY);
        opt_to_flag(&mut flags, &self.fade_speed, Flags::FADE_SPEED);
        opt_to_flag(&mut flags, &self.effect_type, Flags::EFFECT_TYPE);
        opt_to_flag(&mut flags, &self.effect_speed, Flags::EFFECT_SPEED);
        opt_to_flag(&mut flags, &self.gradient_colors, Flags::GRADIENT_COLORS);
        opt_to_flag(&mut flags, &self.gradient_params, Flags::GRADIENT_PARAMS);

        wtr.write_u16::<LE>(flags.bits())?;

        if let Some(onoff) = self.onoff {
            wtr.write_u8(onoff)?;
        }

        if let Some(bright) = self.brightness {
            wtr.write_u8(bright)?;
        }

        if let Some(mirek) = self.color_mirek {
            wtr.write_u16::<LE>(mirek)?;
        }

        if let Some(xy) = self.color_xy {
            wtr.write_u16::<LE>((xy.x * f64::from(0xFFFF)) as u16)?;
            wtr.write_u16::<LE>((xy.y * f64::from(0xFFFF)) as u16)?;
        }

        if let Some(fade_speed) = self.fade_speed {
            wtr.write_u16::<LE>(fade_speed)?;
        }

        if let Some(etype) = self.effect_type {
            wtr.write_u8(etype.to_primitive())?;
        }

        if let Some(grad_color) = &self.gradient_colors {
            let len = u8::try_from(4 + 3 * grad_color.points.len())?;
            wtr.write_u8(len)?;
            wtr.write_all(&grad_color.header.pack()?)?;
            for point in &grad_color.points {
                wtr.write_all(&point.to_quant())?;
            }
        }

        if let Some(effect_speed) = self.effect_speed {
            wtr.write_u8(effect_speed)?;
        }

        if let Some(params) = &self.gradient_params {
            wtr.write_u8(params.scale)?;
            wtr.write_u8(params.offset)?;
        }

        Ok(())
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::error::HueError;
    use crate::xy::XY;
    use crate::zigbee::{EffectType, GradientParams, GradientStyle, HueZigbeeUpdate};
    use crate::{compare, compare_float, compare_xy, compare_xy_quant};

    #[test]
    fn hzb_none() {
        let hz = HueZigbeeUpdate::new();
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x00, 0x00]);
    }

    #[test]
    fn hzb_onoff() {
        let hz = HueZigbeeUpdate::new().with_on_off(true);
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x01, 0x00, 0x01]);
    }

    #[test]
    fn hzb_brightness() {
        let hz = HueZigbeeUpdate::new().with_brightness(0x42);
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x02, 0x00, 0x42]);
    }

    #[test]
    fn hzb_mirek() {
        let hz = HueZigbeeUpdate::new().with_color_mirek(0x1234);
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x04, 0x00, 0x34, 0x12]);
    }

    #[test]
    fn hzb_xy() {
        let hz = HueZigbeeUpdate::new().with_color_xy(XY::new(0.5, 1.0));
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x08, 0x00, 0xFF, 0x7F, 0xFF, 0xFF]);
    }

    #[test]
    fn hzb_fade_speed() {
        let hz = HueZigbeeUpdate::new().with_fade_speed(0x1234);
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x10, 0x00, 0x34, 0x12]);
    }

    #[test]
    fn hzb_effect_type() {
        let hz = HueZigbeeUpdate::new().with_effect_type(EffectType::Candle);
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x20, 0x00, 0x01]);
    }

    #[test]
    fn hzb_gradient_empty() {
        let hz = HueZigbeeUpdate::new()
            .with_gradient_colors(GradientStyle::Scattered, vec![])
            .unwrap();
        let bytes = hz.to_vec().unwrap();
        assert_eq!(
            bytes,
            &[
                0x00, 0x01, // flags
                0x04, // data length
                0x00, // number of lights (<< 4)
                0x02, // style: scattered
                0x00, 0x00 // padding
            ]
        );
    }

    #[test]
    fn hzb_gradient_lights() {
        let col1 = XY::new(0.5, 0.5);
        let hz = HueZigbeeUpdate::new()
            .with_gradient_colors(GradientStyle::Scattered, vec![col1])
            .unwrap();
        let bytes = hz.to_vec().unwrap();
        let quant = col1.to_quant();
        assert_eq!(
            bytes,
            &[
                0x00, 0x01, // flags
                0x07, // data length
                0x10, // number of lights (<< 4)
                0x02, // style: scattered
                0x00, 0x00, // padding
                quant[0], quant[1], quant[2],
            ]
        );
    }

    #[test]
    fn hzb_gradient_too_many() {
        let col = XY::new(0.5, 0.5);
        let res = HueZigbeeUpdate::new()
            .with_gradient_colors(GradientStyle::Scattered, [col].repeat(257));
        assert!(matches!(res, Err(HueError::TryFromIntError(_))));
    }

    #[test]
    fn hzb_effect_speed() {
        let hz = HueZigbeeUpdate::new().with_effect_speed(0xAB);
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x80, 0x00, 0xAB]);
    }

    #[test]
    fn hzb_gradient_params() {
        let hz = HueZigbeeUpdate::new().with_gradient_params(GradientParams {
            scale: 0x12,
            offset: 0x34,
        });
        let bytes = hz.to_vec().unwrap();

        assert_eq!(bytes, &[0x40, 0x00, 0x12, 0x34]);
    }

    #[test]
    fn hzb_is_empty() {
        use HueZigbeeUpdate as HZU;
        assert!(HZU::new().is_empty());
        assert!(!HZU::new().with_on_off(false).is_empty());
        assert!(!HZU::new().with_brightness(0x01).is_empty());
        assert!(!HZU::new().with_color_mirek(0x01).is_empty());
        assert!(!HZU::new().with_color_xy(XY::D50_WHITE_POINT).is_empty());
        assert!(!HZU::new().with_color_xy(XY::D50_WHITE_POINT).is_empty());
        assert!(!HZU::new().with_effect_type(EffectType::Cosmos).is_empty(),);
        assert!(!HZU::new().with_fade_speed(0x01).is_empty());
        assert!(
            !HZU::new()
                .with_gradient_colors(GradientStyle::Mirrored, vec![])
                .unwrap()
                .is_empty(),
        );
        assert!(
            !HZU::new()
                .with_gradient_params(GradientParams {
                    scale: 0x01,
                    offset: 0x02
                })
                .is_empty(),
        );
    }

    #[test]
    fn hzb_parse_eof() {
        let data = [];
        let mut cur = Cursor::new(data.as_slice());
        match HueZigbeeUpdate::from_reader(&mut cur) {
            Ok(_) => panic!(),
            Err(err) => assert!(matches!(err, HueError::IOError(_))),
        }
    }

    #[test]
    fn hzb_parse_empty() {
        let data = [0x00, 0x00];
        let mut cur = Cursor::new(data.as_slice());
        let res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_onoff() {
        let data = [0x01, 0x00, 0x01];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        assert_eq!(res.onoff.take(), Some(0x01));
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_brightness() {
        let data = [0x02, 0x00, 0x42];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        assert_eq!(res.brightness.take(), Some(0x42));
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_mirek() {
        let data = [0x04, 0x00, 0x22, 0x11];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        assert_eq!(res.color_mirek.take(), Some(0x1122));
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_xy() {
        let data = [0x08, 0x00, 0xFF, 0x7F, 0xFF, 0xFF];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        let xy = res.color_xy.take().unwrap();
        compare_xy!(xy, XY::new(0.5, 1.0));
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_fade_speed() {
        let data = [0x10, 0x00, 0x22, 0x11];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        assert_eq!(res.fade_speed.take(), Some(0x1122));
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_effect_type() {
        let data = [0x20, 0x00, 0x01];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        assert_eq!(
            res.effect_type.take().unwrap() as u8,
            EffectType::Candle as u8
        );
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_effect_speed() {
        let data = [0x80, 0x00, 0xAB];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        assert_eq!(res.effect_speed.take().unwrap(), 0xAB);
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_gradient_params() {
        let data = [0x40, 0x00, 0x12, 0x34];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();

        let params = res.gradient_params.take().unwrap();
        assert_eq!(params.scale, 0x12);
        assert_eq!(params.offset, 0x34);
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_gradient_lights() {
        let col1 = XY::new(0.70, 0.70);

        let quant = col1.to_quant();

        let data = [
            0x00,
            0x01, // flags
            0x07, // data length
            0x10, // number of lights (<< 4)
            0x02, // style: scattered
            0x00,
            0x00, // padding
            quant[0] + 0x01,
            quant[1],
            quant[2],
        ];
        let mut cur = Cursor::new(data.as_slice());
        let mut res = HueZigbeeUpdate::from_reader(&mut cur).unwrap();
        let gc = res.gradient_colors.take().unwrap();
        assert_eq!(gc.points.len(), 1);
        assert_eq!(gc.header.nlights, 1);
        assert_eq!(gc.header.resv0, 0);
        assert_eq!(gc.header.resv2, 0);
        assert_eq!(gc.header.style, GradientStyle::Scattered);
        eprintln!("{:.4?}", gc.points[0]);
        compare_xy_quant!(gc.points[0], col1);
        assert!(res.is_empty());
    }

    #[test]
    fn hzb_parse_unknown_flags() {
        let data = [0x00, 0x20];
        let mut cur = Cursor::new(data.as_slice());
        match HueZigbeeUpdate::from_reader(&mut cur) {
            Ok(_) => panic!(),
            Err(err) => assert!(matches!(err, HueError::HueZigbeeUnknownFlags(_))),
        }
    }

    #[test]
    fn grad_params_new_is_default() {
        let a = GradientParams::new();
        let b = GradientParams::default();
        assert_eq!(a.offset, b.offset);
        assert_eq!(a.scale, b.scale);
    }
}
