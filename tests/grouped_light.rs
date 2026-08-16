use hue::api::{
    DimmingUpdate, GroupedLight, LightAlert, LightSignal, LightSignaling, On, ResourceLink, Stub,
};
#[cfg(test)]
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::common::fixture::{self, FixtureDevice, FixtureGroup, Z2mFixture, Z2mGroupFixture};
use crate::common::{TestBridge, TestResult};

pub mod common;

const LIVING_ROOM_GROUP: FixtureGroup = FixtureGroup {
    id: 1,
    friendly_name: "living_room",
};

const KITCHEN_GROUP: FixtureGroup = FixtureGroup {
    id: 2,
    friendly_name: "kitchen",
};

const ALL_GROUP: FixtureGroup = FixtureGroup {
    id: 3,
    friendly_name: "all",
};

fn expected_grouped_light(owner: ResourceLink, on: bool, brightness: f64) -> GroupedLight {
    GroupedLight {
        owner,
        signaling: Some(LightSignaling {
            signal_values: vec![
                LightSignal::Alternating,
                LightSignal::NoSignal,
                LightSignal::OnOff,
                LightSignal::OnOffColor,
            ],
            status: Value::Null,
        }),
        alert: Some(LightAlert {
            action_values: BTreeSet::from(["breathe".to_string()]),
        }),
        dimming: Some(DimmingUpdate { brightness }),
        color: Some(Stub),
        color_temperature: Some(Stub),
        color_temperature_delta: Some(Stub),
        dimming_delta: Some(Stub),
        dynamics: Some(Stub),
        on: Some(On::new(on)),
    }
}

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

    let all = Z2mGroupFixture::new(&ALL_GROUP)
        .with_member(&hue_flux_lightstrip)
        .with_member(&hue_play_gradient_lightstrip)
        .with_member(&misc_dimmer_light)
        .with_member(&ikea_warm_white)
        .with_member(&ikea_color)
        .with_member(&hue_white_ambiance)
        .with_state(json!({
            "state": "ON",
            "brightness": 127
        }));

    let fixture = Z2mFixture::new()
        .with_device(ikea_warm_white)
        .with_device(ikea_color)
        .with_device(hue_white_ambiance)
        .with_device(hue_flux_lightstrip)
        .with_device(hue_play_gradient_lightstrip)
        .with_device(misc_dimmer_light)
        .with_group(kitchen)
        .with_group(living_room)
        .with_group(all);

    fixture.into_state()
}

#[tokio::test]
async fn get_grouped_lights() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;

    let living_room = test.wait_for_grouped_light(LIVING_ROOM_GROUP).await?;
    let kitchen = test.wait_for_grouped_light(KITCHEN_GROUP).await?;
    let all = test.wait_for_grouped_light(ALL_GROUP).await?;

    {
        let grouped_light = test.hue_client.get_grouped_light(&living_room).await?;
        assert_eq!(
            grouped_light,
            expected_grouped_light(grouped_light.owner, false, 0.0),
        );
    };

    {
        let grouped_light = test.hue_client.get_grouped_light(&kitchen).await?;
        assert_eq!(
            expected_grouped_light(grouped_light.owner, true, 100.0),
            grouped_light,
        );
    };

    {
        let grouped_light = test.hue_client.get_grouped_light(&all).await?;
        assert_eq!(
            expected_grouped_light(grouped_light.owner, true, 50.0),
            grouped_light,
        );
    };

    Ok(())
}

#[tokio::test]
async fn turn_on_grouped_light() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let grouped_light = test.wait_for_grouped_light(LIVING_ROOM_GROUP).await?;
    let baseline = test.hue_client.get_grouped_light(&grouped_light).await?;
    assert_eq!(baseline.on, Some(On::new(false)));
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_grouped_light(&grouped_light, &json!({"on": {"on": true}}))
        .await?;
    z2m_requests
        .expect_set(&grouped_light, json!({"state": "ON"}))
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m.publish(&grouped_light, json!({"state": "ON"}))?;

    let update = hue_events
        .expect_update_resource(grouped_light.link)
        .await?;
    assert_eq!(
        update.data,
        json!({
            "owner": baseline.owner,
            "on": {"on": true},
        })
    );

    let updated = test.hue_client.get_grouped_light(&grouped_light).await?;
    assert_eq!(
        updated,
        GroupedLight {
            on: Some(On::new(true)),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
async fn change_brightness() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let grouped_light = test.wait_for_grouped_light(LIVING_ROOM_GROUP).await?;
    let baseline = test.hue_client.get_grouped_light(&grouped_light).await?;
    assert_eq!(baseline.dimming.map(|d| d.brightness), Some(0.0));
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_grouped_light(
            &grouped_light,
            &json!({
              "dimming": {
                "brightness": 25
              }
            }),
        )
        .await?;
    z2m_requests
        .expect_set(&grouped_light, json!({"brightness": 63.5}))
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m
        .publish(&grouped_light, json!({"brightness": 63.5, "state":"ON"}))?;

    let update = hue_events
        .expect_update_resource(grouped_light.link)
        .await?;
    assert_eq!(
        update.data,
        json!({
            "owner": baseline.owner,
            "on": {"on": true},
            "dimming": {"brightness": 25.0},
        })
    );

    let updated = test.hue_client.get_grouped_light(&grouped_light).await?;
    assert_eq!(
        updated,
        GroupedLight {
            on: Some(On::new(true)),
            dimming: Some(DimmingUpdate { brightness: 25.0 }),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
async fn dimming_delta_up() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let grouped_light = test.wait_for_grouped_light(LIVING_ROOM_GROUP).await?;
    let baseline = test.hue_client.get_grouped_light(&grouped_light).await?;
    assert_eq!(baseline.dimming.map(|d| d.brightness), Some(0.0));
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_grouped_light(
            &grouped_light,
            &json!({
              "dimming_delta": {
                "action": "up",
                "brightness_delta": 25
              }
            }),
        )
        .await?;
    z2m_requests
        .expect_set(&grouped_light, json!({"brightness_step": 63.5}))
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m
        .publish(&grouped_light, json!({"brightness": 190.5, "state":"ON"}))?;

    let update = hue_events
        .expect_update_resource(grouped_light.link)
        .await?;
    assert_eq!(
        update.data,
        json!({
            "owner": baseline.owner,
            "on": {"on": true},
            "dimming": {"brightness": 75.0},
        })
    );

    let updated = test.hue_client.get_grouped_light(&grouped_light).await?;
    assert_eq!(
        updated,
        GroupedLight {
            on: Some(On::new(true)),
            dimming: Some(DimmingUpdate { brightness: 75.0 }),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
async fn dimming_delta_down() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let grouped_light = test.wait_for_grouped_light(KITCHEN_GROUP).await?;
    let baseline = test.hue_client.get_grouped_light(&grouped_light).await?;
    assert_eq!(baseline.dimming.map(|d| d.brightness), Some(100.0));
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_grouped_light(
            &grouped_light,
            &json!({
              "dimming_delta": {
                "action": "down",
                "brightness_delta": 50
              }
            }),
        )
        .await?;
    z2m_requests
        .expect_set(&grouped_light, json!({"brightness_step": -127.0}))
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m
        .publish(&grouped_light, json!({"brightness": 127.0, "state":"ON"}))?;

    let update = hue_events
        .expect_update_resource(grouped_light.link)
        .await?;
    assert_eq!(
        update.data,
        json!({
            "owner": baseline.owner,
            "dimming": {"brightness": 50.0},
        })
    );

    let updated = test.hue_client.get_grouped_light(&grouped_light).await?;
    assert_eq!(
        updated,
        GroupedLight {
            on: Some(On::new(true)),
            dimming: Some(DimmingUpdate { brightness: 50.0 }),
            ..baseline
        }
    );
    Ok(())
}
