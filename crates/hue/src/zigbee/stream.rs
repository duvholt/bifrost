use packed_struct::prelude::*;

use crate::error::HueResult;
use crate::zigbee::{HueEntFrame, HueEntFrameLightRecord, HueEntStop, ZigbeeTarget};

pub struct EntertainmentZigbeeStream<T: ZigbeeTarget> {
    pub target: T,
    smoothing: u16,
    counter: u32,
}

impl<T: ZigbeeTarget> EntertainmentZigbeeStream<T> {
    pub const DEFAULT_SMOOTHING: u16 = 0x0400;

    pub fn new(target: T) -> Self {
        Self {
            target,
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

    pub fn reset(&mut self) -> HueResult<T::Result> {
        let ent = HueEntStop {
            x0: 0,
            x1: 1,
            counter: self.counter,
        };

        self.counter += 1;

        self.target.send(0xFC01, 3, &ent.pack()?)
    }

    pub fn frame(&mut self, blks: Vec<HueEntFrameLightRecord>) -> HueResult<T::Result> {
        let ent = HueEntFrame {
            counter: self.counter,
            smoothing: self.smoothing,
            blks,
        };

        self.counter += 1;

        self.target.send(0xFC01, 1, &ent.pack()?)
    }
}
