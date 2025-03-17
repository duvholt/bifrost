use hue::api::ColorGamut;
use hue::zigbee::{GradientParams, GradientStyle, HueZigbeeUpdate};

use bifrost::error::ApiResult;

fn main() -> ApiResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let hz = HueZigbeeUpdate::new()
        .with_on_off(true)
        .with_brightness(0x20)
        .with_gradient_colors(
            GradientStyle::Linear,
            vec![ColorGamut::GAMUT_C.red, ColorGamut::GAMUT_C.red],
        )?
        .with_gradient_params(GradientParams {
            scale: 0x38,
            offset: 0x00,
        });

    println!("{}", hex::encode(&hz.to_vec()?));

    Ok(())
}
