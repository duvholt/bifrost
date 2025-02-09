use crate::error::ZclResult;
use crate::frame::ZclFrame;
use crate::hue::HueEntFrame;

pub fn describe(frame: &ZclFrame, data: &[u8]) -> ZclResult<Option<String>> {
    if frame.cluster_specific() && frame.cmd == 0x02 {
        let (data, csum) = data.split_at(data.len() - 4);
        let csum = u32::from_be_bytes([csum[0], csum[1], csum[2], csum[3]]);
        let hes = HueEntFrame::parse(data)?;
        Ok(Some(format!("{hes:x?} [PROXY, {csum:08x}]")))
    } else {
        Ok(None)
    }
}
