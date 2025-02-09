use crate::error::ZclResult;
use crate::frame::ZclFrame;
use hue::zigbee::HueEntFrame;

pub fn describe(frame: &ZclFrame, data: &[u8]) -> ZclResult<Option<String>> {
    if !frame.cluster_specific() {
        return Ok(None);
    }

    match frame.cmd {
        0x00 => Ok(Some("ScanRequest".to_string())),
        0x02 => {
            let (data, csum) = data.split_at(data.len() - 4);
            let csum = u32::from_be_bytes([csum[0], csum[1], csum[2], csum[3]]);
            let hes = HueEntFrame::parse(data)?;
            Ok(Some(format!("{hes:x?} [PROXY, {csum:08x}]")))
        }
        _ => Ok(None),
    }
}
