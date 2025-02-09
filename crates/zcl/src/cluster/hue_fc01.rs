use packed_struct::PackedStructSlice;

use crate::error::ZclResult;
use crate::frame::ZclFrame;
use crate::hue::{HueEntFrame, HueEntStart, HueEntStop};

pub fn describe(frame: &ZclFrame, data: &[u8]) -> ZclResult<Option<String>> {
    if !frame.cluster_specific() {
        return Ok(None);
    }

    match frame.cmd {
        1 => Ok(Some(format!("{:x?}", HueEntFrame::parse(data)?))),
        3 => Ok(Some(format!("{:x?}", HueEntStop::unpack_from_slice(data)?))),
        7 => Ok(Some(format!("{:x?}", HueEntStart::parse(data)?))),
        _ => Ok(None),
    }
}
