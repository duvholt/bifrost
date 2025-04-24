use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::{ColorTemperatureUpdate, ColorUpdate, DimmingUpdate, On, ResourceLink, Stub};
use crate::legacy_api::ApiLightStateUpdate;
use crate::xy::XY;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GroupedLight {
    pub alert: Value,
    pub dimming: Option<DimmingUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Stub>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_temperature: Option<Stub>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_temperature_delta: Option<Stub>,
    #[serde(default)]
    pub dimming_delta: Stub,
    #[serde(default)]
    pub dynamics: Stub,
    pub on: Option<On>,
    pub owner: ResourceLink,
    pub signaling: Value,
}

impl GroupedLight {
    #[must_use]
    pub const fn new(room: ResourceLink) -> Self {
        Self {
            alert: Value::Null,
            dimming: None,
            color: Some(Stub),
            color_temperature: Some(Stub),
            color_temperature_delta: Some(Stub),
            dimming_delta: Stub,
            dynamics: Stub,
            on: None,
            owner: room,
            signaling: Value::Null,
        }
    }

    #[must_use]
    pub fn as_brightness_opt(&self) -> Option<f64> {
        self.dimming.as_ref().map(|br| br.brightness)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GroupedLightUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on: Option<On>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimming: Option<DimmingUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_temperature: Option<ColorTemperatureUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<ResourceLink>,
}

impl GroupedLightUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_brightness(self, brightness: Option<f64>) -> Self {
        Self {
            dimming: brightness.map(DimmingUpdate::new),
            ..self
        }
    }

    #[must_use]
    pub const fn with_on(self, on: Option<On>) -> Self {
        Self { on, ..self }
    }

    #[must_use]
    pub const fn with_color_temperature(self, mirek: Option<u16>) -> Self {
        Self {
            color_temperature: if let Some(ct) = mirek {
                Some(ColorTemperatureUpdate::new(ct))
            } else {
                None
            },
            ..self
        }
    }

    #[must_use]
    pub const fn with_color_xy(self, val: Option<XY>) -> Self {
        Self {
            color: if let Some(xy) = val {
                Some(ColorUpdate { xy })
            } else {
                None
            },
            ..self
        }
    }
}

/* conversion from v1 api */
impl From<&ApiLightStateUpdate> for GroupedLightUpdate {
    fn from(upd: &ApiLightStateUpdate) -> Self {
        Self::new()
            .with_on(upd.on.map(On::new))
            .with_brightness(upd.bri.map(|b| f64::from(b) / 2.54))
            .with_color_xy(upd.xy.map(XY::from))
            .with_color_temperature(upd.ct)
    }
}
