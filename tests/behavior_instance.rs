use std::time::Duration;

use chrono::{Local, TimeZone};
use hue::api::BehaviorScript;
use serde_json::json;

use crate::common::fixture::{self, FixtureDevice, FixtureGroup, Z2mFixture, Z2mGroupFixture};
use crate::common::{TestBridge, TestResult, TestZ2mDeviceOrGroup};

pub mod common;

#[tokio::test]
async fn hue_wakeup_runs_sunrise_effect_and_disables_itself() -> TestResult<()> {
    const LIGHT: FixtureDevice = FixtureDevice {
        id: 1,
        friendly_name: "bedroom_hue_light",
    };
    const ROOM: FixtureGroup = FixtureGroup {
        id: 1,
        friendly_name: "bedroom",
    };
    let state = {
        let light = fixture::hue::white_ambience(&LIGHT).with_state(json!({"state": "OFF"}));
        let room = Z2mGroupFixture::new(&ROOM)
            .with_member(&light)
            .with_state(json!({"state": "OFF"}));
        Z2mFixture::new()
            .with_device(light)
            .with_group(room)
            .into_state()
    };
    let mut test = TestBridge::start(state).await?;
    test.clock.set_now(
        Local
            .with_ymd_and_hms(2020, 1, 1, 6, 0, 0)
            .single()
            .unwrap(),
    );
    let light = test.wait_for_light(LIGHT).await?;
    let room = test.wait_for_room(&ROOM).await?;
    let mut requests = test.z2m.subscribe_requests();
    let mut events = test.hue_client.subscribe_events();

    let instance = test
        .hue_client
        .post(
            "/clip/v2/resource/behavior_instance",
            &json!({
                "script_id": BehaviorScript::WAKE_UP_ID,
                "enabled": true,
                "metadata": {"name": "Hue sunrise wake up"},
                "configuration": {
                    "end_brightness": 80.0,
                    "fade_in_duration": {"seconds": 10 * 60},
                    "style": "sunrise",
                    "when": {"time_point": {"type": "time", "time": {"hour": 7, "minute": 0}}},
                    "where": [{"group": room.link}]
                }
            }),
        )
        .await?;

    let start = test.clock.next_sleep().await?;
    assert_eq!(start.duration, Duration::from_mins(50));
    requests.expect_quiet().await?;
    start.release();

    requests
        .expect_set(
            &light,
            json!({
                "command": {
                    "cluster": 0xFC03,
                    "command": 0,
                    "payload": {"data": [
                        0xB3, 0x00, // Flags: on/off, brightness, fade speed, effect, effect speed
                        0x01,       // On
                        203,        // 80% brightness
                        0x01, 0x00, // Fade speed
                        0x09,       // Sunrise effect
                        125         // Ten-minute effect duration
                    ]}
                }
            }),
        )
        .await?;
    requests
        .expect_request(&format!("{}/get", light.topic()), json!({"state": ""}))
        .await?;

    let finish = test.clock.next_sleep().await?;
    assert_eq!(finish.duration, Duration::from_mins(10));
    let before = test.hue_client.get_behavior_instance(instance).await?;
    assert!(before.enabled, "behavior instance should be enabled");
    finish.release();

    let update = events.expect_update_resource(instance).await?;
    assert_eq!(update.data["enabled"], false);
    let after = test.hue_client.get_behavior_instance(instance).await?;
    assert!(!after.enabled, "behavior instance should be disabled");
    requests.expect_quiet().await?;
    Ok(())
}

#[tokio::test]
async fn basic_wakeup_fades_room_and_disables_itself() -> TestResult<()> {
    const LIGHT: FixtureDevice = FixtureDevice {
        id: 1,
        friendly_name: "bedroom_light",
    };
    const ROOM: FixtureGroup = FixtureGroup {
        id: 1,
        friendly_name: "bedroom",
    };
    let state = {
        let light = fixture::ikea::tradfri_color(&LIGHT).with_state(json!({"state": "OFF"}));
        let room = Z2mGroupFixture::new(&ROOM)
            .with_member(&light)
            .with_state(json!({"state": "OFF"}));
        Z2mFixture::new()
            .with_device(light)
            .with_group(room)
            .into_state()
    };
    let mut test = TestBridge::start(state).await?;
    test.clock.set_now(
        Local
            .with_ymd_and_hms(2020, 1, 1, 6, 0, 0)
            .single()
            .unwrap(),
    );
    let room = test.wait_for_room(&ROOM).await?;
    let mut requests = test.z2m.subscribe_requests();
    let mut events = test.hue_client.subscribe_events();

    let instance = test
        .hue_client
        .post(
            "/clip/v2/resource/behavior_instance",
            &json!({
                "script_id": BehaviorScript::WAKE_UP_ID,
                "enabled": true,
                "metadata": {"name": "Basic wake up"},
                "configuration": {
                    "end_brightness": 80.0,
                    "fade_in_duration": {"seconds": 10 * 60},
                    "style": "basic",
                    "when": {"time_point": {"type": "time", "time": {"hour": 7, "minute": 0}}},
                    "where": [{"group": room.link}]
                }
            }),
        )
        .await?;

    let start = test.clock.next_sleep().await?;
    // (1h until 07:00) - (10min fade in)
    assert_eq!(start.duration, Duration::from_mins(50));
    requests.expect_quiet().await?;
    start.release();

    requests
        .expect_set(&room, json!({"state": "ON", "brightness": 2.54}))
        .await?;
    requests
        .expect_set(&room, json!({"color_temp": 447}))
        .await?;
    requests
        .expect_set(&room, json!({"brightness": 203.2, "transition": 600.0}))
        .await?;

    let finish = test.clock.next_sleep().await?;
    assert_eq!(finish.duration, Duration::from_mins(10));
    let before = test.hue_client.get_behavior_instance(instance).await?;
    assert!(before.enabled, "behavior instance should be enabled");
    finish.release();

    let update = events.expect_update_resource(instance).await?;
    assert_eq!(update.data["enabled"], false);
    let after = test.hue_client.get_behavior_instance(instance).await?;
    assert!(!after.enabled, "behavior instance should be disabled");
    requests.expect_quiet().await?;
    Ok(())
}
