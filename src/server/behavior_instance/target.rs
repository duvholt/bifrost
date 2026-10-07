use hue::api::{Device, Group, Light, RType, Resource, ResourceLink, configuration};
use itertools::Itertools;

use crate::resource::Resources;

struct TargetLight {
    link: ResourceLink,
    state: Light,
}

struct TargetGroup {
    grouped_light: ResourceLink,
    lights: Vec<TargetLight>,
}

enum Target {
    Light(TargetLight),
    Group(TargetGroup),
}

fn group_target(res: &Resources, group: &impl Group) -> Option<Vec<Target>> {
    let lights_in_room: Vec<_> = group
        .children()
        .filter_map(|rl| match rl.rtype {
            RType::Device => res.get::<Device>(rl).ok().and_then(Device::light_service),
            RType::Light => Some(rl),
            _ => None,
        })
        .filter_map(|light_link| {
            res.get::<Light>(light_link).ok().map(|l| TargetLight {
                link: *light_link,
                state: l.clone(),
            })
        })
        .collect();

    group.grouped_light_service().map(|grouped_light| {
        vec![Target::Group(TargetGroup {
            grouped_light: *grouped_light,
            lights: lights_in_room,
        })]
    })
}

fn resolve_targets(res: &Resources, locations: &[configuration::Where]) -> Vec<Target> {
    let resource_links = locations.iter().flat_map(|room| {
        room.items
            .as_ref()
            .map_or_else(|| vec![room.group], Clone::clone)
    });

    resource_links
        .into_iter()
        .filter_map(|resource_link| {
            let resource = res.get_resource(&resource_link);
            match resource {
                Ok(resource) => Some((resource_link, resource)),
                Err(err) => {
                    log::warn!("Failed to get resource: {err}");
                    None
                }
            }
        })
        .filter_map(|(resource_link, resource)| match resource.obj {
            Resource::Room(room) => group_target(res, &room),
            Resource::Zone(zone) => group_target(res, &zone),
            Resource::Light(light) => Some(vec![Target::Light(TargetLight {
                link: resource_link,
                state: *light,
            })]),
            Resource::BridgeHome(_bridge_home) => {
                let all_rooms = res.get_resources_by_type(RType::Room);
                Some(
                    all_rooms
                        .into_iter()
                        .filter_map(|room_resource| match room_resource.obj {
                            Resource::Room(room) => group_target(res, &room),
                            _ => None,
                        })
                        .concat(),
                )
            }
            _ => Some(Vec::new()),
        })
        .concat()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

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

    fn only_group(targets: &[Target]) -> &TargetGroup {
        assert_eq!(targets.len(), 1);
        let Target::Group(group) = &targets[0] else {
            panic!("Expected a group target");
        };
        group
    }

    fn assert_light(fixture: &Fixture, target: &TargetLight, expected: ResourceLink) {
        assert_eq!(target.link, expected);
        assert_eq!(
            &target.state,
            fixture.resources.get::<Light>(&expected).unwrap()
        );
    }

    #[test]
    fn resolves_room_with_member_states() {
        let mut fixture = Fixture::new();
        let (off_device, off_light) = fixture.light(false, 20.0);
        let (on_device, on_light) = fixture.light(true, 80.0);
        let (room, service) = fixture.group(RType::Room, &[off_device, on_device]);

        let targets = resolve_targets(
            &fixture.resources,
            &[configuration::Where {
                group: room,
                items: None,
            }],
        );

        let group = only_group(&targets);
        assert_eq!(group.grouped_light, service);
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
        let (zone, service) = fixture.group(RType::Zone, &[light]);

        let targets = resolve_targets(
            &fixture.resources,
            &[configuration::Where {
                group: zone,
                items: None,
            }],
        );

        let group = only_group(&targets);
        assert_eq!(group.grouped_light, service);
        assert_eq!(group.lights.len(), 1);
        assert_light(&fixture, &group.lights[0], light);
    }

    #[test]
    fn resolves_items_independently_of_group() {
        let mut fixture = Fixture::new();
        let (_, light) = fixture.light(true, 60.0);
        let missing_room = ResourceLink::new(Uuid::new_v4(), RType::Room);

        let targets = resolve_targets(
            &fixture.resources,
            &[configuration::Where {
                group: missing_room,
                items: Some(vec![light]),
            }],
        );

        assert_eq!(targets.len(), 1);
        let Target::Light(target) = &targets[0] else {
            panic!("Expected an individual light target");
        };
        assert_light(&fixture, target, light);
    }

    #[test]
    fn bridge_home_resolves_all_rooms() {
        let mut fixture = Fixture::new();
        let (device_a, _) = fixture.light(false, 10.0);
        let (device_b, _) = fixture.light(true, 90.0);
        let (room_a, service_a) = fixture.group(RType::Room, &[device_a]);
        let (room_b, service_b) = fixture.group(RType::Room, &[device_b]);
        let home = fixture.add(Resource::BridgeHome(BridgeHome {
            children: BTreeSet::from([room_a, room_b]),
            services: BTreeSet::new(),
        }));
        fixture.group(RType::Zone, &[device_a, device_b]);

        let targets = resolve_targets(
            &fixture.resources,
            &[configuration::Where {
                group: home,
                items: None,
            }],
        );

        assert_eq!(targets.len(), 2);
        let services: BTreeSet<_> = targets
            .iter()
            .map(|target| {
                let Target::Group(group) = target else {
                    panic!("Expected a group target");
                };
                assert_eq!(group.lights.len(), 1);
                group.grouped_light
            })
            .collect();
        assert_eq!(services, BTreeSet::from([service_a, service_b]));
    }
}
