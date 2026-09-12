use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use hue::api::{
    ColorTemperatureUpdate, ColorUpdate, Light, LightGradientPoint, LightGradientUpdate, RType,
    Resource, ResourceLink, Scene, SceneAction, SceneActionElement,
};
use z2m::hexcolor::HexColor;
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

        let lights: Vec<Uuid> = match lock.get_resource(&scene.group)?.obj {
            Resource::Room(room) => room
                .children
                .iter()
                .filter_map(|rl| lock.get(rl).ok())
                .filter_map(hue::api::Device::light_service)
                .map(|rl| rl.rid)
                .collect(),
            Resource::Zone(zone) => zone
                .children
                .iter()
                .filter_map(|rl| lock.get::<Light>(rl).ok().map(|_| rl.rid))
                .collect(),
            _ => {
                log::warn!(
                    "Tried to learn scene for an invalid group type {:?}",
                    scene.group
                );
                return Ok(());
            }
        };

        let learn = SceneInfo {
            expire: Utc::now() + Duration::seconds(5),
            missing: HashSet::from_iter(lights),
            known: HashMap::new(),
        };

        self.scenes.insert(lscene.rid, learn);

        Ok(())
    }

    #[allow(clippy::option_if_let_else, clippy::manual_map)]
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
                color_temperature = Some(ColorTemperatureUpdate::new(mirek));
            }

            let gradient = if let Some(grad) = &upd.gradient {
                Some(LightGradientUpdate {
                    mode: None,
                    points: grad
                        .iter()
                        .map(|p| LightGradientPoint {
                            color: ColorUpdate {
                                xy: HexColor::to_xy_color(p),
                            },
                        })
                        .collect(),
                })
            } else {
                None
            };

            learn.known.insert(
                *uuid,
                SceneAction {
                    color,
                    color_temperature,
                    dimming: light.as_dimming_opt(),
                    on: Some(light.on),
                    gradient,
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use hue::api::{
        Device, DeviceArchetype, DeviceProductData, GroupArchetype, GroupMetadata, LightMetadata,
        Metadata, Resource, Room, SceneMetadata, SceneRecall, Zone,
    };
    use hue::version::SwVersion;

    use super::*;
    use crate::model::state::State;

    #[test]
    fn learns_lights_from_zone_children() -> ApiResult<()> {
        let mut resources = Resources::new(SwVersion::default(), State::new());
        let device_link = RType::Device.random();
        let light_link = RType::Light.random();
        let zone_link = RType::Zone.random();
        let scene_link = RType::Scene.random();

        resources.add(
            &light_link,
            Resource::Light(Box::new(Light::new(
                device_link,
                LightMetadata::new(DeviceArchetype::ClassicBulb, "light"),
            ))),
        )?;
        resources.add(
            &zone_link,
            Resource::Zone(Zone {
                children: BTreeSet::from([light_link]),
                metadata: GroupMetadata::new(GroupArchetype::Home, "zone"),
                services: BTreeSet::new(),
            }),
        )?;
        resources.add(
            &scene_link,
            Resource::Scene(Scene {
                actions: Vec::new(),
                auto_dynamic: false,
                group: zone_link,
                metadata: SceneMetadata {
                    appdata: None,
                    image: None,
                    name: "scene".to_string(),
                },
                palette: serde_json::Value::Null,
                speed: 0.5,
                status: None,
                recall: SceneRecall::default(),
            }),
        )?;

        let mut learner = SceneLearn::new("test".to_string());
        learner.learn_scene_recall(&scene_link, &mut resources)?;

        assert_eq!(
            learner.scenes[&scene_link.rid].missing,
            HashSet::from([light_link.rid])
        );
        Ok(())
    }

    #[test]
    fn learns_lights_from_room_children() -> ApiResult<()> {
        let mut resources = Resources::new(SwVersion::default(), State::new());
        let device_link = RType::Device.random();
        let light_link = RType::Light.random();
        let room_link = RType::Room.random();
        let scene_link = RType::Scene.random();

        resources.add(
            &device_link,
            Resource::Device(Device {
                product_data: DeviceProductData::hue_bridge_v2(&SwVersion::default()),
                metadata: Metadata::new(DeviceArchetype::ClassicBulb, "light"),
                services: BTreeSet::from([light_link]),
                usertest: None,
                identify: None,
            }),
        )?;
        resources.add(
            &room_link,
            Resource::Room(Room {
                children: BTreeSet::from([device_link]),
                metadata: GroupMetadata::new(GroupArchetype::Home, "room"),
                services: BTreeSet::new(),
            }),
        )?;
        resources.add(
            &scene_link,
            Resource::Scene(Scene {
                actions: Vec::new(),
                auto_dynamic: false,
                group: room_link,
                metadata: SceneMetadata {
                    appdata: None,
                    image: None,
                    name: "scene".to_string(),
                },
                palette: serde_json::Value::Null,
                speed: 0.5,
                status: None,
                recall: SceneRecall::default(),
            }),
        )?;

        let mut learner = SceneLearn::new("test".to_string());
        learner.learn_scene_recall(&scene_link, &mut resources)?;

        assert_eq!(
            learner.scenes[&scene_link.rid].missing,
            HashSet::from([light_link.rid])
        );
        Ok(())
    }
}
