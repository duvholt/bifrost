use hue::api::{
    DimmingUpdate, Group, GroupArchetype, GroupMetadata, GroupedLight, LightAlert, LightSignal,
    LightSignaling, On, RType, Resource, ResourceLink, Stub, Zone, ZoneNew,
};
#[cfg(test)]
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::common::fixture::{self, FixtureDevice, FixtureGroup, Z2mFixture};
use crate::common::{
    TestBridge, TestLight, TestResult, TestZ2mBackend, TestZ2mDeviceOrGroup, TestZone, init,
};

pub mod common;

// Lights
const IKEA_WARM_WHITE: FixtureDevice = FixtureDevice {
    id: 1,
    friendly_name: "ikea_warm_white",
};

const IKEA_COLOR: FixtureDevice = FixtureDevice {
    id: 2,
    friendly_name: "ikea_color",
};

const HUE_WHITE_AMBIANCE: FixtureDevice = FixtureDevice {
    id: 3,
    friendly_name: "hue_white_ambiance",
};

const HUE_FLUX_LIGHTSTRIP: FixtureDevice = FixtureDevice {
    id: 4,
    friendly_name: "hue_flux_lightstrip",
};

const HUE_PLAY_GRADIENT_LIGHTSTRIP: FixtureDevice = FixtureDevice {
    id: 5,
    friendly_name: "hue_play_gradient_lightstrip",
};

const MISC_DIMMER_LIGHT: FixtureDevice = FixtureDevice {
    id: 6,
    friendly_name: "misc_dimmer_light",
};
const IKEA_COLOR_WITHOUT_ZONE: FixtureDevice = FixtureDevice {
    id: 7,
    friendly_name: "ikea_color_without_room",
};

fn z2m_state() -> BTreeMap<String, Value> {
    let ikea_warm_white = fixture::ikea::tradfri_warm_white(&IKEA_WARM_WHITE).with_state(json!({
        "state": "OFF",
        "brightness": 127
    }));

    let ikea_color = fixture::ikea::tradfri_color(&IKEA_COLOR).with_state(json!({
        "state": "OFF",
        "brightness": 127
    }));

    let hue_white_ambiance = fixture::hue::white_ambience(&HUE_WHITE_AMBIANCE).with_state(json!({
        "state": "OFF",
        "brightness": 127
    }));

    let hue_flux_lightstrip =
        fixture::hue::flux_lightstrip(&HUE_FLUX_LIGHTSTRIP).with_state(json!({
            "state": "OFF",
            "brightness": 127
        }));

    let hue_play_gradient_lightstrip =
        fixture::hue::play_gradient_lightstrip(&HUE_PLAY_GRADIENT_LIGHTSTRIP).with_state(json!({
            "state": "OFF",
            "brightness": 127
        }));

    let misc_dimmer_light = fixture::misc::dimmer_light(&MISC_DIMMER_LIGHT).with_state(json!({
        "state": "OFF",
        "brightness": 127
    }));

    let ikea_color_without_zone = fixture::ikea::tradfri_color(&IKEA_COLOR_WITHOUT_ZONE)
        .with_state(json!({
            "state": "OFF",
            "brightness": 127
        }));

    let fixture = Z2mFixture::new()
        .with_device(ikea_warm_white)
        .with_device(ikea_color)
        .with_device(hue_white_ambiance)
        .with_device(hue_flux_lightstrip)
        .with_device(hue_play_gradient_lightstrip)
        .with_device(misc_dimmer_light)
        .with_device(ikea_color_without_zone);

    fixture.into_state()
}

#[allow(clippy::too_many_lines)]
async fn create_zone_and_assert<'a>(
    test: &TestBridge,
    z2m: &TestZ2mBackend,
    metadata: &'a GroupMetadata,
    lights: &[&TestLight],
) -> TestResult<TestZone<'a>> {
    let mut z2m_requests = z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    let children: BTreeSet<_> = lights.iter().map(|light| light.link).collect();
    let new_zone_link = test
        .hue_client
        .post_zone(ZoneNew {
            children: children.clone(),
            metadata: metadata.clone(),
        })
        .await?;

    let test_zone = TestZone {
        link: new_zone_link,
        fixture_group: FixtureGroup {
            id: 1,
            friendly_name: &metadata.name,
        },
    };

    // Initial created zone
    z2m_requests
        .expect_request(
            "bridge/request/group/add",
            json!({"id":test_zone.fixture_group.id, "friendly_name":test_zone.fixture_group.friendly_name}),
        )
        .await?;
    let resource_record = hue_events.expect_add(RType::Zone).await?;
    let Resource::Zone(zone_resource) = resource_record.obj else {
        panic!("unexpected resource type");
    };
    assert_eq!(resource_record.id, new_zone_link.rid);
    assert_eq!(
        zone_resource,
        Zone {
            children: children.clone(),
            metadata: metadata.clone(),
            services: BTreeSet::new()
        }
    );
    // Group add response

    // todo: we need to append the group instead of overwriting all groups with just the new one
    // this should likely be handled in the z2m emulator
    z2m.publish_topic("bridge/groups".to_string(), json!([
        {"friendly_name": &test_zone.fixture_group.friendly_name, "id":&test_zone.fixture_group.id, "members":[], "scenes":[]}
    ]))?;
    z2m.publish_topic("bridge/response/group/add".to_string(), json!(
        {"data":{"friendly_name": &test_zone.fixture_group.friendly_name,"id": &test_zone.fixture_group.id},"status":"ok"}
    ))?;

    let grouped_light_record = hue_events.expect_add(RType::GroupedLight).await?;
    let Resource::GroupedLight(grouped_light) = grouped_light_record.obj else {
        panic!("unexpected resource type");
    };
    assert_eq!(
        grouped_light,
        GroupedLight {
            alert: Some(LightAlert {
                action_values: BTreeSet::from(["breathe".to_string()]),
            },),
            dimming: Some(DimmingUpdate { brightness: 0.0 },),
            color: Some(Stub,),
            color_temperature: Some(Stub,),
            color_temperature_delta: Some(Stub,),
            dimming_delta: Some(Stub,),
            dynamics: Some(Stub,),
            on: Some(On { on: true },),
            owner: new_zone_link,
            signaling: Some(LightSignaling {
                signal_values: vec![
                    LightSignal::Alternating,
                    LightSignal::NoSignal,
                    LightSignal::OnOff,
                    LightSignal::OnOffColor,
                ],
                status: Value::Null,
            },),
        }
    );
    let zone_update = hue_events.expect_update_resource(new_zone_link).await?;
    assert_eq!(
        zone_update.data,
        json!({
            "children": [],
            "services": [
                {
                    "rid": grouped_light_record.id,
                    "rtype": "grouped_light",
                },
            ],
        })
    );

    // Member add response
    for i in 0..lights.len() {
        let light = &lights[i];
        let iter_lights = lights.iter().take(i + 1).collect::<Vec<_>>();
        z2m.publish_topic("bridge/response/group/members/add".to_string(), json!(
            {"data":{"device": light.fixture_id.friendly_name,"endpoint":"default","group": &test_zone.fixture_group.friendly_name},"status":"ok"}
        ))?;
        z2m.publish_topic(
            "bridge/groups".to_string(),
            json!([
                {
                    "friendly_name": &test_zone.fixture_group.friendly_name,
                    "id":&test_zone.fixture_group.id,
                    "members": iter_lights.iter().map(|light| json!({"endpoint":11,"ieee_address": light.fixture_id.ieee_address()})).collect::<Vec<_>>(),
                    "scenes":[]
                }
            ]),
        )?;
        let zone_update = hue_events.expect_update_resource(new_zone_link).await?;
        let actual_children =
            serde_json::from_value::<BTreeSet<ResourceLink>>(zone_update.data["children"].clone())?;
        assert_eq!(
            actual_children,
            iter_lights
                .iter()
                .map(|light| light.link)
                .collect::<BTreeSet<_>>()
        );
    }

    let light_joins: Vec<_> = lights
        .iter()
        .map(|light| {
            (
                "bridge/request/group/members/add".to_string(),
                json!({"device": light.fixture_id.friendly_name, "group":test_zone.fixture_group.friendly_name}),
            )
        })
        .collect();

    z2m_requests.expect_requests_unordered(light_joins).await?;

    Ok(test_zone)
}

#[tokio::test]
async fn get_zones() -> TestResult<()> {
    init();
    let test = TestBridge::start(z2m_state()).await?;

    let zones = test.hue_client.get_zones().await?;

    let expected_zones: Vec<Zone> = Vec::new();
    assert_eq!(expected_zones, zones);
    Ok(())
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn create_zone() -> TestResult<()> {
    init();
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(IKEA_COLOR_WITHOUT_ZONE).await?;

    create_zone_and_assert(
        &test,
        test.z2m.only_backend(),
        &GroupMetadata {
            name: "new1".to_string(),
            archetype: GroupArchetype::Barbecue,
        },
        &[&light],
    )
    .await?;

    Ok(())
}

#[tokio::test]
async fn zone_rejects_lights_from_different_backends() -> TestResult<()> {
    let backend_states = {
        let first = fixture::ikea::tradfri_warm_white(&IKEA_WARM_WHITE).with_state(json!({
            "state": "OFF",
            "brightness": 127
        }));
        let second = fixture::ikea::tradfri_color(&IKEA_COLOR_WITHOUT_ZONE).with_state(json!({
            "state": "OFF",
            "brightness": 127
        }));

        BTreeMap::from([
            (
                "first".to_string(),
                Z2mFixture::new().with_device(first).into_state(),
            ),
            (
                "second".to_string(),
                Z2mFixture::new().with_device(second).into_state(),
            ),
        ])
    };
    init();
    let mut test = TestBridge::start_backends(backend_states).await?;
    let first = test.wait_for_light(IKEA_WARM_WHITE).await?;
    let second = test.wait_for_light(IKEA_COLOR_WITHOUT_ZONE).await?;
    let group_metadata = GroupMetadata {
        name: "first-zone".to_string(),
        archetype: GroupArchetype::Home,
    };
    let zone = create_zone_and_assert(
        &test,
        test.z2m.backend("first")?,
        &group_metadata,
        &[&first],
    )
    .await?;
    let zones_before = test.hue_client.get_zones().await?;
    let mut first_requests = test.z2m.backend("first")?.subscribe_requests();
    let mut second_requests = test.z2m.backend("second")?.subscribe_requests();

    assert!(
        test.hue_client
            .post_zone(ZoneNew {
                children: BTreeSet::from([first.link, second.link]),
                metadata: GroupMetadata {
                    name: "mixed".to_string(),
                    archetype: GroupArchetype::Home,
                },
            })
            .await
            .is_err()
    );
    assert!(
        test.hue_client
            .put_zone(&zone, &json!({"children": [second.link]}))
            .await
            .is_err()
    );

    assert_eq!(test.hue_client.get_zones().await?, zones_before);
    first_requests.expect_quiet().await?;
    second_requests.expect_quiet().await?;
    Ok(())
}

#[tokio::test]
async fn empty_zone_can_acquire_first_child() -> TestResult<()> {
    init();
    let mut test = TestBridge::start(z2m_state()).await?;
    let child = test.wait_for_light(IKEA_COLOR_WITHOUT_ZONE).await?;
    let zone = TestZone {
        link: test
            .hue_client
            .post_zone(ZoneNew {
                children: BTreeSet::new(),
                metadata: GroupMetadata {
                    name: "empty".to_string(),
                    archetype: GroupArchetype::Home,
                },
            })
            .await?,
        fixture_group: FixtureGroup {
            id: 1,
            friendly_name: "empty",
        },
    };
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_zone(&zone, &json!({ "children": [child.link] }))
        .await?;

    z2m_requests
        .expect_requests_unordered([
            (
                "bridge/request/group/add",
                json!({"id":zone.fixture_group.id,"friendly_name":zone.fixture_group.friendly_name}),
            ),
            (
                "bridge/request/group/members/add",
                json!({"device":IKEA_COLOR_WITHOUT_ZONE.topic(),"group":zone.topic()}),
            ),
        ])
        .await?;

    test.z2m.publish_topic("bridge/response/group/members/add".to_string(), json!({"data":{"device":IKEA_COLOR_WITHOUT_ZONE.topic(),"endpoint":"default","group":zone.topic()},"status":"ok"}))?;
    test.z2m.publish_topic(
        "bridge/groups".to_string(),
        json!([
            {"friendly_name": &zone.fixture_group.friendly_name, "id":&zone.fixture_group.id, "members":[
                {"endpoint":11,"ieee_address": IKEA_COLOR_WITHOUT_ZONE.ieee_address()}
                ],
             "scenes":[]}
        ]),
    )?;

    let grouped_light = hue_events.expect_add(RType::GroupedLight).await?;
    assert_eq!(
        hue_events.expect_update_resource(zone.link).await?.data,
        json!({
           "children": [child.link],
           "services": [grouped_light.link()],
        })
    );

    hue_events.expect_quiet().await?;

    Ok(())
}

#[tokio::test]
async fn delete_zone() -> TestResult<()> {
    init();
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(IKEA_COLOR_WITHOUT_ZONE).await?;
    let group_metadata = GroupMetadata {
        name: "new1".to_string(),
        archetype: GroupArchetype::Barbecue,
    };
    let zone =
        create_zone_and_assert(&test, test.z2m.only_backend(), &group_metadata, &[&light]).await?;
    let zone_resource = test.hue_client.get_zone(&zone).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    let link = test.hue_client.delete_zone(&zone).await?;
    assert_eq!(link, zone.link);

    z2m_requests
        .expect_request(
            "bridge/request/group/remove",
            json!({"force":false,"id": zone.fixture_group.friendly_name}),
        )
        .await?;
    // todo: this removes all groups instead of just the kitchen group
    test.z2m
        .publish_topic("bridge/groups".to_string(), json!([]))?;
    test.z2m.publish_topic(
        "bridge/response/group/remove".to_string(),
        json!({"data":{"force":false,"id":zone.fixture_group.friendly_name},"status":"ok"}),
    )?;

    let zone_delete = hue_events.expect_delete_resource(zone.link).await?;
    assert_eq!(zone_delete.id, zone.link.rid);

    let grouped_light_service = *zone_resource.grouped_light_service().unwrap();
    let grouped_light_delete = hue_events
        .expect_delete_resource(grouped_light_service)
        .await?;
    assert_eq!(grouped_light_delete.id, grouped_light_service.rid);

    hue_events.expect_quiet().await?;

    Ok(())
}

#[tokio::test]
async fn delete_stale_zone() -> TestResult<()> {
    init();
    let mut test = TestBridge::start(z2m_state()).await?;
    test.wait_for_light(IKEA_COLOR_WITHOUT_ZONE).await?;
    let mut hue_events = test.hue_client.subscribe_events();
    let new_zone_link = test
        .hue_client
        .post_zone(ZoneNew {
            children: BTreeSet::new(),
            metadata: GroupMetadata {
                name: "stale".to_string(),
                archetype: GroupArchetype::Computer,
            },
        })
        .await?;
    let stale_zone = TestZone {
        link: new_zone_link,
        fixture_group: FixtureGroup {
            id: 1,
            friendly_name: "stale",
        },
    };

    let resource_record = hue_events.expect_add(RType::Zone).await?;
    let Resource::Zone(zone_resource) = resource_record.obj else {
        panic!("unexpected resource type");
    };
    assert_eq!(resource_record.id, new_zone_link.rid);
    assert_eq!(
        zone_resource,
        Zone {
            children: BTreeSet::new(),
            metadata: GroupMetadata {
                name: stale_zone.fixture_group.friendly_name.to_string(),
                archetype: GroupArchetype::Computer
            },
            services: BTreeSet::new()
        }
    );

    let link = test.hue_client.delete_zone(&stale_zone).await?;
    assert_eq!(link, stale_zone.link);

    let zone_delete = hue_events.expect_delete_resource(stale_zone.link).await?;
    assert_eq!(zone_delete.id, stale_zone.link.rid);

    hue_events.expect_quiet().await?;

    Ok(())
}

#[tokio::test]
async fn update_metadata() -> TestResult<()> {
    init();
    let mut test = TestBridge::start(z2m_state()).await?;
    let light1 = test.wait_for_light(IKEA_COLOR).await?;
    let light2 = test.wait_for_light(HUE_FLUX_LIGHTSTRIP).await?;
    let group_metadata = GroupMetadata {
        name: "kitchen1".to_string(),
        archetype: GroupArchetype::Home,
    };
    let zone = create_zone_and_assert(
        &test,
        test.z2m.only_backend(),
        &group_metadata,
        &[&light1, &light2],
    )
    .await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_zone(
            &zone,
            &json!({
                "metadata": {
                    "name": "kitchen2"
                }
            }),
        )
        .await?;

    let update = hue_events.expect_update_resource(zone.link).await?;

    assert_eq!(
        update.data,
        json!({
            "metadata": {
                "name": "kitchen2",
                "archetype": "home",
            }
        })
    );

    hue_events.expect_quiet().await?;
    z2m_requests.expect_quiet().await?;

    Ok(())
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn update_zone_children() -> TestResult<()> {
    init();
    let mut test = TestBridge::start(z2m_state()).await?;
    let ikea_color_without_zone_light = test.wait_for_light(IKEA_COLOR_WITHOUT_ZONE).await?;
    let ikea_color_light = test.wait_for_light(IKEA_COLOR).await?;
    let ikea_warm_white_light = test.wait_for_light(IKEA_WARM_WHITE).await?;
    let hue_white_ambiance_light = test.wait_for_light(HUE_WHITE_AMBIANCE).await?;
    let group_metadata = GroupMetadata {
        name: "group_with_members".to_string(),
        archetype: GroupArchetype::Carport,
    };
    let zone = create_zone_and_assert(
        &test,
        test.z2m.only_backend(),
        &group_metadata,
        &[
            &ikea_color_light,
            &ikea_warm_white_light,
            &hue_white_ambiance_light,
        ],
    )
    .await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_zone(
            &zone,
            &json!({
                "children": [
                    &&ikea_color_without_zone_light.link,
                    &ikea_color_light.link
                ]
            }),
        )
        .await?;

    z2m_requests
        .expect_requests_unordered(vec![
            (
                "bridge/request/group/members/add",
                json!({"device":IKEA_COLOR_WITHOUT_ZONE.topic(),"group":zone.topic()}),
            ),
            (
                "bridge/request/group/members/remove",
                json!({"device":IKEA_WARM_WHITE.topic(),"group":zone.topic()}),
            ),
            (
                "bridge/request/group/members/remove",
                json!({"device":&HUE_WHITE_AMBIANCE.topic(),"group":zone.topic()}),
            ),
        ])
        .await?;

    test.z2m.publish_topic("bridge/response/group/members/add".to_string(), json!({"data":{"device":IKEA_COLOR_WITHOUT_ZONE.topic(),"endpoint":"default","group":zone.topic()},"status":"ok"}))?;
    test.z2m.publish_topic(
        "bridge/groups".to_string(),
        json!([
            {"friendly_name": &zone.fixture_group.friendly_name, "id":&zone.fixture_group.id, "members":[
                {"endpoint":11,"ieee_address": IKEA_COLOR.ieee_address()},
                {"endpoint":11,"ieee_address": IKEA_WARM_WHITE.ieee_address()},
                {"endpoint":11,"ieee_address": HUE_WHITE_AMBIANCE.ieee_address()},
                {"endpoint":11,"ieee_address": IKEA_COLOR_WITHOUT_ZONE.ieee_address()}
                ],
             "scenes":[]}
        ]),
    )?;
    test.z2m.publish_topic("bridge/response/group/members/remove".to_string(), json!({"data":{"device":IKEA_WARM_WHITE.topic(),"endpoint":"default","group":zone.topic()},"status":"ok"}))?;
    test.z2m.publish_topic(
        "bridge/groups".to_string(),
        json!([
            {"friendly_name": &zone.fixture_group.friendly_name, "id":&zone.fixture_group.id, "members":[
                {"endpoint":11,"ieee_address": IKEA_COLOR.ieee_address()},
                {"endpoint":11,"ieee_address": HUE_WHITE_AMBIANCE.ieee_address()},
                {"endpoint":11,"ieee_address": IKEA_COLOR_WITHOUT_ZONE.ieee_address()}
                ],
             "scenes":[]}
        ]),
    )?;
    test.z2m.publish_topic("bridge/response/group/members/remove".to_string(), json!({"data":{"device":HUE_WHITE_AMBIANCE.topic(),"endpoint":"default","group":zone.topic()},"status":"ok"}))?;
    test.z2m.publish_topic(
        "bridge/groups".to_string(),
        json!([
            {"friendly_name": &zone.fixture_group.friendly_name, "id":&zone.fixture_group.id, "members":[
                {"endpoint":11,"ieee_address": IKEA_COLOR.ieee_address()},
                {"endpoint":11,"ieee_address": IKEA_COLOR_WITHOUT_ZONE.ieee_address()}
                ],
             "scenes":[]}
        ]),
    )?;

    assert_eq!(
        hue_events.expect_update_resource(zone.link).await?.data,
        json!({
           "children": [
                hue_white_ambiance_light.link,
                ikea_warm_white_light.link,
                ikea_color_without_zone_light.link,
                ikea_color_light.link,
            ],
        })
    );

    assert_eq!(
        hue_events.expect_update_resource(zone.link).await?.data,
        json!({
           "children": [
                hue_white_ambiance_light.link,
                ikea_color_without_zone_light.link,
                ikea_color_light.link,
            ],
        })
    );

    assert_eq!(
        hue_events.expect_update_resource(zone.link).await?.data,
        json!({
           "children": [
                ikea_color_without_zone_light.link,
                ikea_color_light.link,
            ],
        })
    );

    hue_events.expect_quiet().await?;
    z2m_requests.expect_quiet().await?;

    Ok(())
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn handle_z2m_group_changes() -> TestResult<()> {
    init();
    let mut test = TestBridge::start(z2m_state()).await?;
    let ikea_color_without_room_device = test.wait_for_light(IKEA_COLOR_WITHOUT_ZONE).await?;
    let ikea_color_light = test.wait_for_light(IKEA_COLOR).await?;
    let ikea_warm_white_light = test.wait_for_light(IKEA_WARM_WHITE).await?;
    let hue_white_ambiance_light = test.wait_for_light(HUE_WHITE_AMBIANCE).await?;
    let group_metadata = GroupMetadata {
        name: "z2m-group-changes".to_string(),
        archetype: GroupArchetype::Carport,
    };
    let zone = create_zone_and_assert(
        &test,
        test.z2m.only_backend(),
        &group_metadata,
        &[
            &ikea_color_light,
            &ikea_warm_white_light,
            &hue_white_ambiance_light,
        ],
    )
    .await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    // Simulate changing members in z2m

    // Add
    test.z2m.publish_topic(
        "bridge/response/group/members/add".to_string(),
        json!(
        {
            "data":{
                "device":IKEA_COLOR_WITHOUT_ZONE.topic(),
                "endpoint":"default",
                "group": zone.fixture_group.id
            },
            "status":"ok"
        }),
    )?;
    test.z2m.publish_topic(
        "bridge/groups".to_string(),
        json!([
            {
                "friendly_name": &zone.fixture_group.friendly_name,
                "id":&zone.fixture_group.id,
                "members":[
                    {"endpoint":1,"ieee_address": IKEA_COLOR.ieee_address()},
                    {"endpoint":1,"ieee_address": IKEA_WARM_WHITE.ieee_address()},
                    {"endpoint":11,"ieee_address": HUE_WHITE_AMBIANCE.ieee_address()},
                    {"endpoint":1,"ieee_address": IKEA_COLOR_WITHOUT_ZONE.ieee_address()}
                ],
                "scenes":[]
            }
        ]),
    )?;

    assert_eq!(
        hue_events.expect_update_resource(zone.link).await?.data,
        json!({
           "children": [
                hue_white_ambiance_light.link,
                ikea_warm_white_light.link,
                ikea_color_without_room_device.link,
                ikea_color_light.link,
            ],
        })
    );

    test.z2m.publish_topic(
        "bridge/response/group/members/remove".to_string(),
        json!(
        {
            "data":{
                "device":IKEA_WARM_WHITE.topic(),
                "endpoint":"default",
                "group":zone.fixture_group.id
            },
            "status":"ok"
        }),
    )?;
    test.z2m.publish_topic(
        "bridge/groups".to_string(),
        json!([
            {
                "friendly_name": &zone.fixture_group.friendly_name,
                "id":&zone.fixture_group.id,
                "members":[
                    {"endpoint":1,"ieee_address": IKEA_COLOR.ieee_address()},
                    {"endpoint":11,"ieee_address": HUE_WHITE_AMBIANCE.ieee_address()},
                    {"endpoint":1,"ieee_address": IKEA_COLOR_WITHOUT_ZONE.ieee_address()}
                ],
                "scenes":[]
            }
        ]),
    )?;

    assert_eq!(
        hue_events.expect_update_resource(zone.link).await?.data,
        json!({
           "children": [
                hue_white_ambiance_light.link,
                ikea_color_without_room_device.link,
                ikea_color_light.link,
            ],
        })
    );

    hue_events.expect_quiet().await?;
    z2m_requests.expect_quiet().await?;

    Ok(())
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn z2m_delete_group() -> TestResult<()> {
    init();
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(IKEA_COLOR_WITHOUT_ZONE).await?;
    let group_metadata = GroupMetadata {
        name: "z2m-group-changes".to_string(),
        archetype: GroupArchetype::Carport,
    };
    let zone =
        create_zone_and_assert(&test, test.z2m.only_backend(), &group_metadata, &[&light]).await?;
    let zone_grouped_light = test.hue_client.get_zone(&zone).await?;
    let grouped_light_link = *zone_grouped_light.grouped_light_service().unwrap();

    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.z2m.publish_topic(
        "bridge/response/group/remove".to_string(),
        json!(
        {
            "data":{
                "force":false,
                "id":&zone.fixture_group.id.to_string(),
            },
            "status":"ok",
            "transaction":"cnpmk-2"
        }),
    )?;

    assert_eq!(
        hue_events
            .expect_delete_resource(grouped_light_link)
            .await?
            .id,
        grouped_light_link.rid
    );

    assert_eq!(
        hue_events.expect_delete_resource(zone.link).await?.id,
        zone.link.rid
    );

    hue_events.expect_quiet().await?;
    z2m_requests.expect_quiet().await?;

    Ok(())
}
