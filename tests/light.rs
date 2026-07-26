use hue::api::ResourceRecord;
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
async fn read_light() -> TestResult<()> {
    let test_bridge = TestBridge::start(z2m_state()).await?;

    let lights = reqwest::get(test_bridge.hue_url.join("/clip/v2/resource/light")?)
        .await?
        .json::<HueClipResponse<ResourceRecord>>()
        .await?;

    assert_eq!(lights.data.len(), 1);
    Ok(())
}
