use std::collections::BTreeMap;
use std::io::Read;

use serde::{Deserialize, Serialize};
use serde_yml::Value;
use uuid::Uuid;

use hue::api::{DeviceArchetype, Resource};
use hue::error::{HueError, HueResult};
use hue::version::SwVersion;

use crate::error::{ApiError, ApiResult};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AuxData {
    pub topic: Option<String>,
    pub index: Option<u32>,
}

impl AuxData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_topic(self, topic: &str) -> Self {
        Self {
            topic: Some(topic.to_string()),
            ..self
        }
    }

    #[must_use]
    pub fn with_index(self, index: u32) -> Self {
        Self {
            index: Some(index),
            ..self
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdMap {
    forward: BTreeMap<Uuid, u32>,
    reverse: BTreeMap<u32, Uuid>,
    #[serde(skip)]
    next_id: u32,
}

impl IdMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn find_next_id(&mut self) -> u32 {
        while self.reverse.contains_key(&self.next_id) {
            self.next_id += 1;
        }
        self.next_id
    }

    pub fn add(&mut self, uuid: Uuid) -> u32 {
        if let Some(id) = self.forward.get(&uuid).copied() {
            return id;
        }

        let id = self.find_next_id();

        self.forward.insert(uuid, id);
        self.reverse.insert(id, uuid);

        id
    }

    #[must_use]
    pub fn id(&self, uuid: &Uuid) -> Option<u32> {
        self.forward.get(uuid).copied()
    }

    #[must_use]
    pub fn uuid(&self, id: &u32) -> Option<Uuid> {
        self.reverse.get(id).copied()
    }

    pub fn remove(&mut self, uuid: &Uuid) {
        if let Some(id) = self.forward.remove(uuid) {
            self.reverse.remove(&id);
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub enum StateVersion {
    /// Version 0: (`res`, `aux`) tuple, no version field in state
    V0 = 0,

    #[default]
    /// Version 1: { `version`, `aux`, `id_v1`, `res` } map
    V1 = 1,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct State {
    version: StateVersion,
    aux: BTreeMap<Uuid, AuxData>,
    id_v1: IdMap,
    pub res: BTreeMap<Uuid, Resource>,
}

impl State {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn version(state: &Value) -> ApiResult<StateVersion> {
        if state.is_sequence() {
            return Ok(StateVersion::V0);
        }

        if let Some(version) = state.get("version") {
            return Ok(StateVersion::deserialize(version)?);
        }

        Err(ApiError::StateVersionNotFound)
    }

    pub fn patch_bridge_version(&mut self, version: &SwVersion) {
        let software_version = version.get_software_version();
        for (uuid, value) in &mut self.res {
            let Resource::Device(dev) = value else {
                continue;
            };

            let pd = &mut dev.product_data;

            if pd.model_id != hue::HUE_BRIDGE_V2_MODEL_ID {
                continue;
            }

            if pd.product_archetype != DeviceArchetype::BridgeV2 {
                continue;
            }

            if pd.software_version >= software_version {
                log::info!("Bridge device {uuid} already on newest firmware version");
            } else {
                log::info!(
                    "Bridge device {uuid} on older firmware {}. Updating to {}",
                    pd.software_version,
                    &software_version
                );
                pd.software_version.clone_from(&software_version);
            }
        }
    }

    pub fn from_v0(state: Value) -> ApiResult<Self> {
        let (v0res, v0aux): (serde_yml::Mapping, serde_yml::Mapping) =
            serde_yml::from_value(state)?;

        let mut aux = BTreeMap::new();
        let mut res = BTreeMap::new();

        log::debug!("Importing aux data from old v0 state..");
        for (key, value) in v0aux {
            log::debug!("  {key:?}: {value:?}");
            aux.insert(serde_yml::from_value(key)?, serde_yml::from_value(value)?);
        }

        log::debug!("Importing res data from old v0 state..");
        for (key, value) in v0res {
            log::debug!("  {key:?}: {value:?}");
            res.insert(serde_yml::from_value(key)?, serde_yml::from_value(value)?);
        }

        /* generate all missing id_v1 entries */
        log::debug!("Synthesizing id_v1 entries for all resources..");
        let mut id_v1 = IdMap::new();
        for key in res.keys() {
            id_v1.add(*key);
        }

        /* construct upgraded state */
        Ok(Self {
            version: StateVersion::V1,
            aux,
            id_v1,
            res,
        })
    }

    pub fn from_v1(state: Value) -> ApiResult<Self> {
        Ok(serde_yml::from_value(state)?)
    }

    pub fn from_reader(rdr: impl Read) -> ApiResult<Self> {
        let state = serde_yml::from_reader(rdr)?;
        match Self::version(&state)? {
            StateVersion::V0 => Self::from_v0(state),
            StateVersion::V1 => Self::from_v1(state),
        }
    }

    pub fn aux_get(&self, id: &Uuid) -> HueResult<&AuxData> {
        self.aux.get(id).ok_or(HueError::AuxNotFound(*id))
    }

    pub fn aux_set(&mut self, id: Uuid, aux: AuxData) {
        self.aux.insert(id, aux);
    }

    #[must_use]
    pub fn try_get(&self, id: &Uuid) -> Option<&Resource> {
        self.res.get(id)
    }

    pub fn get(&self, id: &Uuid) -> HueResult<&Resource> {
        self.try_get(id).ok_or(HueError::NotFound(*id))
    }

    pub fn get_mut(&mut self, id: &Uuid) -> HueResult<&mut Resource> {
        self.res.get_mut(id).ok_or(HueError::NotFound(*id))
    }

    pub fn insert(&mut self, key: Uuid, value: Resource) {
        self.res.insert(key, value);
        self.id_v1.add(key);
    }

    pub fn remove(&mut self, id: &Uuid) -> ApiResult<()> {
        self.aux.remove(id);
        self.id_v1.remove(id);
        self.res.remove(id).ok_or(HueError::NotFound(*id))?;
        Ok(())
    }

    #[must_use]
    pub fn id_v1(&self, uuid: &Uuid) -> Option<u32> {
        self.id_v1.id(uuid)
    }

    #[must_use]
    pub fn from_id_v1(&self, id: &u32) -> Option<Uuid> {
        self.id_v1.uuid(id)
    }
}
