use crate::api::{EntertainmentSegment, Position};

#[derive(Debug, Clone)]
pub struct GradientProductData {
    pub pixel_count: u32,
    pub points_capable: u32,
    pub entertainment_channel_positions: &'static [Position],
    pub entertainment_segments: &'static [EntertainmentSegment],
}

// These values are just guessed based on chrivers' commit 35e66660c8fa1e39242583e0443745ad0a8d96c3
pub const GRADIENT_LCX005: GradientProductData = GradientProductData {
    pixel_count: 18,
    points_capable: 6,
    entertainment_channel_positions: POSITIONS_LCX005,
    entertainment_segments: &[
        EntertainmentSegment {
            start: 0,
            length: 1,
        },
        EntertainmentSegment {
            start: 1,
            length: 1,
        },
        EntertainmentSegment {
            start: 2,
            length: 1,
        },
        EntertainmentSegment {
            start: 3,
            length: 1,
        },
        EntertainmentSegment {
            start: 4,
            length: 1,
        },
        EntertainmentSegment {
            start: 5,
            length: 1,
        },
        EntertainmentSegment {
            start: 6,
            length: 1,
        },
    ],
};

// These hardcoded channel positions are likely used by all light strips made for monitors or TVs
// Looks like |‾| (like the led strip attached to the back of a TV)
const POSITIONS_LCX005: &[Position] = &[
    Position {
        x: -0.4,
        y: 0.8,
        z: -0.4,
    },
    Position {
        x: -0.4,
        y: 0.8,
        z: 0.4,
    },
    Position {
        x: -0.22,
        y: 0.8,
        z: 0.4,
    },
    Position {
        x: 0.0,
        y: 0.8,
        z: 0.4,
    },
    Position {
        x: 0.22,
        y: 0.8,
        z: 0.4,
    },
    Position {
        x: 0.4,
        y: 0.8,
        z: 0.4,
    },
    Position {
        x: 0.4,
        y: 0.8,
        z: -0.4,
    },
];

pub const GRADIENT_929004610402: &GradientProductData = &GradientProductData {
    pixel_count: 18,
    points_capable: 9,
    entertainment_channel_positions: POSITIONS_929004610402,
    entertainment_segments: &[
        EntertainmentSegment {
            start: 0,
            length: 6,
        },
        EntertainmentSegment {
            start: 6,
            length: 6,
        },
        EntertainmentSegment {
            start: 12,
            length: 6,
        },
    ],
    // other hardware info:
    // 929004610402 is the 3m-long Hue Flux strip light, which has 18 cuttable parts that seem to be the smallest addressable physical unit.
    // Each cuttable part contains 6 groups of SMD LEDs.
    // Although it has 18 possible segments, it's not possible to specify more than 9 colors in the zigbee message.
};

// hardcoded channel positions likely used by all other multi-segment light strips
// Corresponds to a straight line
const POSITIONS_929004610402: &[Position] = &[
    Position {
        x: -0.4,
        y: 0.8,
        z: -0.4,
    },
    Position {
        x: 0.0,
        y: 0.8,
        z: -0.4,
    },
    Position {
        x: 0.4,
        y: 0.8,
        z: -0.4,
    },
];
