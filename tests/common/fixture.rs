use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub struct Z2mFixture {
    devices: Vec<Z2mDeviceFixture>,
    groups: Vec<Z2mGroupFixture>,
}

impl Z2mFixture {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            groups: Vec::new(),
        }
    }

    #[must_use]
    pub fn into_state(self) -> BTreeMap<String, Value> {
        let Self { devices, groups } = self;
        let mut z2m_state = BTreeMap::new();

        for device in &devices {
            if let Some(device_state) = &device.initial_state {
                z2m_state.insert(device.friendly_name().to_string(), device_state.clone());
            }
        }
        for group in &groups {
            if let Some(group_state) = &group.initial_state {
                z2m_state.insert(group.friendly_name.clone(), group_state.clone());
            }
        }
        z2m_state.insert(
            "bridge/devices".to_string(),
            Value::Array(devices.into_iter().map(|d| d.bridge_device).collect()),
        );

        z2m_state.insert(
            "bridge/groups".to_string(),
            Value::Array(
                groups
                    .into_iter()
                    .map(|g| serde_json::to_value(g).unwrap())
                    .collect(),
            ),
        );
        z2m_state
    }

    #[must_use]
    pub fn with_device(mut self, device: Z2mDeviceFixture) -> Self {
        self.devices.push(device);
        self
    }

    #[must_use]
    pub fn with_group(mut self, group: Z2mGroupFixture) -> Self {
        self.groups.push(group);
        self
    }
}

impl Default for Z2mFixture {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Z2mGroupFixture {
    pub id: u32,
    pub friendly_name: String,
    pub members: Vec<Z2mGroupMember>,
    pub scenes: Vec<Z2mSceneFixture>,

    #[serde(skip)]
    pub initial_state: Option<Value>,
}
impl Z2mGroupFixture {
    pub fn new(id: u32, friendly_name: impl Into<String>) -> Self {
        Self {
            id,
            friendly_name: friendly_name.into(),
            members: Vec::new(),
            scenes: Vec::new(),
            initial_state: None,
        }
    }

    #[must_use]
    pub fn with_member(mut self, device: &Z2mDeviceFixture) -> Self {
        self.members.push(Z2mGroupMember::new(device, 1));
        self
    }

    #[must_use]
    pub fn with_scene(mut self, id: u32, name: impl Into<String>) -> Self {
        self.scenes.push(Z2mSceneFixture {
            id,
            name: name.into(),
        });
        self
    }

    #[must_use]
    pub fn with_state(mut self, state: Value) -> Self {
        self.initial_state = Some(state);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Z2mGroupMember {
    pub ieee_address: String,
    pub endpoint: u8,
}

impl Z2mGroupMember {
    #[must_use]
    pub fn new(device: &Z2mDeviceFixture, endpoint: u8) -> Self {
        Self {
            ieee_address: device.ieee_address().to_owned(),
            endpoint,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Z2mSceneFixture {
    pub id: u32,
    pub name: String,
}

pub struct Z2mDeviceFixture {
    pub bridge_device: Value,
    pub initial_state: Option<Value>,
}

impl Z2mDeviceFixture {
    #[must_use]
    fn new(device_id: Z2mFixtureDeviceId, mut bridge_device: Value) -> Self {
        bridge_device["ieee_address"] = Value::String(device_id.ieee_address());
        bridge_device["network_address"] = Value::Number(device_id.network_address().into());
        Self {
            bridge_device,
            initial_state: None,
        }
    }

    #[must_use]
    pub fn with_state(mut self, state: Value) -> Self {
        self.initial_state = Some(state);
        self
    }

    #[must_use]
    pub fn friendly_name(&self) -> &str {
        self.bridge_device["friendly_name"].as_str().unwrap()
    }

    #[must_use]
    pub fn ieee_address(&self) -> &str {
        self.bridge_device["ieee_address"].as_str().unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Z2mFixtureDeviceId(pub u16);

impl Z2mFixtureDeviceId {
    #[must_use]
    pub fn ieee_address(self) -> String {
        format!("0x020000000000{:04x}", self.0)
    }

    #[must_use]
    pub const fn network_address(self) -> u16 {
        self.0
    }
}

#[allow(clippy::too_many_lines)]
pub mod ikea {
    use super::*;
    use serde_json::json;

    #[must_use]
    pub fn tradfri_warm_white(
        device_id: Z2mFixtureDeviceId,
        friendly_name: &str,
    ) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(
            device_id,
            json!(
                {
                "date_code": "20240226",
                "definition": {
                    "description": "TRADFRI bulb E26/E27, warm white, globe, 806/810 lumen",
                    "exposes": [
                        {
                            "features": [
                                {
                                    "access": 7,
                                    "description": "On/off state of this light",
                                    "label": "State",
                                    "name": "state",
                                    "property": "state",
                                    "type": "binary",
                                    "value_off": "OFF",
                                    "value_on": "ON",
                                    "value_toggle": "TOGGLE"
                                },
                                {
                                    "access": 7,
                                    "description": "Brightness of this light",
                                    "label": "Brightness",
                                    "name": "brightness",
                                    "property": "brightness",
                                    "type": "numeric",
                                    "value_max": 254,
                                    "value_min": 0
                                },
                                {
                                    "access": 7,
                                    "description": "Configure genLevelCtrl",
                                    "features": [
                                        {
                                            "access": 7,
                                            "description": "this setting can affect the \"on_level\", \"current_level_startup\" or \"brightness\" setting",
                                            "label": "Execute if off",
                                            "name": "execute_if_off",
                                            "property": "execute_if_off",
                                            "type": "binary",
                                            "value_off": false,
                                            "value_on": true
                                        },
                                        {
                                            "access": 7,
                                            "description": "Defines the desired startup level for a device when it is supplied with power",
                                            "label": "Current level startup",
                                            "name": "current_level_startup",
                                            "presets": [
                                                {
                                                    "description": "Use minimum permitted value",
                                                    "name": "minimum",
                                                    "value": "minimum"
                                                },
                                                {
                                                    "description": "Use previous value",
                                                    "name": "previous",
                                                    "value": "previous"
                                                }
                                            ],
                                            "property": "current_level_startup",
                                            "type": "numeric",
                                            "value_max": 254,
                                            "value_min": 1
                                        }
                                    ],
                                    "label": "Level config",
                                    "name": "level_config",
                                    "property": "level_config",
                                    "type": "composite"
                                }
                            ],
                            "type": "light"
                        },
                        {
                            "access": 2,
                            "description": "Triggers an effect on the light (e.g. make light blink for a few seconds)",
                            "label": "Effect",
                            "name": "effect",
                            "property": "effect",
                            "type": "enum",
                            "values": [
                                "blink",
                                "breathe",
                                "okay",
                                "channel_change",
                                "finish_effect",
                                "stop_effect"
                            ]
                        },
                        {
                            "access": 7,
                            "category": "config",
                            "description": "Controls the behavior when the device is powered on after power loss",
                            "label": "Power-on behavior",
                            "name": "power_on_behavior",
                            "property": "power_on_behavior",
                            "type": "enum",
                            "values": [
                                "off",
                                "on",
                                "toggle",
                                "previous"
                            ]
                        },
                        {
                            "access": 2,
                            "category": "config",
                            "description": "Initiate device identification",
                            "label": "Identify",
                            "name": "identify",
                            "property": "identify",
                            "type": "enum",
                            "values": [
                                "identify"
                            ]
                        },
                        {
                            "access": 1,
                            "category": "diagnostic",
                            "description": "Link quality (signal strength)",
                            "label": "Linkquality",
                            "name": "linkquality",
                            "property": "linkquality",
                            "type": "numeric",
                            "unit": "lqi",
                            "value_max": 255,
                            "value_min": 0
                        }
                    ],
                    "model": "LED2103G5",
                    "options": [
                        {
                            "access": 2,
                            "description": "Controls the transition time (in seconds) of on/off, brightness, color temperature (if applicable) and color (if applicable) changes. Defaults to `0` (no transition).",
                            "label": "Transition",
                            "name": "transition",
                            "property": "transition",
                            "type": "numeric",
                            "value_min": 0,
                            "value_step": 0.1
                        },
                        {
                            "access": 2,
                            "description": "Sets the duration of the identification procedure in seconds (i.e., how long the device would flash).The value ranges from 1 to 30 seconds (default: 3).",
                            "label": "Identify timeout",
                            "name": "identify_timeout",
                            "property": "identify_timeout",
                            "type": "numeric",
                            "value_max": 30,
                            "value_min": 1
                        },
                        {
                            "access": 2,
                            "description": "State actions will also be published as 'action' when true (default false).",
                            "label": "State action",
                            "name": "state_action",
                            "property": "state_action",
                            "type": "binary",
                            "value_off": false,
                            "value_on": true
                        }
                    ],
                    "source": "native",
                    "supports_ota": true,
                    "vendor": "IKEA",
                    "version": "0.0.0"
                },
                "disabled": false,
                "endpoints": {
                    "1": {
                        "bindings": [],
                        "clusters": {
                            "input": [
                                "genBasic",
                                "genIdentify",
                                "genGroups",
                                "genScenes",
                                "genOnOff",
                                "genLevelCtrl",
                                "touchlink",
                                "manuSpecificIkeaUnknown"
                            ],
                            "output": [
                                "genOta"
                            ]
                        },
                        "configured_reportings": [],
                        "scenes": []
                    },
                    "242": {
                        "bindings": [],
                        "clusters": {
                            "input": [
                                "greenPower"
                            ],
                            "output": [
                                "greenPower"
                            ]
                        },
                        "configured_reportings": [],
                        "scenes": []
                    }
                },
                "friendly_name": friendly_name,
                "ieee_address": "",
                "interview_completed": true,
                "interview_state": "SUCCESSFUL",
                "interviewing": false,
                "manufacturer": "IKEA of Sweden",
                "model_id": "TRADFRI bulb E27 WW globe 806lm",
                "network_address": 0,
                "power_source": "Mains (single phase)",
                "software_build_id": "1.0.42",
                "supported": true,
                "type": "Router"
            }),
        )
    }
}
