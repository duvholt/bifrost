use eventsource_stream::Eventsource;
use hue::api::ResourceRecord;
use hue::event::{Event, EventBlock};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Duration};
use tokio::time::timeout;
use tokio_stream::StreamExt;

use crate::common::{HueClipResponse, TestBridge, TestError, TestResult};

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
    let test_bridge = TestBridge::start(z2m_state()).await?;

    let lights = reqwest::get(test_bridge.hue_url.join("/clip/v2/resource/light")?)
        .await?
        .json::<HueClipResponse<ResourceRecord>>()
        .await?;

    assert_eq!(lights.data.len(), 1);
    Ok(())
}

#[tokio::test]
async fn turn_on_light() -> TestResult<()> {
    let test_bridge = TestBridge::start(z2m_state()).await?;
    let client = reqwest::Client::new();

    let lights = client
        .get(test_bridge.hue_url.join("/clip/v2/resource/light")?)
        .send()
        .await?
        .json::<HueClipResponse<ResourceRecord>>()
        .await?;

    let light_id = lights.data[0].id;

    let mut event_stream = reqwest::Client::new()
        .get(test_bridge.hue_url.join("/eventstream/clip/v2")?)
        .send()
        .await?
        .bytes_stream()
        .eventsource();

    client
        .put(
            test_bridge
                .hue_url
                .join(&format!("/clip/v2/resource/light/{light_id}"))?,
        )
        .json(&json!({
            "on": {
                "on": true
            }
        }))
        .send()
        .await?
        .error_for_status()?;

    let update = timeout(Duration::from_secs(2), async {
        while let Some(message) = event_stream.next().await {
            let event = message?;
            if event.data.is_empty() {
                continue;
            }

            let blocks: Vec<EventBlock> = serde_json::from_str(&event.data)?;
            for block in blocks {
                let Event::Update(update) = block.event else {
                    continue;
                };

                if let Some(object) = update.data.into_iter().find(|object| object.id == light_id) {
                    return Ok(object);
                }
            }
        }
        Err(TestError::EventTimeout)
    })
    .await??;

    assert_eq!(update.data["on"], json!({"on": true}));

    Ok(())
}
