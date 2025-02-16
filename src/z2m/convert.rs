use std::collections::BTreeSet;

use hue::xy::XY;

use crate::hue::api::{
    ColorGamut, ColorTemperature, DeviceProductData, Dimming, GamutType, LightColor, LightGradient,
    LightGradientMode, MirekSchema,
};
use crate::hue::devicedb::{hardware_platform_type, product_archetype};
use crate::z2m::api::{Device, Expose, ExposeList, ExposeNumeric};

impl ExposeNumeric {
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn extract_mirek_schema(&self) -> Option<MirekSchema> {
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

impl LightColor {
    #[must_use]
    pub const fn extract_from_expose(expose: &Expose) -> Option<Self> {
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

impl LightGradient {
    #[must_use]
    pub fn extract_from_expose(expose: &ExposeList) -> Option<Self> {
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
                points_capable: *max,
                points: vec![],
                // FIXME: we don't have this information, so guesstimate it
                pixel_count: *max * 3,
            }),
            _ => None,
        }
    }
}

impl ColorTemperature {
    #[must_use]
    pub fn extract_from_expose(expose: &Expose) -> Option<Self> {
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

impl Dimming {
    #[must_use]
    pub const fn extract_from_expose(expose: &Expose) -> Option<Self> {
        let Expose::Numeric(_) = expose else {
            return None;
        };

        Some(Self {
            brightness: 0.01,
            min_dim_level: Some(0.01),
        })
    }
}

impl DeviceProductData {
    #[must_use]
    pub fn guess_from_device(dev: &Device) -> Self {
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
