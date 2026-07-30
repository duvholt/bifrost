use hue::api::ResourceRecord;
use memory2mqtt::service::M2mMode;
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::common::{HueClipResponse, TestBridge, TestResult};

pub mod common;

fn z2m_state() -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "bridge/devices".to_string(),
            json!([{
                "description": null,
                "date_code": null,
                "definition": {
                    "model": "LCT001",
                    "vendor": "Test",
                    "description": "Integration test light",
                    "supports_ota": false,
                    "options": [],
                    "exposes": [{
                        "type": "light",
                        "features": [
                            {"type": "binary", "name": "state", "value_off": "OFF", "value_on": "ON"},
                            {"type": "numeric", "name": "brightness", "value_min": 0, "value_max": 254},
                            {"type": "numeric", "name": "color_temp", "unit": "mired", "value_min": 153, "value_max": 500},
                            {"type": "composite", "name": "color_xy"}
                        ]
                    }]
                },
                "disabled": false,
                "endpoints": {},
                "friendly_name": "desk",
                "ieee_address": "0x0017880100000001",
                "interview_completed": true,
                "interviewing": false,
                "manufacturer": "Test",
                "model_id": "LCT001",
                "network_address": 1,
                "software_build_id": null,
                "supported": true,
                "type": "Router"
            }]),
        ),
        (
            "bridge/groups".to_string(),
            json!([{
                "id": 1,
                "friendly_name": "office",
                "members": [{"ieee_address": "0x0017880100000001", "endpoint": 1}],
                "scenes": [{"id": 7, "name": "Relax"}]
            }]),
        ),
        (
            "desk".to_string(),
            json!({
                "state": "OFF",
                "brightness": 10,
                "color_temp": 300,
                "color_mode": "color_temp",
                "color": {"x": 0.3, "y": 0.3}
            }),
        ),
    ])
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

    let lights = test_bridge
        .hue_client
        .get::<HueClipResponse<ResourceRecord>>("/clip/v2/resource/light")
        .await?;

    let light_id = lights.data[0].id;

    test_bridge.clear_events();
    test_bridge.z2m.clear_requests();

    test_bridge
        .hue_client
        .put(
            &format!("/clip/v2/resource/light/{light_id}"),
            &json!({
                "on": {
                    "on": true
                }
            }),
        )
        .await?;

    test_bridge
        .z2m
        .expect_request("desk/set", json!({"state": "ON"}))
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

    let lights = test_bridge
        .hue_client
        .get::<HueClipResponse<ResourceRecord>>("/clip/v2/resource/light")
        .await?;
    let light_id = lights.data[0].id;

    test_bridge.clear_events();
    test_bridge.z2m.clear_requests();

    test_bridge
        .hue_client
        .put(
            &format!("/clip/v2/resource/light/{light_id}"),
            &json!({"on": {"on": true}}),
        )
        .await?;

    test_bridge
        .z2m
        .expect_request("desk/set", json!({"state": "ON"}))
        .await?;

    test_bridge.z2m.send("desk", json!({"state": "ON"}))?;

    let update = test_bridge.wait_for_event_update(light_id).await?;
    assert_eq!(update.data["on"], json!({"on": true}));

    Ok(())
}
