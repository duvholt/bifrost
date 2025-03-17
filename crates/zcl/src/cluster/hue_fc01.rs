use packed_struct::PackedStructSlice;

use crate::error::ZclResult;
use crate::frame::ZclFrame;
use hue::zigbee::{HueEntFrame, HueEntSegmentConfig, HueEntSegmentLayout, HueEntStop};

pub fn describe(frame: &ZclFrame, data: &[u8]) -> ZclResult<Option<String>> {
    if !frame.cluster_specific() {
        return Ok(None);
    }

    match frame.cmd {
        1 => Ok(Some(format!("{:x?}", HueEntFrame::parse(data)?))),
        3 => Ok(Some(format!("{:x?}", HueEntStop::unpack_from_slice(data)?))),
        4 => {
            let res = if frame.c2s() && data.len() == 1 {
                "HueEntSegmentLayoutReq".to_string()
            } else {
                format!("{:x?}", HueEntSegmentLayout::parse(data)?)
            };
            Ok(Some(res))
        }
        7 => Ok(Some(format!("{:x?}", HueEntSegmentConfig::parse(data)?))),
        _ => Ok(None),
    }
}
