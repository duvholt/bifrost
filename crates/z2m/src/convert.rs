use std::collections::BTreeSet;
use std::io::Cursor;

use hue::api::{
    ColorGamut, ColorTemperature, DeviceProductData, Dimming, DimmingDeltaAction, GamutType,
    GroupedLightUpdate, LightColor, LightGradient, LightGradientMode, LightGradientPoint,
    LightGradientUpdate, LightUpdate, MirekSchema,
};
use hue::devicedb::product_data;
use hue::error::HueError;
use hue::gradient::GradientProductData;
use hue::xy::XY;
use hue::zigbee::HueZigbeeUpdate;

use crate::api::{Device, Expose, ExposeList, ExposeNumeric};
use crate::update::{DeviceColorMode, DeviceUpdate};

pub trait ExtractExposeNumeric {
    fn extract_mirek_schema(&self) -> Option<MirekSchema>;
}

impl ExtractExposeNumeric for ExposeNumeric {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn extract_mirek_schema(&self) -> Option<MirekSchema> {
        if self.unit.as_deref() == Some("mired") {
            if let (Some(min), Some(max)) = (self.value_min, self.value_max) {
                return Some(MirekSchema {
                    mirek_minimum: min as u32,
                    mirek_maximum: max as u32,
                });
            }
        }
        None
    }
}

pub trait ExtractLightColor {
    #[must_use]
    fn extract_from_expose(expose: &Expose) -> Option<Self>
    where
        Self: Sized;
}

impl ExtractLightColor for LightColor {
    fn extract_from_expose(expose: &Expose) -> Option<Self> {
        let Expose::Composite(_) = expose else {
            return None;
        };

        Some(Self {
            gamut: Some(ColorGamut::GAMUT_C),
            gamut_type: GamutType::C,
            xy: XY::D65_WHITE_POINT,
        })
    }
}

pub trait ExtractLightGradient {
    #[must_use]
    fn extract_from_expose(
        expose: &ExposeList,
        gradient_product_data: &GradientProductData,
    ) -> Option<Self>
    where
        Self: Sized;
}

impl ExtractLightGradient for LightGradient {
    fn extract_from_expose(
        expose: &ExposeList,
        gradient_product_data: &GradientProductData,
    ) -> Option<Self> {
        match expose {
            ExposeList {
                length_max: Some(max),
                ..
            } => Some(Self {
                mode: LightGradientMode::InterpolatedPalette,
                mode_values: BTreeSet::from([
                    LightGradientMode::InterpolatedPalette,
                    LightGradientMode::InterpolatedPaletteMirrored,
                    LightGradientMode::RandomPixelated,
                ]),
                points_capable: *max.min(&5),
                points: vec![],
                pixel_count: gradient_product_data.pixel_count,
            }),
            _ => None,
        }
    }
}

pub trait ExtractColorTemperature: Sized {
    #[must_use]
    fn extract_from_expose(expose: &Expose) -> Option<Self>;
}

impl ExtractColorTemperature for ColorTemperature {
    fn extract_from_expose(expose: &Expose) -> Option<Self> {
        let Expose::Numeric(num) = expose else {
            return None;
        };

        let schema_opt = num.extract_mirek_schema();
        let mirek_valid = schema_opt.is_some();
        let mirek_schema = schema_opt.unwrap_or(MirekSchema::DEFAULT);
        let mirek = None;

        Some(Self {
            mirek,
            mirek_schema,
            mirek_valid,
        })
    }
}

pub trait ExtractDimming: Sized {
    #[must_use]
    fn extract_from_expose(expose: &Expose) -> Option<Self>;
}

impl ExtractDimming for Dimming {
    fn extract_from_expose(expose: &Expose) -> Option<Self> {
        let Expose::Numeric(_) = expose else {
            return None;
        };

        Some(Self {
            brightness: 0.01,
            min_dim_level: Some(0.01),
        })
    }
}

pub trait ExtractDeviceProductData {
    #[must_use]
    fn guess_from_device(dev: &Device) -> Self;
}

impl ExtractDeviceProductData for DeviceProductData {
    fn guess_from_device(dev: &Device) -> Self {
        fn str_or_unknown(name: Option<&str>) -> String {
            name.map_or("<unknown>", |v| v).to_string()
        }

        let dev_model_id = str_or_unknown(dev.model_id.as_deref());

        let product_data: Option<hue::devicedb::SimpleProductData<'_>> =
            product_data(&dev_model_id);

        let model_id = str_or_unknown(
            product_data
                .as_ref()
                .and_then(|p| p.model_id)
                .or(dev.model_id.as_deref()),
        );

        let product_name = str_or_unknown(
            product_data
                .as_ref()
                .map(|p| p.product_name)
                .or_else(|| dev.definition.as_ref().map(|def| def.model.as_str())),
        );

        let manufacturer_name = str_or_unknown(
            product_data
                .as_ref()
                .map(|p| p.manufacturer_name)
                .or(dev.manufacturer.as_deref()),
        );
        let certified = manufacturer_name == Self::SIGNIFY_MANUFACTURER_NAME;
        let software_version = dev
            .software_build_id
            .as_deref()
            .unwrap_or("0.0.0")
            .to_string();

        let product_archetype = product_data
            .as_ref()
            .map(|p| p.product_archetype.clone())
            .unwrap_or_default();
        let hardware_platform_type = product_data
            .as_ref()
            .and_then(|p| p.hardware_platform_type)
            .map(ToString::to_string);

        Self {
            model_id,
            manufacturer_name,
            product_name,
            product_archetype,
            certified,
            software_version,
            hardware_platform_type,
        }
    }
}

impl From<&DeviceUpdate> for LightUpdate {
    fn from(value: &DeviceUpdate) -> Self {
        if let Some(philips_raw) = &value.philips_raw {
            match hex::decode(&philips_raw)
                .map_err(HueError::from)
                .and_then(|data| {
                    let mut cur = Cursor::new(data);
                    HueZigbeeUpdate::from_reader(&mut cur)
                }) {
                Ok(hz) => {
                    let upd = hz.into();
                    log::trace!(
                        "Converted Philips raw update to light update {philips_raw} {upd:#?}"
                    );
                    return upd;
                }
                Err(err) => {
                    log::error!(
                        "Failed to parse Philips Hue raw update {philips_raw}: {err}. Falling back to using z2m data"
                    );
                }
            }
        }

        let mut upd = Self::new()
            .with_on(value.state.map(Into::into))
            .with_brightness(value.brightness.map(|b| b / 254.0 * 100.0))
            .with_color_temperature(value.color_temp)
            .with_gradient(value.gradient.as_ref().map(|s| {
                LightGradientUpdate {
                    mode: None,
                    points: s
                        .iter()
                        .map(|hc| LightGradientPoint::xy(hc.to_xy_color()))
                        .collect(),
                }
            }));

        if value.color_mode != Some(DeviceColorMode::ColorTemp) {
            upd = upd.with_color_xy(value.color.and_then(|col| col.xy));
        }

        upd
    }
}

impl From<&GroupedLightUpdate> for DeviceUpdate {
    fn from(upd: &GroupedLightUpdate) -> Self {
        Self::default()
            .with_state(upd.on.map(|on| on.on))
            .with_brightness(upd.dimming.map(|dim| dim.brightness / 100.0 * 254.0))
            .with_brightness_step(upd.dimming_delta.map(|dim_delta| {
                let brightness = dim_delta.brightness_delta / 100.0 * 254.0;
                match dim_delta.action {
                    DimmingDeltaAction::Up => brightness,
                    DimmingDeltaAction::Down => -brightness,
                }
            }))
            .with_color_temp(upd.color_temperature.and_then(|ct| ct.mirek))
            .with_color_xy(upd.color.map(|col| col.xy))
            .with_transition(
                upd.dynamics
                    .as_ref()
                    .and_then(|d| d.duration.map(|duration| f64::from(duration) / 1000.0)),
            )
    }
}
