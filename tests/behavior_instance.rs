use std::time::Duration;

use chrono::{DateTime, Local, TimeZone, Weekday};
use hue::api::{BehaviorScript, WakeupConfiguration, WakeupStyle, configuration};
use serde_json::json;

use crate::common::fixture::{self, FixtureDevice, FixtureGroup, Z2mFixture, Z2mGroupFixture};
use crate::common::{TestBridge, TestResult, TestZ2mDeviceOrGroup, Z2mRequests};

pub mod common;

const BASIC_LIGHT: FixtureDevice = FixtureDevice {
    id: 1,
    friendly_name: "basic_light",
};
const HUE_LIGHT: FixtureDevice = FixtureDevice {
    id: 2,
    friendly_name: "hue_light",
};
const BEDROOM: FixtureGroup = FixtureGroup {
    id: 1,
    friendly_name: "bedroom",
};

struct WakeupFixture {
    test: TestBridge,
    room: common::TestRoom<'static>,
    basic: common::TestLight,
    hue: common::TestLight,
}

async fn wakeup_fixture(now: DateTime<Local>) -> TestResult<WakeupFixture> {
    const BASIC_ON: FixtureDevice = FixtureDevice {
        id: 3,
        friendly_name: "basic_already_on",
    };
    const HUE_ON: FixtureDevice = FixtureDevice {
        id: 4,
        friendly_name: "hue_already_on",
    };
    let basic = fixture::ikea::tradfri_color(&BASIC_LIGHT).with_state(json!({"state": "OFF"}));
    let hue = fixture::hue::white_ambience(&HUE_LIGHT).with_state(json!({"state": "OFF"}));
    let basic_on = fixture::ikea::tradfri_color(&BASIC_ON).with_state(json!({"state": "ON"}));
    let hue_on = fixture::hue::white_ambience(&HUE_ON).with_state(json!({"state": "ON"}));
    let room = Z2mGroupFixture::new(&BEDROOM)
        .with_member(&basic)
        .with_member(&hue)
        .with_member(&basic_on)
        .with_member(&hue_on)
        .with_state(json!({"state": "OFF"}));
    let state = Z2mFixture::new()
        .with_device(basic)
        .with_device(hue)
        .with_device(basic_on)
        .with_device(hue_on)
        .with_group(room)
        .into_state();
    let mut test = TestBridge::start(state).await?;
    test.clock.set_now(now);
    let basic = test.wait_for_light(BASIC_LIGHT).await?;
    let hue = test.wait_for_light(HUE_LIGHT).await?;
    test.wait_for_light(BASIC_ON).await?;
    test.wait_for_light(HUE_ON).await?;
    let room = test.wait_for_room(&BEDROOM).await?;
    Ok(WakeupFixture {
        test,
        room,
        basic,
        hue,
    })
}

async fn expect_quiet_sleep_and_advance(
    test: &mut TestBridge,
    requests: &mut Z2mRequests,
    duration: Duration,
) -> TestResult<()> {
    let start = test.clock.expect_sleep(duration).await?;
    requests.expect_quiet().await?;
    test.clock.advance(duration);
    start.release();
    Ok(())
}

fn wednesday_at_six() -> DateTime<Local> {
    Local
        .with_ymd_and_hms(2020, 1, 1, 6, 0, 0)
        .single()
        .unwrap()
}

// A single basic fade that leaves the lights on after completion.
const fn one_time_basic_wakeup_configuration(
    end_brightness: f64,
    fade_in_duration: configuration::Duration,
    time: configuration::Time,
    where_field: Vec<configuration::Where>,
) -> WakeupConfiguration {
    WakeupConfiguration {
        end_brightness,
        fade_in_duration,
        turn_lights_off_after: None,
        style: Some(WakeupStyle::Basic),
        when: configuration::When {
            time_point: configuration::TimePoint::Time { time },
            recurrence_days: None,
        },
        where_field,
    }
}

fn wakeup_body(configuration: &WakeupConfiguration) -> serde_json::Value {
    json!({
        "script_id": BehaviorScript::WAKE_UP_ID,
        "enabled": true,
        "metadata": {"name": "Wake-up regression test"},
        "configuration": configuration
    })
}

fn individual_fade_payloads(brightness: f64, duration: Duration) -> [serde_json::Value; 3] {
    [
        json!({"state": "ON", "brightness": 2.54, "transition": 0.4}),
        json!({"color_temp": 447, "transition": 0.4}),
        json!({"brightness": brightness * 254.0 / 100.0, "transition": duration.as_secs_f64()}),
    ]
}

async fn expect_grouped_fade(
    z2m: &mut Z2mRequests,
    group: &(impl TestZ2mDeviceOrGroup + Sync),
    brightness: f64,
    duration: Duration,
) -> TestResult<()> {
    z2m.expect_set(group, json!({"state": "ON", "brightness": 2.54}))
        .await?;
    z2m.expect_set(group, json!({"color_temp": 447})).await?;
    z2m.expect_set(
        group,
        json!({
            "brightness": brightness * 254.0 / 100.0,
            "transition": duration.as_secs_f64(),
        }),
    )
    .await
}

async fn expect_individual_fade(
    z2m: &mut Z2mRequests,
    light: &(impl TestZ2mDeviceOrGroup + Sync),
    brightness: f64,
    duration: Duration,
) -> TestResult<()> {
    for payload in individual_fade_payloads(brightness, duration) {
        z2m.expect_set(light, payload).await?;
    }
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
    test.clock.set_now(wednesday_at_six());
    let room = test.wait_for_room(&ROOM).await?;
    let mut requests = test.z2m.subscribe_requests();

    let config = one_time_basic_wakeup_configuration(
        80.0,
        configuration::Duration { seconds: 600 },
        configuration::Time { hour: 7, minute: 0 },
        vec![configuration::Where {
            group: room.link,
            items: None,
        }],
    );
    let instance = test
        .hue_client
        .post("/clip/v2/resource/behavior_instance", &wakeup_body(&config))
        .await?;

    expect_quiet_sleep_and_advance(&mut test, &mut requests, Duration::from_mins(50)).await?;
    expect_grouped_fade(&mut requests, &room, 80.0, Duration::from_mins(10)).await?;

    let finish = test.clock.expect_sleep(Duration::from_mins(10)).await?;
    test.hue_client
        .expect_behavior_enabled(instance, true)
        .await?;
    test.clock.advance(Duration::from_mins(10));
    finish.release();

    test.hue_client
        .expect_behavior_enabled(instance, false)
        .await?;
    requests.expect_quiet().await?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
#[tokio::test]
async fn wakeup_fades_multiple_rooms_and_selected_lights_then_turns_them_off() -> TestResult<()> {
    const ROOMS: [FixtureGroup; 3] = [
        FixtureGroup {
            id: 1,
            friendly_name: "bedroom",
        },
        FixtureGroup {
            id: 2,
            friendly_name: "guest_room",
        },
        FixtureGroup {
            id: 3,
            friendly_name: "living_room",
        },
    ];
    const LIGHTS: [FixtureDevice; 6] = [
        FixtureDevice {
            id: 1,
            friendly_name: "bedroom_light",
        },
        FixtureDevice {
            id: 2,
            friendly_name: "guest_light",
        },
        FixtureDevice {
            id: 3,
            friendly_name: "sofa_light",
        },
        FixtureDevice {
            id: 4,
            friendly_name: "reading_light",
        },
        FixtureDevice {
            id: 5,
            friendly_name: "unselected_light",
        },
        FixtureDevice {
            id: 6,
            friendly_name: "selected_already_on",
        },
    ];

    // Two whole rooms, plus two selected lights in a third room.
    let [
        bedroom_light,
        guest_light,
        sofa_light,
        reading_light,
        unselected_light,
        already_on_light,
    ] = LIGHTS
        .map(|device| fixture::ikea::tradfri_color(&device).with_state(json!({"state": "OFF"})));
    // Basic whole-room fades include already-on members in the group commands.
    let bedroom_light = bedroom_light.with_state(json!({"state": "ON"}));
    let already_on_light = already_on_light.with_state(json!({"state": "ON"}));
    let bedroom = Z2mGroupFixture::new(&ROOMS[0])
        .with_member(&bedroom_light)
        .with_state(json!({"state": "OFF"}));
    let guest_room = Z2mGroupFixture::new(&ROOMS[1])
        .with_member(&guest_light)
        .with_state(json!({"state": "OFF"}));
    let living_room = Z2mGroupFixture::new(&ROOMS[2])
        .with_member(&sofa_light)
        .with_member(&reading_light)
        .with_member(&unselected_light)
        .with_member(&already_on_light)
        .with_state(json!({"state": "OFF"}));
    let fixture = Z2mFixture::new()
        .with_device(bedroom_light)
        .with_device(guest_light)
        .with_device(sofa_light)
        .with_device(reading_light)
        .with_device(unselected_light)
        .with_device(already_on_light)
        .with_group(bedroom)
        .with_group(guest_room)
        .with_group(living_room);

    let mut test = TestBridge::start(fixture.into_state()).await?;
    test.clock.set_now(wednesday_at_six());
    let mut lights = Vec::new();
    for device in LIGHTS {
        lights.push(test.wait_for_light(device).await?);
    }
    let bedroom = test.wait_for_room(&ROOMS[0]).await?;
    let guest_room = test.wait_for_room(&ROOMS[1]).await?;
    let living_room = test.wait_for_room(&ROOMS[2]).await?;
    let selected_lights = &lights[2..4];

    let mut config = one_time_basic_wakeup_configuration(
        80.0,
        configuration::Duration { seconds: 600 },
        configuration::Time { hour: 7, minute: 0 },
        vec![
            configuration::Where {
                group: bedroom.link,
                items: None,
            },
            configuration::Where {
                group: guest_room.link,
                items: None,
            },
            configuration::Where {
                group: living_room.link,
                // Already-on selected lights receive neither fade nor off commands.
                items: Some(vec![
                    selected_lights[0].link,
                    selected_lights[1].link,
                    lights[5].link,
                ]),
            },
        ],
    );
    config.turn_lights_off_after = Some(configuration::Duration { seconds: 300 });
    let body = wakeup_body(&config);
    let mut requests = test.z2m.subscribe_requests();
    let instance = test
        .hue_client
        .post("/clip/v2/resource/behavior_instance", &body)
        .await?;

    expect_quiet_sleep_and_advance(&mut test, &mut requests, Duration::from_mins(50)).await?;

    for room in [&bedroom, &guest_room] {
        expect_grouped_fade(&mut requests, room, 80.0, Duration::from_mins(10)).await?;
    }
    for light in selected_lights {
        expect_individual_fade(&mut requests, light, 80.0, Duration::from_mins(10)).await?;
    }

    // wait for fade to end
    expect_quiet_sleep_and_advance(&mut test, &mut requests, Duration::from_mins(10)).await?;

    let delay = test.clock.expect_sleep(Duration::from_mins(5)).await?;
    requests.expect_quiet().await?;
    test.hue_client
        .expect_behavior_enabled(instance, true)
        .await?;
    test.clock.advance(Duration::from_mins(5));
    delay.release();

    for room in [&bedroom, &guest_room] {
        requests.expect_set(room, json!({"state": "OFF"})).await?;
    }
    for light in selected_lights {
        requests.expect_set(light, json!({"state": "OFF"})).await?;
    }

    test.hue_client
        .expect_behavior_enabled(instance, false)
        .await?;
    requests.expect_quiet().await?;
    Ok(())
}

#[tokio::test]
async fn recurring_wakeup_stays_enabled_and_runs_on_next_selected_weekday() -> TestResult<()> {
    let WakeupFixture { mut test, room, .. } = wakeup_fixture(wednesday_at_six()).await?;
    // January 1, 2020 is Wednesday. Thursday must be skipped.
    let mut config = one_time_basic_wakeup_configuration(
        80.0,
        configuration::Duration { seconds: 600 },
        configuration::Time { hour: 7, minute: 0 },
        vec![configuration::Where {
            group: room.link,
            items: None,
        }],
    );
    config.when.recurrence_days = Some(vec![Weekday::Wed, Weekday::Fri]);
    let body = wakeup_body(&config);
    let mut requests = test.z2m.subscribe_requests();
    let instance = test
        .hue_client
        .post("/clip/v2/resource/behavior_instance", &body)
        .await?;

    expect_quiet_sleep_and_advance(&mut test, &mut requests, Duration::from_mins(50)).await?;
    expect_grouped_fade(&mut requests, &room, 80.0, Duration::from_mins(10)).await?;
    expect_quiet_sleep_and_advance(&mut test, &mut requests, Duration::from_mins(10)).await?;

    let until_friday = Duration::from_mins(2 * 24 * 60 - 10);
    let next = test.clock.expect_sleep(until_friday).await?;
    test.hue_client
        .expect_behavior_enabled(instance, true)
        .await?;
    requests.expect_quiet().await?;
    test.clock.advance(until_friday);
    next.release();

    expect_grouped_fade(&mut requests, &room, 80.0, Duration::from_mins(10)).await?;
    expect_quiet_sleep_and_advance(&mut test, &mut requests, Duration::from_mins(10)).await?;
    let _next = test
        .clock
        .expect_sleep(Duration::from_mins(5 * 24 * 60 - 10))
        .await?;
    test.hue_client
        .expect_behavior_enabled(instance, true)
        .await?;
    requests.expect_quiet().await?;
    Ok(())
}

#[tokio::test]
async fn sunrise_wakeup_uses_individual_effect_and_basic_fallback_in_mixed_room() -> TestResult<()>
{
    let WakeupFixture {
        mut test,
        room,
        basic,
        hue,
    } = wakeup_fixture(wednesday_at_six()).await?;
    let mut config = one_time_basic_wakeup_configuration(
        80.0,
        configuration::Duration { seconds: 600 },
        configuration::Time { hour: 7, minute: 0 },
        vec![configuration::Where {
            group: room.link,
            items: None,
        }],
    );
    config.style = Some(WakeupStyle::Sunrise);
    config.turn_lights_off_after = Some(configuration::Duration { seconds: 300 });
    let body = wakeup_body(&config);
    let mut requests = test.z2m.subscribe_requests();
    let instance = test
        .hue_client
        .post("/clip/v2/resource/behavior_instance", &body)
        .await?;
    expect_quiet_sleep_and_advance(&mut test, &mut requests, Duration::from_mins(50)).await?;

    // Members may be traversed in either order. Both must start before the fade wait.
    let mut expected: Vec<_> = individual_fade_payloads(80.0, Duration::from_mins(10))
        .into_iter()
        .map(|payload| (format!("{}/set", basic.topic()), payload))
        .collect();
    expected.push((
        format!("{}/set", hue.topic()),
        json!({
            "command": {
                "cluster": 0xFC03, "command": 0,
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
    ));
    expected.push((format!("{}/get", hue.topic()), json!({"state": ""})));
    requests.expect_requests_unordered(expected).await?;
    let finish = test.clock.expect_sleep(Duration::from_mins(10)).await?;
    test.hue_client
        .expect_behavior_enabled(instance, true)
        .await?;
    requests.expect_quiet().await?;
    test.clock.advance(Duration::from_mins(10));
    finish.release();
    expect_quiet_sleep_and_advance(&mut test, &mut requests, Duration::from_mins(5)).await?;
    // Already-on members were skipped and must not be switched off either.
    requests
        .expect_requests_unordered([
            (format!("{}/set", basic.topic()), json!({"state": "OFF"})),
            (
                format!("{}/set", hue.topic()),
                json!({"command": {
                    "cluster": 0xFC03, "command": 0,
                    "payload": {"data": [0x11, 0x00, 0x00, 0x01, 0x00]}
                }}),
            ),
            (format!("{}/get", hue.topic()), json!({"state": ""})),
        ])
        .await?;
    test.hue_client
        .expect_behavior_enabled(instance, false)
        .await?;
    requests.expect_quiet().await?;
    Ok(())
}
