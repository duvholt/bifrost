use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::common::{
    TestBridge, TestResult,
    fixture::{self, FixtureDevice, Z2mFixture, Z2mGroupFixture},
};

pub mod common;

const LAMP: FixtureDevice = FixtureDevice {
    id: 1,
    friendly_name: "lamp",
};

fn z2m_state() -> BTreeMap<String, Value> {
    let lamp = fixture::ikea::tradfri_warm_white(&LAMP).with_state(json!({
        "state": "OFF",
        "brightness": 100
    }));

    let living_room = Z2mGroupFixture::new(1, "livingroom")
        .with_member(&lamp)
        .with_scene(1, "Relax")
        .with_state(json!({
            "state": "OFF",
            "brightness": 100
        }));

    let fixture = Z2mFixture::new().with_device(lamp).with_group(living_room);

    fixture.into_state()
}

#[tokio::test]
async fn get_lights() -> TestResult<()> {
    let mut test_bridge = TestBridge::start(z2m_state()).await?;

    test_bridge.wait_for_light(LAMP).await?;

    let lights = test_bridge.hue_client.get_lights().await?;

    assert_eq!(lights.data.len(), 1);
    Ok(())
}

#[tokio::test]
async fn turn_on_light() -> TestResult<()> {
    let mut test_bridge = TestBridge::start(z2m_state()).await?;

    let light = test_bridge.wait_for_light(LAMP).await?;

    let mut z2m_requests = test_bridge.z2m.subscribe_requests();
    let mut hue_events = test_bridge.hue_client.subscribe_events();

    test_bridge
        .hue_client
        .put_light(&light, &json!({"on": {"on": true}}))
        .await?;

    z2m_requests
        .expect_set(&light, json!({"state": "ON"}))
        .await?;

    hue_events.expect_quiet().await?;

    test_bridge.z2m.publish(&light, json!({"state": "ON"}))?;

    let update = hue_events.expect_update(&light).await?;
    assert_eq!(update.data["on"], json!({"on": true}));

    hue_events.expect_quiet().await?;

    Ok(())
}
