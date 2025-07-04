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

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct GroupedLightDynamicsUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
}

impl GroupedLightDynamicsUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_duration(self, duration: Option<impl Into<u32>>) -> Self {
        Self {
            duration: duration.map(Into::into),
            ..self
        }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamics: Option<GroupedLightDynamicsUpdate>,
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
    pub fn with_on(self, on: impl Into<Option<On>>) -> Self {
        Self {
            on: on.into(),
            ..self
        }
    }

    #[must_use]
    pub fn with_color_temperature(self, mirek: impl Into<Option<u16>>) -> Self {
        Self {
            color_temperature: mirek.into().map(ColorTemperatureUpdate::new),
            ..self
        }
    }

    #[must_use]
    pub fn with_color_xy(self, val: impl Into<Option<XY>>) -> Self {
        Self {
            color: val.into().map(ColorUpdate::new),
            ..self
        }
    }

    #[must_use]
    pub const fn with_dynamics(self, dynamics: Option<GroupedLightDynamicsUpdate>) -> Self {
        Self { dynamics, ..self }
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
            .with_dynamics(
                upd.transitiontime
                    .map(|t| GroupedLightDynamicsUpdate::new().with_duration(Some(t * 100))),
            )
    }
}
