use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use hue::api::{LightGradientUpdate, On};
use hue::xy::XY;

use crate::hexcolor::HexColor;

#[allow(clippy::pub_underscore_fields)]
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DeviceUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<DeviceState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_temp: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_mode: Option<DeviceColorMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<DeviceColor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Vec<HexColor>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linkquality: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_options: Option<ColorOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_temp_startup: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level_config: Option<LevelConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub elapsed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub power_on_behavior: Option<PowerOnBehavior>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default)]
    pub update: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_available: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub battery: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<f64>,

    /* all other fields */
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    #[serde(default, flatten)]
    pub __: HashMap<String, Value>,
}

impl DeviceUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_state(self, state: Option<bool>) -> Self {
        Self {
            state: state.map(|on| {
                if on {
                    DeviceState::On
                } else {
                    DeviceState::Off
                }
            }),
            ..self
        }
    }

    #[must_use]
    pub fn with_brightness(self, brightness: Option<f64>) -> Self {
        Self {
            brightness: brightness.map(|b| b.clamp(1.0, 254.0)),
            ..self
        }
    }

    #[must_use]
    pub fn with_color_temp(self, mirek: Option<u16>) -> Self {
        Self {
            color_temp: mirek,
            ..self
        }
    }

    #[must_use]
    pub fn with_color_xy(self, xy: Option<XY>) -> Self {
        Self {
            color: xy.map(DeviceColor::xy),
            ..self
        }
    }

    #[must_use]
    pub fn with_gradient(self, grad: Option<LightGradientUpdate>) -> Self {
        Self {
            gradient: grad.map(|g| {
                g.points
                    .iter()
                    .map(|p| {
                        let [r, g, b] = p.color.xy.to_rgb(255.0);
                        HexColor::new(r, g, b)
                    })
                    .collect()
            }),
            ..self
        }
    }

    #[must_use]
    pub fn with_transition(self, transition: Option<f64>) -> Self {
        Self { transition, ..self }
    }
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct DeviceColor {
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    h: Option<f64>,
    #[allow(dead_code)]
    #[serde(skip_serializing)]
    s: Option<f64>,

    pub hue: Option<f64>,
    pub saturation: Option<f64>,

    #[serde(flatten)]
    pub xy: Option<XY>,
}

impl DeviceColor {
    #[must_use]
    pub const fn xy(xy: XY) -> Self {
        Self {
            h: None,
            s: None,
            hue: None,
            saturation: None,
            xy: Some(xy),
        }
    }

    #[must_use]
    pub const fn hs(h: f64, s: f64) -> Self {
        Self {
            h: None,
            s: None,
            hue: Some(h),
            saturation: Some(s),
            xy: None,
        }
    }
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub enum PowerOnBehavior {
    #[default]
    Unknown,

    #[serde(rename = "on")]
    On,

    #[serde(rename = "off")]
    Off,

    #[serde(rename = "previous")]
    Previous,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ColorOptions {
    pub execute_if_off: bool,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct LevelConfig {
    pub execute_if_off: Option<bool>,
    pub on_off_transition_time: Option<u16>,
    pub on_transition_time: Option<u16>,
    pub off_transition_time: Option<u16>,
    pub current_level_startup: Option<CurrentLevelStartup>,
    pub on_level: Option<OnLevel>,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum CurrentLevelStartup {
    Previous,
    Minimum,
    #[serde(untagged)]
    Value(u8),
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum OnLevel {
    Previous,
    #[serde(untagged)]
    Value(u8),
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum DeviceColorMode {
    ColorTemp,
    Hs,
    Xy,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DeviceState {
    On,
    Off,
    Lock,
    Unlock,
}

impl From<DeviceState> for On {
    fn from(value: DeviceState) -> Self {
        Self {
            on: value == DeviceState::On,
        }
    }
}
