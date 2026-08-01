use hue::api::ResourceRecord;
use memory2mqtt::service::M2mMode;
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::common::{
    HueClipResponse, TestBridge, TestResult,
    fixture::{self, Z2mFixture, Z2mFixtureDeviceId, Z2mGroupFixture},
};

pub mod common;

fn z2m_state() -> BTreeMap<String, Value> {
    let lamp = fixture::ikea::tradfri_warm_white(Z2mFixtureDeviceId(1), "lamp").with_state(json!({
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

    test_bridge
        .wait_for_event_add(hue::api::RType::Light)
        .await?;

    let lights = test_bridge
        .hue_client
        .get::<HueClipResponse<ResourceRecord>>("/clip/v2/resource/light")
        .await?;

    assert_eq!(lights.data.len(), 1);
    Ok(())
}

#[tokio::test]
async fn turn_on_light() -> TestResult<()> {
    let mut test_bridge = TestBridge::start(z2m_state()).await?;

    test_bridge
        .wait_for_event_add(hue::api::RType::Light)
        .await?;

    let lights = test_bridge.hue_client.get_lights().await?;
    let light_id = lights.data[0].id;

    test_bridge.clear_events();
    test_bridge.z2m.clear_requests();

    test_bridge
        .hue_client
        .put_light(
            light_id,
            &json!({
                "on": {
                    "on": true
                }
            }),
        )
        .await?;

    test_bridge
        .z2m
        .expect_request("lamp/set", json!({"state": "ON"}))
        .await?;

    let update = test_bridge.wait_for_event_update(light_id).await?;

    assert_eq!(update.data["on"], json!({"on": true}));

    Ok(())
}

#[tokio::test]
async fn turn_on_light_with_manual_z2m() -> TestResult<()> {
    let mut test_bridge = TestBridge::start_with_z2m_mode(z2m_state(), M2mMode::Manual).await?;

    test_bridge
        .wait_for_event_add(hue::api::RType::Light)
        .await?;

    let lights = test_bridge.hue_client.get_lights().await?;
    let light_id = lights.data[0].id;

    test_bridge.clear_events();
    test_bridge.z2m.clear_requests();

    test_bridge
        .hue_client
        .put_light(light_id, &json!({"on": {"on": true}}))
        .await?;

    test_bridge
        .z2m
        .expect_request("lamp/set", json!({"state": "ON"}))
        .await?;

    test_bridge.z2m.send("lamp", json!({"state": "ON"}))?;

    let update = test_bridge.wait_for_event_update(light_id).await?;
    assert_eq!(update.data["on"], json!({"on": true}));

    Ok(())
}
