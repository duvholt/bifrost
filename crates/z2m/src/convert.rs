use std::collections::BTreeSet;

use hue::api::{
    ColorGamut, ColorTemperature, DeviceProductData, Dimming, GamutType, GroupedLightUpdate,
    LightColor, LightGradient, LightGradientMode, LightGradientPoint, LightGradientUpdate,
    LightUpdate, MirekSchema,
};
use hue::devicedb::{hardware_platform_type, product_archetype};
use hue::xy::XY;

use crate::api::{Device, Expose, ExposeList, ExposeNumeric};
use crate::update::{DeviceColorMode, DeviceUpdate};

pub trait ExtractExposeNumeric {
    fn extract_mirek_schema(&self) -> Option<MirekSchema>;
}

impl ExtractExposeNumeric for ExposeNumeric {
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn extract_mirek_schema(&self) -> Option<MirekSchema> {
        let is_mired = self.unit.as_deref() == Some("mired")
            || self.base.name.as_deref() == Some("color_temp")
            || self.base.name.as_deref() == Some("color_temperature");

        if is_mired {
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
    fn extract_from_expose(expose: &ExposeList) -> Option<Self>
    where
        Self: Sized;
}

impl ExtractLightGradient for LightGradient {
    #[must_use]
    fn extract_from_expose(expose: &ExposeList) -> Option<Self> {
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
                pixel_count: *max.min(&7),
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
    #[must_use]
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

pub trait ExtractLightEffects: Sized {
    #[must_use]
    fn extract_from_expose(expose: &Expose) -> Option<Self>;
}

impl ExtractLightEffects for hue::api::LightEffects {
    #[must_use]
    fn extract_from_expose(expose: &Expose) -> Option<Self> {
        let Expose::Enum(enum_exp) = expose else {
            return None;
        };

        let mut effects = Vec::new();
        effects.push(hue::api::LightEffect::NoEffect);

        for val in &enum_exp.values {
            if let Some(s) = val.as_str() {
                match s {
                    "blink" => effects.push(hue::api::LightEffect::Sparkle),
                    "breathe" => effects.push(hue::api::LightEffect::Opal),
                    "okay" => effects.push(hue::api::LightEffect::Glisten),
                    "channel_change" => effects.push(hue::api::LightEffect::Prism),
                    "colorloop" => effects.push(hue::api::LightEffect::Prism),
                    "finish_effect" | "stop_effect" => {},
                    _ => {}
                }
            }
        }

        if effects.len() > 1 {
            effects.sort();
            effects.dedup();
            Some(Self {
                status_values: effects.clone(),
                status: hue::api::LightEffect::NoEffect,
                effect_values: effects,
            })
        } else {
            None
        }
    }
}

pub trait ExtractLightEffectsV2: Sized {
    #[must_use]
    fn extract_from_expose(expose: &Expose) -> Option<Self>;
}

impl ExtractLightEffectsV2 for hue::api::LightEffectsV2 {
    #[must_use]
    fn extract_from_expose(expose: &Expose) -> Option<Self> {
        let effects_v1 = hue::api::LightEffects::extract_from_expose(expose)?;
        Some(Self {
            action: hue::api::LightEffectValues {
                effect_values: effects_v1.effect_values.clone(),
            },
            status: hue::api::LightEffectStatus {
                effect: hue::api::LightEffect::NoEffect,
                effect_values: effects_v1.effect_values,
                parameters: None,
            },
        })
    }
}

impl ExtractDimming for Dimming {
    #[must_use]
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
    #[must_use]
    fn guess_from_device(dev: &Device) -> Self {
        fn str_or_unknown(name: Option<&String>) -> String {
            name.map_or("<unknown>", |v| v).to_string()
        }

        let product_name = str_or_unknown(dev.definition.as_ref().map(|def| &def.model));
        let model_id = str_or_unknown(dev.model_id.as_ref());
        let manufacturer_name = str_or_unknown(dev.manufacturer.as_ref());
        let certified = manufacturer_name == Self::SIGNIFY_MANUFACTURER_NAME;
        let software_version = str_or_unknown(dev.software_build_id.as_ref());

        let product_archetype = product_archetype(&model_id).unwrap_or_default();
        let hardware_platform_type = hardware_platform_type(&model_id).map(ToString::to_string);

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
            .with_color_temp(upd.color_temperature.and_then(|ct| ct.mirek))
            .with_color_xy(upd.color.map(|col| col.xy))
            .with_transition(
                upd.dynamics
                    .as_ref()
                    .and_then(|d| d.duration.map(|duration| f64::from(duration) / 1000.0)),
            )
    }
}
