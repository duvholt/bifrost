use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, fs::File, path::PathBuf};

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
    #[must_use]
    pub fn new(fixture_group: &FixtureGroup) -> Self {
        Self {
            id: fixture_group.id,
            friendly_name: fixture_group.friendly_name.to_string(),
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

#[derive(Debug)]
pub struct Z2mDeviceFixture {
    pub bridge_device: Value,
    pub initial_state: Option<Value>,
}

impl Z2mDeviceFixture {
    #[must_use]
    fn new(device_id: &FixtureDevice, mut bridge_device: Value) -> Self {
        bridge_device["friendly_name"] = Value::String(device_id.topic());
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureDevice {
    pub id: u16,
    pub friendly_name: &'static str,
}

impl FixtureDevice {
    #[must_use]
    pub fn ieee_address(&self) -> String {
        format!("0x020000000000{:04x}", self.id)
    }

    #[must_use]
    pub fn mac_address(&self) -> String {
        format!(
            "02:00:00:00:00:00:{}",
            self.id
                .to_be_bytes()
                .into_iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(":")
        )
    }

    #[must_use]
    pub const fn network_address(&self) -> u16 {
        self.id
    }

    #[must_use]
    pub fn topic(&self) -> String {
        self.friendly_name.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FixtureGroup {
    pub id: u32,
    pub friendly_name: &'static str,
}

impl FixtureGroup {
    #[must_use]
    pub fn topic(&self) -> String {
        self.friendly_name.to_string()
    }
}

fn json_file(filename: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("common")
        .join("fixtures")
        .join(filename);
    let file = File::open(path).unwrap_or_else(|_| panic!("unable to open file {filename}"));
    serde_json::from_reader(file).unwrap_or_else(|_| panic!("unable to parse file {filename}"))
}

#[allow(clippy::too_many_lines)]
pub mod ikea {
    use super::*;

    #[must_use]
    pub fn tradfri_warm_white(device_id: &FixtureDevice) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(device_id, json_file("ikea/warm_white.json"))
    }

    #[must_use]
    pub fn tradfri_color(device_id: &FixtureDevice) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(device_id, json_file("ikea/color.json"))
    }
}

#[allow(clippy::too_many_lines)]
pub mod hue {
    use super::*;

    #[must_use]
    pub fn white_ambience(device_id: &FixtureDevice) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(device_id, json_file("hue/white_ambiance.json"))
    }

    #[must_use]
    pub fn flux_lightstrip(device_id: &FixtureDevice) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(device_id, json_file("hue/flux_lightstrip.json"))
    }

    #[must_use]
    pub fn play_gradient_lightstrip(device_id: &FixtureDevice) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(device_id, json_file("hue/play_gradient_lightstrip.json"))
    }

    #[must_use]
    pub fn dimmer_switch(device_id: &FixtureDevice) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(device_id, json_file("hue/dimmer_switch_gen1.json"))
    }

    #[must_use]
    pub fn friends_of_hue_switch(device_id: &FixtureDevice) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(device_id, json_file("hue/friends_of_hue.json"))
    }
}

#[allow(clippy::too_many_lines)]
pub mod misc {
    use super::*;

    #[must_use]
    pub fn dimmer_light(device_id: &FixtureDevice) -> Z2mDeviceFixture {
        Z2mDeviceFixture::new(device_id, json_file("dimmer_light.json"))
    }
}
