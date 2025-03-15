use packed_struct::PrimitiveEnum;

use crate::attr::{
    ZclCommand, ZclReadAttr, ZclReadAttrResp, ZclReportAttr, ZclWriteAttr, ZclWriteAttrResp,
};
use crate::error::ZclResult;
use crate::frame::ZclFrame;

pub fn describe(frame: &ZclFrame, data: &[u8]) -> ZclResult<Option<String>> {
    let cmd = ZclCommand::from_primitive(frame.cmd);
    let desc = match cmd {
        Some(ZclCommand::ReadAttrib) => {
            let req = ZclReadAttr::parse(data)?;
            Some(format!("Attr rd  -> {:04x?}", req.attr))
        }
        Some(ZclCommand::ReadAttribResp) => {
            let req = ZclReadAttrResp::parse(data)?;
            Some(format!("Attr rd <-  {:?}", req.attr))
        }
        Some(ZclCommand::WriteAttrib) => {
            let req = ZclWriteAttr::parse(data)?;
            Some(format!("Attr wr  -> {:?}", req.attr))
        }
        Some(ZclCommand::WriteAttribResp) => {
            let req = ZclWriteAttrResp::parse(data)?;
            Some(format!("Attr wr <-  {:02x?}", req.attr))
        }
        Some(ZclCommand::ReportAttrib) => {
            let req = ZclReportAttr::parse(data)?;
            Some(format!("Attr rp <-  {:02x?}", req.attr))
        }
        Some(ZclCommand::DefaultResp) => {
            /* let req = ZclDefaultResp::parse(data)?; */
            /* format!("Attr dr <-  {:02x} {:02x}", req.cmd, req.stat) */
            return Ok(Some(String::new()));
        }
        _ => None,
    };

    Ok(desc)
}
