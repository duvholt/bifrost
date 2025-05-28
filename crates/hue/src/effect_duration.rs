use crate::error::HueResult;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EffectDuration(pub u8);

const RESOLUTION_01S_BASE: u8 = 0xFC;
const RESOLUTION_05S_BASE: u8 = 0xCC;
const RESOLUTION_15S_BASE: u8 = 0xA5;
const RESOLUTION_01M_BASE: u8 = 0x79;
const RESOLUTION_05M_BASE: u8 = 0x4A;

const RESOLUTION_01S: u32 = 1; // 1s.
const RESOLUTION_05S: u32 = 5; // 5s.
const RESOLUTION_15S: u32 = 15; // 15s.
const RESOLUTION_01M: u32 = 60; // 1min.
const RESOLUTION_05M: u32 = 5 * 60; // 5min.

impl EffectDuration {
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn from_seconds(seconds: u32) -> HueResult<Self> {
        let (base, resolution) = if seconds < 60 {
            // 1min
            (RESOLUTION_01S_BASE, RESOLUTION_01S)
        } else if seconds < 293 {
            // ~5min
            (RESOLUTION_05S_BASE, RESOLUTION_05S)
        } else if seconds < 295 {
            // 293 and 294 do not fit into any of the bases as they both output 145
            return Ok(Self(146));
        } else if seconds < 878 {
            // ~15min
            (RESOLUTION_15S_BASE, RESOLUTION_15S)
        } else if seconds < 885 {
            return Ok(Self(107));
        } else if seconds < 3510 {
            // ~60min
            (RESOLUTION_01M_BASE, RESOLUTION_01M)
        } else if seconds < 3540 {
            return Ok(Self(63));
        } else if seconds <= 6 * 60 * 60 {
            // 06hrs
            (RESOLUTION_05M_BASE, RESOLUTION_05M)
        } else {
            return Err(crate::error::HueError::EffectDurationOutOfRange(seconds));
        };
        Ok(Self(
            base - ((f64::from(seconds) / f64::from(resolution)).round() as u8),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn seconds_to_effect_duration() {
        // sniffed from the real Hue hub
        let values = vec![
            (5, 145),
            (10, 125),
            (15, 106),
            (20, 101),
            (25, 96),
            (30, 91),
            (35, 86),
            (40, 81),
            (45, 76),
            (50, 71),
            (55, 66),
            (60, 62),
        ];
        for (input, output) in values {
            assert_eq!(
                EffectDuration::from_seconds(input * 60).unwrap(),
                EffectDuration(output)
            );
        }
    }

    #[test]
    pub fn check_for_gaps() {
        // this test only verifies that there are no gaps when converting from seconds to effect duration
        // the steps and resolution might still be wrong
        let six_hours = 6 * 60 * 60;
        let mut prev = 253;
        for seconds in 0..six_hours {
            let EffectDuration(next) = EffectDuration::from_seconds(seconds).unwrap();
            if next != prev {
                assert_eq!(next, prev - 1, "Skipped at {seconds}s");
                prev = next;
            }
        }
    }

    #[test]
    pub fn out_of_range() {
        let seconds = 10 * 60 * 60; // 10h
        assert!(EffectDuration::from_seconds(seconds).is_err());
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn timed_effect_zigbee_dump() {
        // these values were recorded by request timed_effects to a light
        // "timed_effects": {
        //    "effect": "sunrise",
        //    "duration": 3539000
        //  }
        let input: Vec<(u32, u64)> = vec![
            (1500, 0xb000040009fa),
            (2500, 0xb000040009f9),
            (58000, 0xb000040009c2),
            (59000, 0xb000040009c1),
            (60000, 0xb000040009c0),
            (61000, 0xb000040009c0),
            (63000, 0xb000040009bf),
            (68000, 0xb000040009be),
            (73000, 0xb000040009bd),
            (277000, 0xb00004000995),
            (278000, 0xb00004000994),
            (282000, 0xb00004000994),
            (283000, 0xb00004000993),
            (287000, 0xb00004000993),
            (288000, 0xb00004000992),
            (294000, 0xb00004000992),
            (295000, 0xb00004000991),
            (308000, 0xb00004000990),
            (323000, 0xb0000400098f),
            (338000, 0xb0000400098e),
            (353000, 0xb0000400098d),
            (862000, 0xb0000400096c),
            (863000, 0xb0000400096b),
            (864000, 0xb0000400096b),
            (872000, 0xb0000400096b),
            (873000, 0xb0000400096b),
            (874000, 0xb0000400096b),
            (875000, 0xb0000400096b),
            (876000, 0xb0000400096b),
            (877000, 0xb0000400096b),
            (878000, 0xb0000400096b),
            (879000, 0xb0000400096b),
            (880000, 0xb0000400096b),
            (881000, 0xb0000400096b),
            (882000, 0xb0000400096b),
            (883000, 0xb0000400096b),
            (884000, 0xb0000400096b),
            (885000, 0xb0000400096a),
            (886000, 0xb0000400096a),
            (887000, 0xb0000400096a),
            (888000, 0xb0000400096a),
            (899000, 0xb0000400096a),
            (900000, 0xb0000400096a),
            (901000, 0xb0000400096a),
            (930000, 0xb00004000969),
            (990000, 0xb00004000968),
            (1050000, 0xb00004000967),
            (3390000, 0xb00004000940),
            (3450000, 0xb0000400093f),
            (3510000, 0xb0000400093f),
            (3539000, 0xb0000400093f),
            (3540000, 0xb0000400093e),
            (3599000, 0xb0000400093e),
            (3600000, 0xb0000400093e),
            (3601000, 0xb0000400093e),
            (3750000, 0xb0000400093d),
            (4050000, 0xb0000400093c),
            (4350000, 0xb0000400093b),
            (20850000, 0xb00004000904),
            (21150000, 0xb00004000903),
            (21450000, 0xb00004000902),
            // max
            (21600000, 0xb00004000902),
        ];

        for (input_ms, zigbee_data) in input {
            let nearest_second: u32 = (f64::from(input_ms) / 1000.0).round() as u32;
            let ed = EffectDuration::from_seconds(nearest_second).unwrap();
            let zigbee_effect_duration = (zigbee_data & 0xff) as u8; // last byte is effect duration
            assert_eq!(
                ed.0, zigbee_effect_duration,
                "Failed to convert {input_ms}ms ({nearest_second}s) into effect duration {zigbee_effect_duration}"
            );
        }
    }
}
