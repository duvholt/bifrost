use chrono::Duration;
use packed_struct::prelude::*;

use crate::error::{HueError, HueResult};
use crate::zigbee::{HueEntFrame, HueEntFrameLightRecord, HueEntSegmentConfig, HueEntStop};

pub struct EntertainmentZigbeeStream {
    smoothing: u16,
    counter: u32,
}

pub const PHILIPS_HUE_ZIGBEE_VENDOR_ID: u16 = 0x100B;

#[derive(Debug, Clone)]
pub struct ZigbeeMessage {
    /// Zigbee cluster id
    pub cluster: u16,

    /// Zigbee command id
    pub command: u8,

    /// Zigbee Zcl data bytes
    pub data: Vec<u8>,

    /// Disable default response
    pub ddr: bool,

    /// Frametype
    pub frametype: u8,

    /// Manufacturer Code
    pub mfc: Option<u16>,
}

impl ZigbeeMessage {
    #[must_use]
    pub const fn new(cluster: u16, command: u8, data: Vec<u8>) -> Self {
        Self {
            cluster,
            command,
            data,
            frametype: 1,
            ddr: true,
            mfc: Some(PHILIPS_HUE_ZIGBEE_VENDOR_ID),
        }
    }

    #[must_use]
    pub fn with_ddr(self, ddr: bool) -> Self {
        Self { ddr, ..self }
    }

    #[must_use]
    pub fn with_mfc(self, mfc: Option<u16>) -> Self {
        Self { mfc, ..self }
    }
}

impl Default for EntertainmentZigbeeStream {
    fn default() -> Self {
        Self::new(0)
    }
}

impl EntertainmentZigbeeStream {
    pub const DEFAULT_SMOOTHING: u16 = 0x0400;
    pub const CLUSTER: u16 = 0xFC01;
    pub const CMD_FRAME: u8 = 1;
    pub const CMD_RESET: u8 = 3;
    pub const CMD_LIGHT_BALANCE: u8 = 5;
    pub const CMD_SEGMENT_MAP: u8 = 7;

    /// The maximum fade time (0xFFFF) seems to correspond to 2.56 seconds.
    /// (determined experimentally)
    pub const SMOOTHING_MAX_MICROS: i64 = 2_560_000;

    #[must_use]
    pub const fn new(counter: u32) -> Self {
        Self {
            smoothing: Self::DEFAULT_SMOOTHING,
            counter,
        }
    }

    #[must_use]
    pub const fn counter(&self) -> u32 {
        self.counter
    }

    #[must_use]
    pub const fn smoothing(&self) -> u16 {
        self.smoothing
    }

    pub const fn set_smoothing(&mut self, smoothing: u16) {
        self.smoothing = smoothing;
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn duration_to_smoothing(duration: Duration) -> HueResult<u16> {
        // Get number of microseconds, if positive and less than maximum
        let us = duration
            .num_microseconds()
            .filter(|us| (0..Self::SMOOTHING_MAX_MICROS).contains(us))
            .ok_or(HueError::HueZigbeeEncodeError)?;

        // Scale to target range
        let smoothing = (us * 0x10000 / Self::SMOOTHING_MAX_MICROS) as u16;

        Ok(smoothing)
    }

    pub fn set_smoothing_duration(&mut self, duration: Duration) -> HueResult<()> {
        self.set_smoothing(Self::duration_to_smoothing(duration)?);
        Ok(())
    }

    pub fn segment_mapping(&mut self, map: &[u16]) -> HueResult<ZigbeeMessage> {
        let msg = HueEntSegmentConfig::new(map);

        Ok(ZigbeeMessage::new(
            Self::CLUSTER,
            Self::CMD_SEGMENT_MAP,
            msg.pack()?,
        ))
    }

    pub fn reset(&mut self) -> HueResult<ZigbeeMessage> {
        let ent = HueEntStop {
            x0: 0,
            x1: 1,
            counter: self.counter,
        };

        Ok(ZigbeeMessage::new(
            Self::CLUSTER,
            Self::CMD_RESET,
            ent.pack_to_vec()?,
        ))
    }

    pub fn frame(&mut self, blks: Vec<HueEntFrameLightRecord>) -> HueResult<ZigbeeMessage> {
        let ent = HueEntFrame {
            counter: self.counter,
            smoothing: self.smoothing,
            blks,
        };

        self.counter += 1;

        Ok(ZigbeeMessage::new(
            Self::CLUSTER,
            Self::CMD_FRAME,
            ent.pack()?,
        ))
    }
}

#[cfg(test)]
mod tests {

    use chrono::Duration;

    use crate::zigbee::{
        EntertainmentZigbeeStream as EZS, PHILIPS_HUE_ZIGBEE_VENDOR_ID, ZigbeeMessage,
    };

    #[allow(clippy::bool_assert_comparison)]
    #[test]
    fn zigbee_message() {
        let zb = ZigbeeMessage::new(0x1122, 0x33, vec![0x44, 0x55]);
        assert_eq!(zb.cluster, 0x1122);
        assert_eq!(zb.command, 0x33);
        assert_eq!(zb.data, [0x44, 0x55]);
        assert_eq!(zb.ddr, true);
        assert_eq!(zb.frametype, 1);
        assert_eq!(zb.mfc, Some(PHILIPS_HUE_ZIGBEE_VENDOR_ID));

        let zb = zb.with_ddr(false);
        assert_eq!(zb.ddr, false);

        let zb = zb.with_mfc(None);
        assert_eq!(zb.mfc, None);

        let zb = zb.with_mfc(Some(0x1234));
        assert_eq!(zb.mfc, Some(0x1234));
    }

    #[test]
    fn entertainment_zigbee_stream_default() {
        let val = EZS::default();
        assert_eq!(val.counter, 0);
        assert_eq!(val.smoothing, EZS::DEFAULT_SMOOTHING);
    }

    #[test]
    fn entertainment_zigbee_stream() {
        let mut val = EZS {
            smoothing: 0x1122,
            counter: 0x11_22_33_44,
        };
        assert_eq!(val.counter(), 0x11_22_33_44);
        assert_eq!(val.smoothing(), 0x1122);

        val.set_smoothing(0x3344);
        assert_eq!(val.smoothing(), 0x3344);

        val.set_smoothing_duration(Duration::milliseconds(0))
            .unwrap();
        assert_eq!(val.smoothing(), 0);
    }

    #[test]
    fn ezs_reset() {
        let mut ezs = EZS::new(0x1122);

        let rst = ezs.reset().unwrap();
        assert_eq!(rst.cluster, EZS::CLUSTER);
        assert_eq!(rst.command, EZS::CMD_RESET);
        assert_eq!(rst.data, [0x00, 0x01, 0x22, 0x11, 0x00, 0x00]);

        // counter should be the same
        assert_eq!(ezs.counter(), 0x1122);
    }

    #[allow(clippy::bool_assert_comparison, clippy::cast_possible_truncation)]
    #[test]
    fn ezs_frame() {
        let mut ezs = EZS::new(0x1122);

        let rst = ezs.frame(vec![]).unwrap();
        assert_eq!(rst.cluster, EZS::CLUSTER);
        assert_eq!(rst.command, EZS::CMD_FRAME);
        assert_eq!(
            rst.data,
            [
                0x22,
                0x11,
                0x00,
                0x00,
                ezs.smoothing as u8,
                (ezs.smoothing >> 8) as u8
            ]
        );

        // counter should be incremented
        assert_eq!(ezs.counter(), 0x1123);
    }

    #[test]
    fn ezs_segment_mapping() {
        let mut ezs = EZS::new(0x1122);

        let rst = ezs.segment_mapping(&[0xA0A1, 0xB0B1]).unwrap();
        assert_eq!(rst.cluster, EZS::CLUSTER);
        assert_eq!(rst.command, EZS::CMD_SEGMENT_MAP);
        assert_eq!(rst.data, [0x00, 0x02, 0xA1, 0xA0, 0xB1, 0xB0]);

        // counter should be the same
        assert_eq!(ezs.counter(), 0x1122);
    }

    #[test]
    fn duration_zero() {
        let zero_dur = Duration::seconds(0);
        let smo = EZS::duration_to_smoothing(zero_dur).unwrap();
        assert_eq!(smo, 0);
    }

    #[test]
    fn duration_half() {
        let max_dur = Duration::microseconds(EZS::SMOOTHING_MAX_MICROS / 2);
        let smo = EZS::duration_to_smoothing(max_dur).unwrap();
        assert_eq!(smo, 0x8000);
    }

    #[test]
    fn duration_max() {
        let max_dur = Duration::microseconds(EZS::SMOOTHING_MAX_MICROS - 1);
        let smo = EZS::duration_to_smoothing(max_dur).unwrap();
        assert_eq!(smo, 0xFFFF);
    }

    #[test]
    fn duration_negative() {
        let max_dur = Duration::microseconds(-1);
        let smo = EZS::duration_to_smoothing(max_dur);
        assert!(smo.is_err());
    }

    #[test]
    fn duration_over_limit() {
        let max_dur = Duration::microseconds(EZS::SMOOTHING_MAX_MICROS);
        let smo = EZS::duration_to_smoothing(max_dur);
        assert!(smo.is_err());
    }
}
