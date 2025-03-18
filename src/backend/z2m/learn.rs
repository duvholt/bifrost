use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use hue::api::{
    ColorTemperatureUpdate, ColorUpdate, Light, RType, ResourceLink, Room, Scene, SceneAction,
    SceneActionElement,
};
use z2m::update::{DeviceColor, DeviceUpdate};

use crate::error::ApiResult;
use crate::resource::Resources;

pub struct SceneLearn {
    name: String,
    scenes: HashMap<Uuid, SceneInfo>,
}

#[derive(Debug)]
struct SceneInfo {
    pub expire: DateTime<Utc>,
    pub missing: HashSet<Uuid>,
    pub known: HashMap<Uuid, SceneAction>,
}

impl SceneLearn {
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            scenes: HashMap::new(),
        }
    }

    pub fn cleanup(&mut self) {
        let now = Utc::now();
        self.scenes.retain(|uuid, lscene| {
            let expired = lscene.expire < now;
            if expired {
                log::warn!(
                    "[{}] Failed to learn scene {uuid} before deadline",
                    self.name
                );
            }
            !expired
        });
    }

    pub fn learn_scene_recall(
        &mut self,
        lscene: &ResourceLink,
        lock: &mut Resources,
    ) -> ApiResult<()> {
        let scene: &Scene = lock.get(lscene)?;

        if !scene.actions.is_empty() {
            return Ok(());
        }

        let room: &Room = lock.get(&scene.group)?;

        let lights: Vec<Uuid> = room
            .children
            .iter()
            .filter_map(|rl| lock.get(rl).ok())
            .filter_map(hue::api::Device::light_service)
            .map(|rl| rl.rid)
            .collect();

        let learn = SceneInfo {
            expire: Utc::now() + Duration::seconds(5),
            missing: HashSet::from_iter(lights),
            known: HashMap::new(),
        };

        self.scenes.insert(lscene.rid, learn);

        Ok(())
    }

    pub fn learn(&mut self, uuid: &Uuid, res: &Resources, upd: &DeviceUpdate) -> ApiResult<()> {
        for learn in self.scenes.values_mut() {
            if !learn.missing.remove(uuid) {
                continue;
            }

            let rlink = RType::Light.link_to(*uuid);
            let light = res.get::<Light>(&rlink)?;
            let mut color_temperature = None;
            let mut color = None;
            if let Some(DeviceColor { xy: Some(xy), .. }) = upd.color {
                color = Some(ColorUpdate { xy });
            } else if let Some(mirek) = upd.color_temp {
                color_temperature = Some(ColorTemperatureUpdate { mirek });
            }

            learn.known.insert(
                *uuid,
                SceneAction {
                    color,
                    color_temperature,
                    dimming: light.as_dimming_opt(),
                    on: Some(light.on),
                    gradient: None,
                    effects: json!({}),
                },
            );

            log::info!("[{}] Learn: {learn:?}", self.name);
        }

        Ok(())
    }

    pub fn collect(&mut self, res: &mut Resources) -> ApiResult<()> {
        let keys: Vec<Uuid> = self.scenes.keys().copied().collect();
        for uuid in &keys {
            if self.scenes[uuid].missing.is_empty() {
                let lscene = self.scenes.remove(uuid).unwrap();
                log::info!("[{}] Learned all lights {uuid}", self.name);
                let actions: Vec<SceneActionElement> = lscene
                    .known
                    .into_iter()
                    .map(|(uuid, action)| SceneActionElement {
                        action,
                        target: RType::Light.link_to(uuid),
                    })
                    .collect();
                res.update::<Scene>(uuid, |scene| scene.actions = actions)?;
            }
        }

        Ok(())
    }
}
