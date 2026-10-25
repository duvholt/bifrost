use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use hue::api::{LightGradientUpdate, On};
use hue::xy::XY;

use crate::hexcolor::HexColor;

#[allow(clippy::pub_underscore_fields)]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct DeviceUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<DeviceState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness_step: Option<f64>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<DeviceEffect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub philips_raw: Option<String>,

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
    pub fn with_brightness_step(self, brightness_step: Option<f64>) -> Self {
        Self {
            brightness_step: brightness_step.map(|b| b.clamp(-254.0, 254.0)),
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
    pub fn with_effect(self, effect: DeviceEffect) -> Self {
        Self {
            effect: Some(effect),
            ..self
        }
    }

    #[must_use]
    pub fn with_transition(self, transition: Option<f64>) -> Self {
        Self { transition, ..self }
    }
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub enum PowerOnBehavior {
    #[serde(rename = "on")]
    On,

    #[serde(rename = "off")]
    Off,

    #[serde(rename = "previous")]
    Previous,

    #[serde(rename = "toggle")]
    Toggle,

    #[serde(untagged)]
    Unknown(String),
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ColorOptions {
    pub execute_if_off: bool,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LevelConfig {
    pub execute_if_off: Option<bool>,
    pub on_off_transition_time: Option<f64>,
    pub on_transition_time: Option<f64>,
    pub off_transition_time: Option<f64>,
    pub current_level_startup: Option<CurrentLevelStartup>,
    pub on_level: Option<OnLevel>,
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CurrentLevelStartup {
    Previous,
    Minimum,
    #[serde(untagged)]
    Value(u8),
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OnLevel {
    Previous,
    #[serde(untagged)]
    Value(u8),
}

#[derive(Copy, Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceEffect {
    Blink,
    Breathe,
    Okay,
    ChannelChange,
    FinishEffect,
    StopEffect,
    None,
    #[serde(untagged)]
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use hue::xy::XY;
    use serde_json::json;

    use super::{
        CurrentLevelStartup, DeviceState, DeviceUpdate, LevelConfig, OnLevel, PowerOnBehavior,
    };

    #[test]
    fn ikea_color_report() {
        let payload = r#"
            {
                "brightness": 4,
                "color": {
                    "h": 20, "hue": 20, "s": 98, "saturation": 98,
                    "x": 0.561, "y": 0.4042
                },
                "color_mode": "xy",
                "color_options": {"execute_if_off": true},
                "color_temp": 454,
                "color_temp_startup": 418,
                "effect": null,
                "identify": null,
                "level_config": {
                    "current_level_startup": "minimum",
                    "execute_if_off": true,
                    "on_level": "previous",
                    "on_off_transition_time": 0.5
                },
                "linkquality": 172,
                "power_on_behavior": "toggle",
                "state": "ON",
                "update": {
                    "installed_version": 16777284,
                    "latest_release_notes": "https://ww8.ikea.com/ikeahomesmart/releasenotes/releasenotes.html",
                    "latest_source": "https://raw.githubusercontent.com/Koenkk/zigbee-OTA/master/images/IKEA/zingo_cws-0x2805-1.0.44-prod.ota.ota.signed",
                    "latest_version": 16777284,
                    "state": "idle"
                }
            }
        "#;
        let update: DeviceUpdate = serde_json::from_str(payload).unwrap();

        assert_eq!(update.state, Some(DeviceState::On));
        assert_eq!(update.brightness, Some(4.0));
        assert_eq!(update.power_on_behavior, Some(PowerOnBehavior::Toggle));
        assert_eq!(update.color.unwrap().xy, Some(XY::new(0.561, 0.4042)));
        assert_eq!(update.color_temp, Some(454));

        let level = update.level_config.unwrap();
        assert_eq!(
            level.current_level_startup,
            Some(CurrentLevelStartup::Minimum)
        );
        assert_eq!(level.on_level, Some(OnLevel::Previous));
        assert_eq!(level.on_off_transition_time, Some(0.5));
    }

    #[test]
    fn level_config() {
        let level: LevelConfig = serde_json::from_value(json!({
            "current_level_startup": 190,
            "execute_if_off": true,
            "on_level": "previous",
            "on_off_transition_time": 0.5,
            "on_transition_time": 1.5,
            "off_transition_time": 2.5
        }))
        .unwrap();

        assert_eq!(
            level.current_level_startup,
            Some(CurrentLevelStartup::Value(190))
        );
        assert_eq!(level.execute_if_off, Some(true));
        assert_eq!(level.on_level, Some(OnLevel::Previous));
        assert_eq!(level.on_off_transition_time, Some(0.5));
        assert_eq!(level.on_transition_time, Some(1.5));
        assert_eq!(level.off_transition_time, Some(2.5));
    }

    #[test]
    fn unknown_power_on_behavior() {
        let update: DeviceUpdate =
            serde_json::from_str(r#"{"power_on_behavior":"restore"}"#).unwrap();

        assert_eq!(
            update.power_on_behavior,
            Some(PowerOnBehavior::Unknown("restore".to_owned()))
        );
    }
}
