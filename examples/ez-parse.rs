#![allow(clippy::cast_possible_truncation, clippy::match_same_arms)]

use std::fmt::Debug;
use std::io::{stdin, Cursor};

use serde::Deserialize;

use bifrost::error::ApiResult;
use zcl::cluster;
use zcl::error::ZclResult;
use zcl::frame::{ZclFrame, ZclFrameType};

#[macro_use]
extern crate log;

pub fn f64_str<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use std::str::FromStr;
    let s = String::deserialize(deserializer)?;
    f64::from_str(&s).map_err(serde::de::Error::custom)
}

pub fn u16_hex<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(u16::from_be(
        u16::from_str_radix(&s, 16).map_err(serde::de::Error::custom)?,
    ))
}

pub fn u16_hex_opt<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    if let Some(s) = opt {
        Ok(Some(u16::from_be(
            u16::from_str_radix(&s, 16).map_err(serde::de::Error::custom)?,
        )))
    } else {
        Ok(None)
    }
}

pub fn vec_hex_opt<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    if let Some(s) = opt {
        Ok(hex::decode(s).map_err(serde::de::Error::custom)?)
    } else {
        Ok(vec![])
    }
}

#[derive(Debug, Deserialize)]
pub struct Record {
    /* pub src_mac: String, */
    /* pub cmd: Option<String>, */
    #[serde(deserialize_with = "f64_str")]
    pub time: f64,

    pub index: u64,

    #[serde(deserialize_with = "u16_hex")]
    pub src: u16,

    #[serde(deserialize_with = "u16_hex")]
    pub dst: u16,

    #[serde(deserialize_with = "u16_hex")]
    pub cluster: u16,

    #[serde(deserialize_with = "vec_hex_opt")]
    pub data: Vec<u8>,
}

fn parse(rec: &Record) -> ZclResult<()> {
    if rec.data.is_empty() {
        return Ok(());
    }

    let mut cur = Cursor::new(&rec.data);
    let frame = ZclFrame::parse(&mut cur)?;

    let data = &rec.data[cur.position() as usize..];

    let src = hex::encode(rec.src.to_be_bytes());
    let dst = hex::encode(rec.dst.to_be_bytes());
    let flags = frame.flags;
    let cmd = frame.cmd;
    let cls = rec.cluster;
    let index = rec.index;

    let describe = |cat: &str, desc: ZclResult<Option<String>>| {
        match desc {
            Ok(Some(desc)) => {
                if desc.is_empty() {
                    return;
                }

                info!(
                    "[{index:6}] [{src} -> {dst}] {flags:?} [{cls:04x}] {cmd:02x} :: {cat}{desc} {}",
                    hex::encode(data)
                );
            }
            Ok(None) => {
                warn!(
                    "[{index:6}] [{src} -> {dst}] {flags:?} [{cls:04x}] {cmd:02x} :: {cat}Unknown {}",
                    hex::encode(data)
                );
            }
            Err(err) => {
                error!(
                    "[{index:6}] [{src} -> {dst}] {flags:?} [{cls:04x}] {cmd:02x} :: FAILED {}: {err}",
                    hex::encode(data)
                );
            }
        };
    };

    if frame.flags.frame_type == ZclFrameType::ProfileWide {
        describe("", cluster::standard::describe(&frame, data));
        return Ok(());
    }

    match rec.cluster {
        0x0003 => describe("Effect:", Ok(cluster::effects::describe(&frame, data))),
        0x0004 => describe("Group:", Ok(cluster::groups::describe(&frame, data))),
        0x0005 => describe("Scene:", Ok(cluster::scenes::describe(&frame, data))),
        0x0006 => describe("OnOff:", Ok(cluster::onoff::describe(&frame, data))),
        0x0008 => describe("LevelCtrl:", Ok(cluster::levelctrl::describe(&frame, data))),

        0x0019 => {
            // suppress OTA messages
        }

        0x0406 => {
            // suppress occypancy sensing
        }

        0x1000 => describe(
            "Commissioning:",
            cluster::commissioning::describe(&frame, data),
        ),

        0xFC01 => describe("HueEnt:", cluster::hue_fc01::describe(&frame, data)),
        0xFC03 => describe("HueCmp:", cluster::hue_fc03::describe(&frame, data)),

        _ => describe("UNKNOWN:", Ok(None)),
    }

    Ok(())
}

fn main() -> ApiResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    for line in stdin().lines() {
        let line = line?;

        match serde_json::from_str::<Record>(line.trim()) {
            Ok(data) => {
                if let Err(err) = parse(&data) {
                    error!("Failed parse: {err}");
                    eprintln!("    {line:<40}");
                    eprintln!("    {data:?}");
                }
            }

            Err(err) => {
                error!("Failed to parse json: {err}");
                eprintln!("    {line:<40}");
            }
        }
    }

    Ok(())
}
