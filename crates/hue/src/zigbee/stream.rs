use packed_struct::prelude::*;

use crate::error::HueResult;
use crate::zigbee::{HueEntFrame, HueEntFrameLightRecord, HueEntSegmentConfig, HueEntStop};

pub struct EntertainmentZigbeeStream {
    smoothing: u16,
    counter: u32,
}

#[derive(Debug, Clone)]
pub struct ZigbeeMessage {
    pub cluster: u16,
    pub command: u8,
    pub data: Vec<u8>,
    pub ddr: bool,
}

impl ZigbeeMessage {
    pub fn new(cluster: u16, command: u8, data: Vec<u8>) -> Self {
        Self {
            cluster,
            command,
            data,
            ddr: true,
        }
    }

    pub fn with_ddr(self, ddr: bool) -> Self {
        Self { ddr, ..self }
    }
}

impl Default for EntertainmentZigbeeStream {
    fn default() -> Self {
        Self::new()
    }
}

impl EntertainmentZigbeeStream {
    pub const DEFAULT_SMOOTHING: u16 = 0x0400;
    pub const CLUSTER: u16 = 0xFC01;
    pub const CMD_SEGMENT_MAP: u8 = 7;
    pub const CMD_RESET: u8 = 3;
    pub const CMD_FRAME: u8 = 1;

    pub const fn new() -> Self {
        Self {
            smoothing: Self::DEFAULT_SMOOTHING,
            counter: 0,
        }
    }

    pub fn smoothing(&self) -> u16 {
        self.smoothing
    }

    pub fn set_smoothing(&mut self, smoothing: u16) {
        self.smoothing = smoothing;
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

        self.counter += 1;

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
