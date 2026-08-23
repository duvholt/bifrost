use hue::api::{
    DimmingUpdate, Group, GroupArchetype, GroupMetadata, GroupedLight, LightAlert, LightSignal,
    LightSignaling, On, RType, Resource, Room, RoomNew, Stub,
};
#[cfg(test)]
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::common::fixture::{self, FixtureDevice, FixtureGroup, Z2mFixture, Z2mGroupFixture};
use crate::common::{TestBridge, TestResult, TestRoom};

pub mod common;

const LIVING_ROOM_GROUP: FixtureGroup = FixtureGroup {
    id: 1,
    friendly_name: "living_room",
};

const KITCHEN_GROUP: FixtureGroup = FixtureGroup {
    id: 2,
    friendly_name: "kitchen",
};

const IKEA_COLOR_WITHOUT_ROOM: FixtureDevice = FixtureDevice {
    id: 7,
    friendly_name: "ikea_color_without_room",
};

fn z2m_state() -> BTreeMap<String, Value> {
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

    let ikea_warm_white = fixture::ikea::tradfri_warm_white(&FixtureDevice {
        id: 1,
        friendly_name: "ikea_warm_white",
    })
    .with_state(json!({
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

    let ikea_color_without_room = fixture::ikea::tradfri_color(&IKEA_COLOR_WITHOUT_ROOM)
        .with_state(json!({
            "state": "OFF",
            "brightness": 127
        }));

    let living_room = Z2mGroupFixture::new(&LIVING_ROOM_GROUP)
        .with_member(&hue_flux_lightstrip)
        .with_member(&hue_play_gradient_lightstrip)
        .with_member(&misc_dimmer_light)
        .with_state(json!({
            "state": "OFF",
            "brightness": 0
        }));

    let kitchen = Z2mGroupFixture::new(&KITCHEN_GROUP)
        .with_member(&ikea_warm_white)
        .with_member(&ikea_color)
        .with_member(&hue_white_ambiance)
        .with_state(json!({
            "state": "ON",
            "brightness": 254
        }));

    let fixture = Z2mFixture::new()
        .with_device(ikea_warm_white)
        .with_device(ikea_color)
        .with_device(hue_white_ambiance)
        .with_device(hue_flux_lightstrip)
        .with_device(hue_play_gradient_lightstrip)
        .with_device(misc_dimmer_light)
        .with_device(ikea_color_without_room)
        .with_group(kitchen)
        .with_group(living_room);

    fixture.into_state()
}

#[tokio::test]
async fn get_rooms() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;

    let living_room = test.wait_for_room(LIVING_ROOM_GROUP).await?;
    let kitchen = test.wait_for_room(KITCHEN_GROUP).await?;

    let mut rooms = test.hue_client.get_rooms().await?;
    rooms.sort_by_key(|r| r.metadata.name.clone());

    let living_room_response = test.hue_client.get_room(&living_room).await?;
    assert_eq!(living_room_response.services.len(), 1);
    assert_eq!(living_room_response.children.len(), 3);
    assert_eq!(living_room_response.metadata.name, "living_room");

    let kitchen_response = test.hue_client.get_room(&kitchen).await?;
    assert_eq!(kitchen_response.services.len(), 1);
    assert_eq!(kitchen_response.children.len(), 3);
    assert_eq!(kitchen_response.metadata.name, "kitchen");

    assert_eq!(vec![kitchen_response, living_room_response], rooms);
    Ok(())
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn create_room() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light_device = test.wait_for_device(IKEA_COLOR_WITHOUT_ROOM).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    let new_room_link = test
        .hue_client
        .post_room(RoomNew {
            children: BTreeSet::from([light_device.link()]),
            metadata: GroupMetadata {
                name: "new1".to_string(),
                archetype: GroupArchetype::Barbecue,
            },
        })
        .await?;

    // Initial created room
    z2m_requests
        .expect_request(
            "bridge/request/group/add",
            json!({"id":3, "friendly_name":"new1"}),
        )
        .await?;
    let resource_record = hue_events.expect_add(RType::Room).await?;
    let Resource::Room(room_resource) = resource_record.obj else {
        panic!("unexpected resource type");
    };
    assert_eq!(resource_record.id, new_room_link.rid);
    assert_eq!(
        room_resource,
        Room {
            children: BTreeSet::from([light_device.link()]),
            metadata: GroupMetadata {
                name: "new1".to_string(),
                archetype: GroupArchetype::Barbecue
            },
            services: BTreeSet::new()
        }
    );
    let test_room = TestRoom {
        link: new_room_link,
        fixture_group: FixtureGroup {
            id: 3,
            friendly_name: "new1",
        },
    };

    // Group add response

    // todo: we need to append the group instead of overwriting all groups with just the new one
    // this should likely be handled in the z2m emulator
    test.z2m.publish_topic("bridge/groups".to_string(), json!([
        {"friendly_name": &test_room.fixture_group.friendly_name, "id":&test_room.fixture_group.id, "members":[], "scenes":[]}
    ]))?;
    test.z2m.publish_topic("bridge/response/group/add".to_string(), json!(
        {"data":{"friendly_name": &test_room.fixture_group.friendly_name,"id": &test_room.fixture_group.id},"status":"ok"}
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
            owner: new_room_link,
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
    let room_update = hue_events.expect_update_resource(new_room_link).await?;
    assert_eq!(
        room_update.data,
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
    z2m_requests
        .expect_request(
            "bridge/request/group/members/add",
            json!({"device": IKEA_COLOR_WITHOUT_ROOM.friendly_name, "group":"new1"}),
        )
        .await?;
    test.z2m.publish_topic("bridge/response/group/members/add".to_string(), json!(
        {"data":{"device": IKEA_COLOR_WITHOUT_ROOM.friendly_name,"endpoint":"default","group": &test_room.fixture_group.friendly_name},"status":"ok"}
    ))?;
    test.z2m.publish_topic(
        "bridge/groups".to_string(),
        json!([
            {"friendly_name": &test_room.fixture_group.friendly_name, "id":&test_room.fixture_group.id, "members":[{"endpoint":11,"ieee_address": IKEA_COLOR_WITHOUT_ROOM.ieee_address()}],
             "scenes":[]}
        ]),
    )?;
    let room_update = hue_events.expect_update_resource(new_room_link).await?;
    assert_eq!(
        room_update.data,
        json!({
            "children": [light_device.link()],
        })
    );

    // Room added to bridge home
    let bridge_home_update = hue_events.expect_update_type(RType::BridgeHome).await?;
    assert_eq!(
        bridge_home_update.data,
        json!(
             {
                "children": [
                    {
                        "rid": "242e5082-74bc-5dc1-865b-cd0649682ec2",
                        "rtype": "room",
                    },
                    {
                        "rid": "5d129726-6c45-59ad-9c6f-a81ab81f7532",
                        "rtype": "room",
                    },
                    {
                        "rid": "7a16c640-6be0-583f-a3f3-5c5d941bada5",
                        "rtype": "device",
                    },
                    {
                        "rid": new_room_link.rid,
                        "rtype": "room",
                    },
                ],
            }
        )
    );

    hue_events.expect_quiet().await?;

    Ok(())
}

#[tokio::test]
async fn delete_room() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let kitchen_room = test.wait_for_room(KITCHEN_GROUP).await?;
    let kitchen_room_resource = test.hue_client.get_room(&kitchen_room).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    let link = test.hue_client.delete_room(&kitchen_room).await?;
    assert_eq!(link, kitchen_room.link);

    z2m_requests
        .expect_request(
            "bridge/request/group/remove",
            json!({"force":false,"id": kitchen_room.fixture_group.friendly_name}),
        )
        .await?;
    // todo: this removes all groups instead of just the kitchen group
    test.z2m
        .publish_topic("bridge/groups".to_string(), json!([]))?;
    test.z2m.publish_topic(
        "bridge/response/group/remove".to_string(),
        json!({"data":{"force":false,"id":kitchen_room.fixture_group.friendly_name},"status":"ok"}),
    )?;

    let bridge_home_update = hue_events.expect_update_type(RType::BridgeHome).await?;
    assert_eq!(
        bridge_home_update.data,
        json!(
             {
                "children": [
                    {
                        "rid": "242e5082-74bc-5dc1-865b-cd0649682ec2",
                        "rtype": "room",
                    },
                    {
                        "rid": "7a16c640-6be0-583f-a3f3-5c5d941bada5",
                        "rtype": "device",
                    },
                ],
            }
        )
    );
    let room_delete = hue_events.expect_delete_resource(kitchen_room.link).await?;
    assert_eq!(room_delete.id, kitchen_room.link.rid);

    let grouped_light_service = *kitchen_room_resource.grouped_light_service().unwrap();
    let grouped_light_delete = hue_events
        .expect_delete_resource(grouped_light_service)
        .await?;
    assert_eq!(grouped_light_delete.id, grouped_light_service.rid);

    hue_events.expect_quiet().await?;

    Ok(())
}

#[tokio::test]
async fn delete_stale_room() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    test.wait_for_room(KITCHEN_GROUP).await?;
    test.wait_for_room(LIVING_ROOM_GROUP).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    let new_room_link = test
        .hue_client
        .post_room(RoomNew {
            children: BTreeSet::new(),
            metadata: GroupMetadata {
                name: "stale".to_string(),
                archetype: GroupArchetype::Computer,
            },
        })
        .await?;

    // Initial created room
    z2m_requests
        .expect_request(
            "bridge/request/group/add",
            json!({"id":3, "friendly_name":"stale"}),
        )
        .await?;
    let resource_record = hue_events.expect_add(RType::Room).await?;
    let Resource::Room(room_resource) = resource_record.obj else {
        panic!("unexpected resource type");
    };
    assert_eq!(resource_record.id, new_room_link.rid);
    assert_eq!(
        room_resource,
        Room {
            children: BTreeSet::new(),
            metadata: GroupMetadata {
                name: "stale".to_string(),
                archetype: GroupArchetype::Computer
            },
            services: BTreeSet::new()
        }
    );
    let stale_room = TestRoom {
        link: new_room_link,
        fixture_group: FixtureGroup {
            id: 3,
            friendly_name: "stale",
        },
    };
    let grouped_light_record = hue_events.expect_add(RType::GroupedLight).await?;

    // simulate error in z2m by not sending any responses

    let link = test.hue_client.delete_room(&stale_room).await?;
    assert_eq!(link, stale_room.link);

    let room_delete = hue_events.expect_delete_resource(stale_room.link).await?;
    assert_eq!(room_delete.id, stale_room.link.rid);

    hue_events
        .expect_delete_resource(grouped_light_record.link())
        .await?;

    hue_events.expect_quiet().await?;

    Ok(())
}

#[tokio::test]
async fn update_metadata() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let kitchen_group = test.wait_for_room(KITCHEN_GROUP).await?;
    test.wait_for_room(LIVING_ROOM_GROUP).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_room(
            &kitchen_group,
            &json!({
                "metadata": {
                    "name": "kitchen2"
                }
            }),
        )
        .await?;

    let update = hue_events
        .expect_update_resource(kitchen_group.link)
        .await?;

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
