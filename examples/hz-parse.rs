#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
use std::io::{stdin, BufRead, Cursor};

use itertools::Itertools;
use log::warn;
use packed_struct::PrimitiveEnumDynamicStr;

use hue::zigbee::{Flags, GradientColors, HueZigbeeUpdate};

use bifrost::error::ApiResult;

#[must_use]
pub fn present_gradcolors(grad: &GradientColors) -> String {
    let mut res = format!(
        "{}-{}-{}-{:<9}",
        grad.header.nlights,
        grad.header.resv0,
        grad.header.resv2,
        grad.header.style.to_display_str(),
    );
    for p in &grad.points {
        let x = (p.x * 1000.0) as u32;
        let y = (p.y * 1000.0) as u32;
        res += &format!(" {x:03}.{y:03}");
    }
    res
}

fn show(data: &[u8]) -> ApiResult<()> {
    let flags = Flags::from_bits(u16::from(data[0]) | (u16::from(data[1]) << 8)).unwrap();

    let mut cur = Cursor::new(data);
    let hz = HueZigbeeUpdate::from_reader(&mut cur)?;

    let desc = format!(
        " {:04x} : {:2} : {:2} : {:4} : {:11} : {:<10} : {:2} : {:<55} : {:4} : {:4} ",
        flags.bits(),
        hz.onoff.map(|x| format!("{x:02x}")).unwrap_or_default(),
        hz.brightness
            .map(|x| format!("{x:02x}"))
            .unwrap_or_default(),
        hz.color_mirek
            .map(|x| format!("{x:04x}"))
            .unwrap_or_default(),
        hz.color_xy
            .map(|xy| { format!("{:.3},{:.3}", xy.x, xy.y) })
            .unwrap_or_default(),
        hz.effect_type
            .map(|x| x.to_display_str())
            .unwrap_or_default(),
        hz.effect_speed
            .map(|x| format!("{x:02x}"))
            .unwrap_or_default(),
        hz.gradient_colors
            .as_ref()
            .map(present_gradcolors)
            .unwrap_or_default(),
        hz.gradient_params
            .map(|gt| format!("{:02x}{:02x}", gt.scale, gt.offset))
            .unwrap_or_default(),
        hz.fade_speed
            .map(|x| format!("{x:04x}"))
            .unwrap_or_default(),
    );

    println!(
        "|{desc}|   {} {}",
        hex::encode(cur.fill_buf()?),
        flags.iter_names().map(|(name, _)| name).join(" | ")
    );

    Ok(())
}

fn check(orig: &[u8]) -> ApiResult<()> {
    let mut cur = Cursor::new(orig);
    let hz = HueZigbeeUpdate::from_reader(&mut cur)?;

    let mut dest = Cursor::new(vec![]);
    hz.serialize(&mut dest)?;

    let data = dest.into_inner();

    if orig != data {
        warn!("DIFF:");
        warn!("  {} before", hex::encode(orig));
        warn!("  {} after", hex::encode(&data));
    };

    Ok(())
}

fn main() -> ApiResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    eprintln!(
        "| flag | on | br | mrek | (colx,coly) | effect ty. | es | gradient data                                           | grad | fade |"
    );

    for line in stdin().lines() {
        let line = line?;
        let data = hex::decode(&line)?;
        println!("==================== {line:<40}");
        show(&data)?;
        check(&data)?;
    }

    Ok(())
}
