use hue::api::{Device, Group, Light, RType, Resource, ResourceLink, configuration};

use crate::resource::Resources;

pub struct ResolvedLight {
    pub link: ResourceLink,
    pub state: Box<Light>,
}

pub struct ResolvedGroup {
    grouped_light: ResourceLink,
    lights: Vec<ResolvedLight>,
}

pub enum ResolvedResource {
    Light(ResolvedLight),
    Group(ResolvedGroup),
}

pub fn resolve_resources(
    res: &Resources,
    locations: &[configuration::Where],
) -> Vec<ResolvedResource> {
    let mut resolved_resources = Vec::new();

    for location in locations {
        #[allow(clippy::option_if_let_else)]
        let resource_links = match &location.items {
            Some(items) => items.as_slice(),
            None => std::slice::from_ref(&location.group),
        };

        for resource_link in resource_links {
            let resource = res.get_resource(resource_link);
            let resource = match resource {
                Ok(resource) => resource,
                Err(err) => {
                    log::warn!("Failed to get resource: {err}");
                    continue;
                }
            };
            match resource.obj {
                Resource::Room(room) => {
                    if let Some(group) = resolve_group(res, &room) {
                        resolved_resources.push(ResolvedResource::Group(group));
                    }
                }
                Resource::Zone(zone) => {
                    if let Some(group) = resolve_group(res, &zone) {
                        resolved_resources.push(ResolvedResource::Group(group));
                    }
                }
                Resource::Light(light) => {
                    resolved_resources.push(ResolvedResource::Light(ResolvedLight {
                        link: *resource_link,
                        state: light,
                    }));
                }
                Resource::BridgeHome(_bridge_home) => {
                    let all_rooms = res.get_resources_by_type(RType::Room);
                    resolved_resources.extend(all_rooms.into_iter().filter_map(|room_resource| {
                        match room_resource.obj {
                            Resource::Room(room) => {
                                resolve_group(res, &room).map(ResolvedResource::Group)
                            }
                            _ => None,
                        }
                    }));
                }
                _ => {}
            }
        }
    }

    resolved_resources
}

fn resolve_group(res: &Resources, group: &impl Group) -> Option<ResolvedGroup> {
    let grouped_light = *group.grouped_light_service()?;

    let lights: Vec<_> = group
        .children()
        .filter_map(|rl| match rl.rtype {
            RType::Device => res.get::<Device>(rl).ok().and_then(Device::light_service),
            RType::Light => Some(rl),
            _ => None,
        })
        .filter_map(|light_link| {
            res.get::<Light>(light_link).ok().map(|l| ResolvedLight {
                link: *light_link,
                state: Box::new(l.clone()),
            })
        })
        .collect();

    Some(ResolvedGroup {
        grouped_light,
        lights,
    })
}

pub enum TargetResource {
    Light(ResourceLink),
    GroupedLight(ResourceLink),
}

pub trait SelectionPolicy {
    type Effect: Clone + Eq;

    /// None means skip the light
    fn effect_for_light(&self, light: &ResolvedLight) -> Option<Self::Effect>;
    /// If all lights in a group share the same effect we can issue a grouped light request instead of indivial light requests
    fn supports_grouped_light(&self, effect: &Self::Effect) -> bool;
}

pub struct ExecutionTarget<E> {
    pub resource: TargetResource,
    pub effect: E,
}

pub fn select_targets<P: SelectionPolicy>(
    resources: &[ResolvedResource],
    policy: &P,
) -> Vec<ExecutionTarget<P::Effect>> {
    let mut targets = Vec::new();
    for resource in resources {
        match resource {
            ResolvedResource::Light(light) => {
                if let Some(effect) = policy.effect_for_light(light) {
                    targets.push(ExecutionTarget {
                        resource: TargetResource::Light(light.link),
                        effect,
                    });
                }
            }
            ResolvedResource::Group(group) => {
                targets.extend(select_group_targets(group, policy));
            }
        }
    }
    targets
}

fn select_group_targets<P: SelectionPolicy>(
    group: &ResolvedGroup,
    policy: &P,
) -> Vec<ExecutionTarget<P::Effect>> {
    let light_effects: Vec<_> = group
        .lights
        .iter()
        .map(|light| policy.effect_for_light(light))
        .collect();
    if let Some(effect) = common_effect(&light_effects)
        && policy.supports_grouped_light(effect)
    {
        return vec![ExecutionTarget {
            resource: TargetResource::GroupedLight(group.grouped_light),
            effect: effect.clone(),
        }];
    }
    light_effects
        .into_iter()
        .zip(&group.lights)
        .filter_map(|(effect, light)| {
            effect.map(|effect| ExecutionTarget {
                resource: TargetResource::Light(light.link),
                effect,
            })
        })
        .collect()
}

fn common_effect<E: Eq>(effects: &[Option<E>]) -> Option<&E> {
    let first = effects.first()?.as_ref()?;
    effects
        .iter()
        .all(|e| e.as_ref() == Some(first))
        .then_some(first)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use hue::api::{
        BridgeHome, DeviceArchetype, DeviceProductData, Dimming, GroupArchetype, GroupMetadata,
        GroupedLight, LightMetadata, Metadata, On, Room, Zone,
    };
    use hue::version::SwVersion;
    use uuid::Uuid;

    use crate::model::state::State;

    use super::*;

    struct Fixture {
        resources: Resources,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                resources: Resources::new(SwVersion::default(), State::new()),
            }
        }

        fn add(&mut self, resource: Resource) -> ResourceLink {
            let link = ResourceLink::new(Uuid::new_v4(), resource.rtype());
            self.resources.add(&link, resource).unwrap();
            link
        }

        fn light(&mut self, on: bool, brightness: f64) -> (ResourceLink, ResourceLink) {
            let device_link = self.add(Resource::Device(Device {
                product_data: DeviceProductData {
                    model_id: "test-light".into(),
                    manufacturer_name: "Test".into(),
                    product_name: "Test light".into(),
                    product_archetype: DeviceArchetype::UnknownArchetype,
                    certified: false,
                    software_version: "1".into(),
                    hardware_platform_type: None,
                },
                metadata: Metadata {
                    name: "Test light".into(),
                    archetype: DeviceArchetype::UnknownArchetype,
                },
                services: BTreeSet::new(),
                usertest: None,
                identify: None,
            }));
            let mut light = Light::new(
                device_link,
                LightMetadata::new(DeviceArchetype::UnknownArchetype, "Test light"),
            );
            light.on = On::new(on);
            light.dimming = Some(Dimming {
                brightness,
                min_dim_level: None,
            });
            let light = self.add(Resource::Light(Box::new(light)));
            self.resources
                .update(&device_link.rid, |device: &mut Device| {
                    device.services.insert(light);
                })
                .unwrap();
            (device_link, light)
        }

        fn group(
            &mut self,
            rtype: RType,
            children: &[ResourceLink],
        ) -> (ResourceLink, ResourceLink) {
            let group_link = ResourceLink::new(Uuid::new_v4(), rtype);
            let grouped_light = self.add(Resource::GroupedLight(GroupedLight::new(group_link)));
            let children = children.iter().copied().collect();
            let metadata = GroupMetadata::new(GroupArchetype::Other, "Test group");
            let services = BTreeSet::from([grouped_light]);
            let resource = match rtype {
                RType::Room => Resource::Room(Room {
                    children,
                    metadata,
                    services,
                }),
                RType::Zone => Resource::Zone(Zone {
                    children,
                    metadata,
                    services,
                }),
                _ => panic!("Expected room or zone"),
            };
            self.resources.add(&group_link, resource).unwrap();
            (group_link, grouped_light)
        }
    }

    fn only_group(resources: &[ResolvedResource]) -> &ResolvedGroup {
        assert_eq!(resources.len(), 1);
        let ResolvedResource::Group(group) = &resources[0] else {
            panic!("Expected a group resource");
        };
        group
    }

    fn assert_light(fixture: &Fixture, resolved_light: &ResolvedLight, expected: ResourceLink) {
        assert_eq!(resolved_light.link, expected);
        assert_eq!(
            &*resolved_light.state,
            fixture.resources.get::<Light>(&expected).unwrap()
        );
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum TestEffect {
        Basic,
        Sunrise,
    }

    struct TestPolicy {
        effects: BTreeMap<ResourceLink, TestEffect>,
        allow_group: bool,
    }

    impl SelectionPolicy for TestPolicy {
        type Effect = TestEffect;

        fn effect_for_light(&self, light: &ResolvedLight) -> Option<Self::Effect> {
            self.effects.get(&light.link).cloned()
        }

        fn supports_grouped_light(&self, _effect: &Self::Effect) -> bool {
            self.allow_group
        }
    }

    fn selection_group(
        effects: &[Option<TestEffect>],
    ) -> (
        Vec<ResolvedResource>,
        ResourceLink,
        Vec<ResourceLink>,
        TestPolicy,
    ) {
        let mut fixture = Fixture::new();
        let mut lights = Vec::new();
        let mut policy = TestPolicy {
            effects: BTreeMap::new(),
            allow_group: true,
        };
        for effect in effects {
            let (_, light) = fixture.light(false, 20.0);
            lights.push(light);
            if let Some(effect) = effect {
                policy.effects.insert(light, effect.clone());
            }
        }
        let (zone, grouped_light_link) = fixture.group(RType::Zone, &lights);
        let resources = resolve_resources(
            &fixture.resources,
            &[configuration::Where {
                group: zone,
                items: None,
            }],
        );
        (resources, grouped_light_link, lights, policy)
    }

    fn assert_selected_lights(
        selected: &[ExecutionTarget<TestEffect>],
        expected: &[(ResourceLink, TestEffect)],
    ) {
        let actual: BTreeMap<_, _> = selected
            .iter()
            .map(|target| {
                let TargetResource::Light(link) = target.resource else {
                    panic!("Expected an individual light resource");
                };
                (link, target.effect.clone())
            })
            .collect();
        assert_eq!(selected.len(), expected.len());
        assert_eq!(actual, expected.iter().cloned().collect());
    }

    #[test]
    fn selects_group_when_all_members_have_same_groupable_effect() {
        let (resources, grouped_light_link, _, policy) =
            selection_group(&[Some(TestEffect::Basic), Some(TestEffect::Basic)]);

        let selected = select_targets(&resources, &policy);

        assert_eq!(selected.len(), 1);
        let TargetResource::GroupedLight(link) = selected[0].resource else {
            panic!("Expected a group resource");
        };
        assert_eq!(link, grouped_light_link);
        assert_eq!(selected[0].effect, TestEffect::Basic);
    }

    #[test]
    fn selects_individual_lights_when_policy_forbids_grouping() {
        let (resources, _, lights, mut policy) =
            selection_group(&[Some(TestEffect::Sunrise), Some(TestEffect::Sunrise)]);
        policy.allow_group = false;

        let selected = select_targets(&resources, &policy);

        assert_selected_lights(
            &selected,
            &[
                (lights[0], TestEffect::Sunrise),
                (lights[1], TestEffect::Sunrise),
            ],
        );
    }

    #[test]
    fn selects_individual_lights_when_effects_differ() {
        let (resources, _, lights, policy) =
            selection_group(&[Some(TestEffect::Basic), Some(TestEffect::Sunrise)]);

        let selected = select_targets(&resources, &policy);

        assert_selected_lights(
            &selected,
            &[
                (lights[0], TestEffect::Basic),
                (lights[1], TestEffect::Sunrise),
            ],
        );
    }

    #[test]
    fn skipped_member_prevents_group_selection() {
        let (resources, _, lights, policy) = selection_group(&[Some(TestEffect::Basic), None]);

        let selected = select_targets(&resources, &policy);

        assert_selected_lights(&selected, &[(lights[0], TestEffect::Basic)]);
    }

    #[test]
    fn selects_only_eligible_individual_resources() {
        let (resources, _, lights, policy) = selection_group(&[Some(TestEffect::Sunrise), None]);
        let ResolvedResource::Group(group) = resources.into_iter().next().unwrap() else {
            panic!("Expected a group resource");
        };
        let individual_resources: Vec<_> = group
            .lights
            .into_iter()
            .map(ResolvedResource::Light)
            .collect();

        let selected = select_targets(&individual_resources, &policy);

        assert_selected_lights(&selected, &[(lights[0], TestEffect::Sunrise)]);
    }

    #[test]
    fn selects_nothing_for_empty_or_entirely_skipped_targets() {
        for effects in [vec![], vec![None, None]] {
            let (resources, _, _, policy) = selection_group(&effects);
            assert!(select_targets(&resources, &policy).is_empty());
            assert!(select_targets(&[], &policy).is_empty());
        }
    }

    #[test]
    fn resolves_room_with_member_states() {
        let mut fixture = Fixture::new();
        let (off_device, off_light) = fixture.light(false, 20.0);
        let (on_device, on_light) = fixture.light(true, 80.0);
        let (room, grouped_light_link) = fixture.group(RType::Room, &[off_device, on_device]);

        let resources = resolve_resources(
            &fixture.resources,
            &[configuration::Where {
                group: room,
                items: None,
            }],
        );

        let group = only_group(&resources);
        assert_eq!(group.grouped_light, grouped_light_link);
        assert_eq!(group.lights.len(), 2);
        for link in [off_light, on_light] {
            let member = group
                .lights
                .iter()
                .find(|light| light.link == link)
                .unwrap();
            assert_light(&fixture, member, link);
        }
    }

    #[test]
    fn resolves_zone_with_direct_light_children() {
        let mut fixture = Fixture::new();
        let (_, light) = fixture.light(false, 30.0);
        let (zone, grouped_light_link) = fixture.group(RType::Zone, &[light]);

        let resources = resolve_resources(
            &fixture.resources,
            &[configuration::Where {
                group: zone,
                items: None,
            }],
        );

        let group = only_group(&resources);
        assert_eq!(group.grouped_light, grouped_light_link);
        assert_eq!(group.lights.len(), 1);
        assert_light(&fixture, &group.lights[0], light);
    }

    #[test]
    fn resolves_items_independently_of_group() {
        let mut fixture = Fixture::new();
        let (_, light) = fixture.light(true, 60.0);
        let missing_room = ResourceLink::new(Uuid::new_v4(), RType::Room);

        let resources = resolve_resources(
            &fixture.resources,
            &[configuration::Where {
                group: missing_room,
                items: Some(vec![light]),
            }],
        );

        assert_eq!(resources.len(), 1);
        let ResolvedResource::Light(resolved_light) = &resources[0] else {
            panic!("Expected an individual light resource");
        };
        assert_light(&fixture, resolved_light, light);
    }

    #[test]
    fn bridge_home_resolves_all_rooms() {
        let mut fixture = Fixture::new();
        let (device_a, _) = fixture.light(false, 10.0);
        let (device_b, _) = fixture.light(true, 90.0);
        let (room_a, grouped_light_link_a) = fixture.group(RType::Room, &[device_a]);
        let (room_b, grouped_light_link_b) = fixture.group(RType::Room, &[device_b]);
        let home = fixture.add(Resource::BridgeHome(BridgeHome {
            children: BTreeSet::from([room_a, room_b]),
            services: BTreeSet::new(),
        }));
        fixture.group(RType::Zone, &[device_a, device_b]);

        let resources = resolve_resources(
            &fixture.resources,
            &[configuration::Where {
                group: home,
                items: None,
            }],
        );

        assert_eq!(resources.len(), 2);
        let grouped_light_links: BTreeSet<_> = resources
            .iter()
            .map(|resource| {
                let ResolvedResource::Group(group) = resource else {
                    panic!("Expected a group resource");
                };
                assert_eq!(group.lights.len(), 1);
                group.grouped_light
            })
            .collect();
        assert_eq!(
            grouped_light_links,
            BTreeSet::from([grouped_light_link_a, grouped_light_link_b])
        );
    }
}
