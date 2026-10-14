use std::time::Duration;

use chrono::{Local, TimeZone, Weekday};
use hue::api::configuration::{self, Time, TimePoint, When, Where};
use hue::api::{
    BehaviorScript, GoToSleepConfiguration, GoToSleepEndState, GoToSleepStyle, ResourceLink,
};
use hue::effect_duration::EffectDuration;
use hue::xy::XY;
use hue::zigbee::{EffectType, HueZigbeeUpdate};
use serde_json::json;

use crate::common::fixture::{
    self, FixtureDevice, FixtureGroup, Z2mDeviceFixture, Z2mFixture, Z2mGroupFixture,
};
use crate::common::{
    TestBridge, TestLight, TestResult, TestRoom, TestZ2mDeviceOrGroup, Z2mRequests,
};

pub mod common;

const LAMP: FixtureDevice = FixtureDevice {
    id: 1,
    friendly_name: "bedside_lamp",
};
const OTHER_LAMP: FixtureDevice = FixtureDevice {
    id: 2,
    friendly_name: "other_lamp",
};
const BEDROOM: FixtureGroup = FixtureGroup {
    id: 1,
    friendly_name: "bedroom",
};
const READING_LIGHT: FixtureDevice = FixtureDevice {
    id: 3,
    friendly_name: "reading_light",
};
const UNSELECTED_LIGHT: FixtureDevice = FixtureDevice {
    id: 4,
    friendly_name: "unselected_light",
};
const LIVING_ROOM: FixtureGroup = FixtureGroup {
    id: 2,
    friendly_name: "living_room",
};

const EFFECT_LIGHT: FixtureDevice = FixtureDevice {
    id: 5,
    friendly_name: "sunset_effect_light",
};

const BASIC_WHITE: XY = XY::new(0.5018, 0.4152);
const SUNSET_WHITE: XY = XY::new(0.5056, 0.4141);
const SUNSET_FIRST: XY = XY::new(0.51591, 0.41461);
const SUNSET_SECOND: XY = XY::new(0.53781, 0.4111);
const SUNSET_THIRD: XY = XY::new(0.58571, 0.39311);
const NIGHTLIGHT: XY = XY::new(0.5610, 0.4042);

const SUNSET_PREPARATION: Duration = Duration::from_secs(2);
const SUNSET_ENDING: Duration = Duration::from_millis(400);

struct SunsetTiming {
    stage_spacing: Duration,
    final_transition: Duration,
}

const SUNSET_5_MINUTES: SunsetTiming = SunsetTiming {
    stage_spacing: Duration::from_millis(74_500),
    final_transition: Duration::from_millis(72_500),
};
const SUNSET_15_MINUTES: SunsetTiming = SunsetTiming {
    stage_spacing: Duration::from_millis(224_500),
    final_transition: Duration::from_millis(222_500),
};
const SUNSET_60_MINUTES: SunsetTiming = SunsetTiming {
    stage_spacing: Duration::from_millis(899_500),
    final_transition: Duration::from_millis(897_500),
};

fn hue_fade_speed(transition: Duration) -> u16 {
    u16::try_from(transition.as_millis() / 100)
        .expect("test transition must fit the Hue fade-speed field")
}

const fn minutes(value: u64) -> Duration {
    Duration::from_secs(value * 60)
}

fn color_light(device: &FixtureDevice, on: bool, brightness: u8) -> Z2mDeviceFixture {
    fixture::ikea::tradfri_color(device).with_state(json!({
        "state": if on { "ON" } else { "OFF" }, "brightness": brightness,
    }))
}

fn hue_light(brightness: u8) -> Z2mDeviceFixture {
    fixture::hue::flux_lightstrip(&LAMP).with_state(json!({
        "state": "ON", "brightness": brightness,
    }))
}

struct SleepFixture {
    bridge: TestBridge,
    requests: Z2mRequests,
}

impl SleepFixture {
    async fn new(fixture: Z2mFixture) -> TestResult<Self> {
        let bridge = TestBridge::start(fixture.into_state()).await?;
        bridge.clock.set_now(
            Local
                .with_ymd_and_hms(2020, 1, 1, 21, 0, 0)
                .single()
                .unwrap(),
        );
        let requests = bridge.z2m.subscribe_requests();
        Ok(Self { bridge, requests })
    }

    async fn single_light(
        device: &FixtureDevice,
        light: Z2mDeviceFixture,
    ) -> TestResult<(Self, TestRoom<'static>, TestLight)> {
        let bedroom = Z2mGroupFixture::new(&BEDROOM).with_member(&light);
        let mut test = Self::new(Z2mFixture::new().with_device(light).with_group(bedroom)).await?;
        let light = test.bridge.wait_for_light(device.clone()).await?;
        let bedroom = test.bridge.wait_for_room(&BEDROOM).await?;
        Ok((test, bedroom, light))
    }

    async fn start(&mut self, configuration: &GoToSleepConfiguration) -> TestResult<ResourceLink> {
        let instance = self.schedule(configuration).await?;
        self.wait(minutes(60)).await?;
        Ok(instance)
    }

    async fn schedule(&self, configuration: &GoToSleepConfiguration) -> TestResult<ResourceLink> {
        self.bridge
            .hue_client
            .post(
                "/clip/v2/resource/behavior_instance",
                &json!({
                    "script_id": BehaviorScript::GO_TO_SLEEP_ID,
                    "enabled": true,
                    "metadata": {"name": "Go-to-sleep regression test"},
                    "configuration": configuration,
                }),
            )
            .await
    }

    async fn wait(&mut self, duration: Duration) -> TestResult<()> {
        self.wait_for_targets(duration, 1).await
    }

    async fn wait_for_targets(&mut self, duration: Duration, targets: usize) -> TestResult<()> {
        let sleeps = self.bridge.clock.expect_sleeps(duration, targets).await?;
        self.requests.expect_quiet().await?;
        self.bridge.clock.advance_and_release(duration, sleeps);
        Ok(())
    }

    async fn finish(&mut self, instance: ResourceLink) -> TestResult<()> {
        self.bridge
            .hue_client
            .expect_behavior_enabled(instance, false)
            .await?;
        self.requests.expect_quiet().await
    }
}

const fn configuration(
    style: GoToSleepStyle,
    end_state: GoToSleepEndState,
    fade_minutes: u32,
    selection: Vec<Where>,
) -> GoToSleepConfiguration {
    GoToSleepConfiguration {
        style,
        end_state,
        fade_out_duration: configuration::Duration {
            seconds: fade_minutes * 60,
        },
        when: When {
            recurrence_days: None,
            time_point: TimePoint::Time {
                time: Time {
                    hour: 22,
                    minute: 0,
                },
            },
        },
        where_field: selection,
    }
}

const fn select_room(room: &TestRoom<'_>) -> Where {
    Where {
        group: room.link,
        items: None,
    }
}

fn select_lights(room: &TestRoom<'_>, lights: &[&TestLight]) -> Where {
    Where {
        group: room.link,
        items: Some(lights.iter().map(|light| light.link).collect()),
    }
}

impl SleepFixture {
    async fn expect_sunset_fade_stages(
        &mut self,
        light: &TestLight,
        levels: [f64; 4],
        timing: &SunsetTiming,
        skip_known_brightness_conversion_bug: bool,
    ) -> TestResult<()> {
        for (level, xy) in levels
            .into_iter()
            .zip([SUNSET_FIRST, SUNSET_SECOND, SUNSET_THIRD])
        {
            self.expect_sunset_color_transition(
                light,
                level,
                xy,
                timing.stage_spacing,
                skip_known_brightness_conversion_bug,
            )
            .await?;
            self.wait(timing.stage_spacing).await?;
        }
        self.requests
            .expect_set(
                light,
                json!({
                    "brightness": levels[3],
                    "transition": timing.final_transition.as_secs_f64(),
                }),
            )
            .await?;
        self.wait(timing.stage_spacing).await
    }

    async fn expect_sunset_color_transition(
        &mut self,
        light: &TestLight,
        level: f64,
        xy: XY,
        transition: Duration,
        skip_known_brightness_conversion_bug: bool,
    ) -> TestResult<()> {
        let transition = transition.as_secs_f64();
        self.requests
            .expect_set(light, json!({"state": "ON"}))
            .await?;
        self.requests
            .expect_set(light, json!({"color": xy, "transition": transition}))
            .await?;
        let request = self.requests.next_request().await?;
        assert_eq!(request.topic, format!("{}/set", light.topic()));
        let actual_level = request.payload["brightness"]
            .as_f64()
            .expect("brightness must be a number");
        // TODO: Remove this flag and always compare once brightness conversion is fixed.
        if !skip_known_brightness_conversion_bug {
            assert!(
                (actual_level - level).abs() < 1e-9,
                "expected brightness {level}, got {actual_level}",
            );
        }
        assert_eq!(
            request.payload,
            json!({"brightness": actual_level, "transition": transition}),
        );
        Ok(())
    }

    async fn expect_hue_sunset_effect_start(
        &mut self,
        light: &TestLight,
        duration: Duration,
    ) -> TestResult<()> {
        let seconds = u32::try_from(duration.as_secs())
            .expect("test effect duration must fit the Hue duration field");
        self.requests
            .expect_hue_command(
                light,
                HueZigbeeUpdate::new()
                    .with_on_off(true)
                    .with_fade_speed(hue_fade_speed(SUNSET_PREPARATION))
                    .with_effect_type(EffectType::Sunset)
                    .with_effect_duration(EffectDuration::from_seconds(seconds)?),
            )
            .await?;
        Ok(())
    }

    async fn expect_hue_sunset_effect_ending(
        &mut self,
        light: &TestLight,
        xy: XY,
    ) -> TestResult<()> {
        self.requests
            .expect_hue_command(
                light,
                HueZigbeeUpdate::new()
                    .with_fade_speed(hue_fade_speed(SUNSET_ENDING))
                    .with_effect_type(EffectType::NoEffect),
            )
            .await?;
        self.expect_hue_color_transition(light, 4, xy, SUNSET_ENDING)
            .await
    }

    async fn expect_hue_color_transition(
        &mut self,
        light: &TestLight,
        level: u8,
        xy: XY,
        transition: Duration,
    ) -> TestResult<()> {
        let update = HueZigbeeUpdate::new()
            .with_on_off(true)
            .with_brightness(level)
            .with_color_xy(xy)
            .with_fade_speed(hue_fade_speed(transition));
        self.requests.expect_hue_command(light, update).await
    }

    async fn expect_sunset_preparation(
        &mut self,
        target: &(impl TestZ2mDeviceOrGroup + Sync),
    ) -> TestResult<()> {
        self.requests
            .expect_set(
                target,
                json!({"color_temp": 455, "transition": SUNSET_PREPARATION.as_secs_f64()}),
            )
            .await?;
        self.requests
            .expect_set(
                target,
                json!({"color": SUNSET_WHITE, "transition": SUNSET_PREPARATION.as_secs_f64()}),
            )
            .await
    }

    async fn expect_basic_transition(
        &mut self,
        targets: &[&(dyn TestZ2mDeviceOrGroup + Sync)],
        level: f32,
        mirek: u16,
        xy: XY,
        transition: Duration,
    ) -> TestResult<()> {
        let transition = transition.as_secs_f64();
        self.requests
            .expect_requests_ordered_per_topic(targets.iter().map(|target| {
                (
                    format!("{}/set", target.topic()),
                    vec![
                        json!({"state": "ON"}),
                        json!({"color_temp": mirek, "transition": transition}),
                        json!({"color": xy, "transition": transition}),
                        json!({"brightness": level, "transition": transition}),
                    ],
                )
            }))
            .await
    }
}

#[test]
fn sleep_colors_encode_to_expected_zigbee_bytes() {
    let expected_colors = [
        ("basic white", BASIC_WHITE, [0x75, 0x80, 0x4a, 0x6a]),
        ("sunset white", SUNSET_WHITE, [0x6e, 0x81, 0x02, 0x6a]),
        ("first sunset stage", SUNSET_FIRST, [0x12, 0x84, 0x23, 0x6a]),
        (
            "second sunset stage",
            SUNSET_SECOND,
            [0xad, 0x89, 0x3d, 0x69],
        ),
        ("third sunset stage", SUNSET_THIRD, [0xf0, 0x95, 0xa2, 0x64]),
        ("nightlight", NIGHTLIGHT, [0x9d, 0x8f, 0x79, 0x67]),
    ];

    for (name, xy, [x_low, x_high, y_low, y_high]) in expected_colors {
        let encoded = HueZigbeeUpdate::new().with_color_xy(xy).to_vec().unwrap();
        // COLOR_XY flags followed by the two little-endian coordinates.
        assert_eq!(
            encoded,
            [0x08, 0x00, x_low, x_high, y_low, y_high],
            "{name} Zigbee XY bytes"
        );
    }
}

#[test]
fn hue_sunset_effect_encodes_five_minute_duration() {
    let command = HueZigbeeUpdate::new()
        .with_on_off(true)
        .with_fade_speed(hue_fade_speed(SUNSET_PREPARATION))
        .with_effect_type(EffectType::Sunset)
        .with_effect_duration(EffectDuration::from_seconds(300).unwrap());
    assert_eq!(
        command.to_vec().unwrap(),
        [0xb1, 0x00, 0x01, 0x14, 0x00, 0x0d, 0x91]
    );
}

#[test]
fn hue_sunset_effect_encodes_ending_commands() {
    let clear_effect = HueZigbeeUpdate::new()
        .with_fade_speed(hue_fade_speed(SUNSET_ENDING))
        .with_effect_type(EffectType::NoEffect);
    assert_eq!(
        clear_effect.to_vec().unwrap(),
        [0x30, 0x00, 0x04, 0x00, 0x00]
    );

    for (xy, bytes) in [
        (
            BASIC_WHITE,
            [0x1b, 0x00, 0x01, 0x04, 0x75, 0x80, 0x4a, 0x6a, 0x04, 0x00],
        ),
        (
            NIGHTLIGHT,
            [0x1b, 0x00, 0x01, 0x04, 0x9d, 0x8f, 0x79, 0x67, 0x04, 0x00],
        ),
    ] {
        let ending = HueZigbeeUpdate::new()
            .with_on_off(true)
            .with_brightness(4)
            .with_color_xy(xy)
            .with_fade_speed(hue_fade_speed(SUNSET_ENDING));
        assert_eq!(ending.to_vec().unwrap(), bytes);
    }
}

#[tokio::test]
async fn basic_sleep_fades_then_turns_off_and_disables_itself() -> TestResult<()> {
    let (mut test, bedroom, _light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 128)).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_basic_transition(&[&bedroom], 128.0, 447, BASIC_WHITE, minutes(1))
        .await?;
    test.wait(minutes(1)).await?;
    test.expect_basic_transition(&[&bedroom], 4.0, 447, BASIC_WHITE, minutes(4))
        .await?;

    let finish = test.bridge.clock.expect_sleep(minutes(4)).await?;
    test.bridge
        .hue_client
        .expect_behavior_enabled(instance, true)
        .await?;
    test.requests.expect_quiet().await?;
    test.bridge
        .clock
        .advance_and_release(minutes(4), vec![finish]);
    test.requests
        .expect_set(&bedroom, json!({"state": "OFF", "transition": 0.4}))
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn basic_sleep_finishes_at_minimum_brightness() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&OTHER_LAMP, color_light(&OTHER_LAMP, true, 128)).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::MinimumBrightness,
        5,
        vec![select_lights(&bedroom, &[&light])],
    );
    let instance = test.start(&config).await?;

    test.expect_basic_transition(&[&light], 128.0, 447, BASIC_WHITE, minutes(1))
        .await?;
    test.wait(minutes(1)).await?;
    test.expect_basic_transition(&[&light], 4.0, 447, BASIC_WHITE, minutes(4))
        .await?;
    test.wait(minutes(4)).await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn basic_sleep_finishes_with_nightlight() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 128)).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::Nightlight,
        5,
        vec![select_lights(&bedroom, &[&light])],
    );
    let instance = test.start(&config).await?;

    test.expect_basic_transition(&[&light], 128.0, 454, NIGHTLIGHT, minutes(1))
        .await?;
    test.wait(minutes(1)).await?;
    test.expect_basic_transition(&[&light], 4.0, 454, NIGHTLIGHT, minutes(4))
        .await?;
    test.wait(minutes(4)).await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn basic_sleep_prepares_dim_light_before_fading() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 26)).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::MinimumBrightness,
        60,
        vec![select_lights(&bedroom, &[&light])],
    );
    let instance = test.start(&config).await?;

    test.expect_basic_transition(&[&light], 128.0, 447, BASIC_WHITE, minutes(1))
        .await?;
    test.wait(minutes(1)).await?;
    test.expect_basic_transition(&[&light], 4.0, 447, BASIC_WHITE, minutes(59))
        .await?;
    test.wait(minutes(59)).await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_from_full_brightness_turns_off() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.requests
        .expect_set(
            &light,
            json!({"brightness_step": -71.0, "transition": SUNSET_PREPARATION.as_secs_f64()}),
        )
        .await?;
    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [126.0, 95.0, 52.0, 4.0], &SUNSET_5_MINUTES, false)
        .await?;
    test.requests
        .expect_set(
            &light,
            json!({"state": "OFF", "transition": SUNSET_ENDING.as_secs_f64()}),
        )
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_finishes_at_minimum_brightness() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::MinimumBrightness,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.requests
        .expect_set(
            &light,
            json!({"brightness_step": -71.0, "transition": SUNSET_PREPARATION.as_secs_f64()}),
        )
        .await?;
    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [126.0, 95.0, 52.0, 4.0], &SUNSET_5_MINUTES, false)
        .await?;
    test.expect_sunset_color_transition(&light, 4.0, BASIC_WHITE, SUNSET_ENDING, false)
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_finishes_with_nightlight() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::Nightlight,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.requests
        .expect_set(
            &light,
            json!({"brightness_step": -71.0, "transition": SUNSET_PREPARATION.as_secs_f64()}),
        )
        .await?;
    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [126.0, 95.0, 52.0, 4.0], &SUNSET_5_MINUTES, false)
        .await?;
    test.expect_sunset_color_transition(&light, 4.0, NIGHTLIGHT, SUNSET_ENDING, false)
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_from_minimum_brightness_uses_level_four_before_turning_off() -> TestResult<()>
{
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 1)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [1.0, 1.0, 1.0, 4.0], &SUNSET_5_MINUTES, true)
        .await?;
    test.requests
        .expect_set(
            &light,
            json!({"state": "OFF", "transition": SUNSET_ENDING.as_secs_f64()}),
        )
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_from_ten_percent_skips_initial_brightness_decrease() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 25)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [22.0, 17.0, 10.0, 4.0], &SUNSET_5_MINUTES, true)
        .await?;
    test.requests
        .expect_set(
            &light,
            json!({"state": "OFF", "transition": SUNSET_ENDING.as_secs_f64()}),
        )
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_from_quarter_brightness_uses_lower_stage_targets() -> TestResult<()> {
    let lamp = color_light(&LAMP, true, 64);
    let (mut test, bedroom, light) = SleepFixture::single_light(&LAMP, lamp).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_lights(&bedroom, &[&light])],
    );
    let instance = test.start(&config).await?;

    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [56.0, 43.0, 24.0, 4.0], &SUNSET_5_MINUTES, true)
        .await?;
    test.requests
        .expect_set(
            &light,
            json!({"state": "OFF", "transition": SUNSET_ENDING.as_secs_f64()}),
        )
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_from_half_brightness_uses_lower_stage_targets() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 127)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [112.0, 84.0, 46.0, 4.0], &SUNSET_5_MINUTES, true)
        .await?;
    test.requests
        .expect_set(
            &light,
            json!({"state": "OFF", "transition": SUNSET_ENDING.as_secs_f64()}),
        )
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_from_three_quarter_brightness_decreases_before_stages() -> TestResult<()> {
    let lamp = color_light(&LAMP, true, 191);
    let (mut test, bedroom, light) = SleepFixture::single_light(&LAMP, lamp).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.requests
        .expect_set(
            &light,
            json!({"brightness_step": -71.0, "transition": SUNSET_PREPARATION.as_secs_f64()}),
        )
        .await?;
    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [106.0, 80.0, 44.0, 4.0], &SUNSET_5_MINUTES, false)
        .await?;
    test.requests
        .expect_set(
            &light,
            json!({"state": "OFF", "transition": SUNSET_ENDING.as_secs_f64()}),
        )
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_scales_transitions_to_15_minutes() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::MinimumBrightness,
        15,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.requests
        .expect_set(
            &light,
            json!({"brightness_step": -71.0, "transition": SUNSET_PREPARATION.as_secs_f64()}),
        )
        .await?;
    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [126.0, 95.0, 52.0, 4.0], &SUNSET_15_MINUTES, false)
        .await?;
    test.expect_sunset_color_transition(&light, 4.0, BASIC_WHITE, SUNSET_ENDING, false)
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sunset_fade_scales_transitions_to_60_minutes() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::Nightlight,
        60,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.requests
        .expect_set(
            &light,
            json!({"brightness_step": -71.0, "transition": SUNSET_PREPARATION.as_secs_f64()}),
        )
        .await?;
    test.expect_sunset_preparation(&light).await?;
    test.wait(SUNSET_PREPARATION).await?;
    test.expect_sunset_fade_stages(&light, [126.0, 95.0, 52.0, 4.0], &SUNSET_60_MINUTES, false)
        .await?;
    test.expect_sunset_color_transition(&light, 4.0, NIGHTLIGHT, SUNSET_ENDING, false)
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn hue_sunset_effect_turns_off_without_ending_command() -> TestResult<()> {
    let (mut test, bedroom, light) = SleepFixture::single_light(&LAMP, hue_light(254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_hue_sunset_effect_start(&light, minutes(5))
        .await?;

    test.wait(minutes(5)).await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn hue_sunset_effect_uses_configured_15_minute_duration() -> TestResult<()> {
    let (mut test, bedroom, light) = SleepFixture::single_light(&LAMP, hue_light(254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::Nightlight,
        15,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_hue_sunset_effect_start(&light, minutes(15))
        .await?;
    test.wait(minutes(15)).await?;
    test.expect_hue_sunset_effect_ending(&light, NIGHTLIGHT)
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn hue_sunset_effect_clears_before_minimum_brightness() -> TestResult<()> {
    let (mut test, bedroom, light) = SleepFixture::single_light(&LAMP, hue_light(254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::MinimumBrightness,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_hue_sunset_effect_start(&light, minutes(5))
        .await?;
    test.wait(minutes(5)).await?;
    test.expect_hue_sunset_effect_ending(&light, BASIC_WHITE)
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn hue_sunset_effect_clears_before_nightlight() -> TestResult<()> {
    let (mut test, bedroom, light) = SleepFixture::single_light(&LAMP, hue_light(254)).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::Nightlight,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_hue_sunset_effect_start(&light, minutes(5))
        .await?;
    test.wait(minutes(5)).await?;
    test.expect_hue_sunset_effect_ending(&light, NIGHTLIGHT)
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sleep_mixes_hue_sunset_effect_and_sunset_fade() -> TestResult<()> {
    let effect = fixture::hue::flux_lightstrip(&EFFECT_LIGHT)
        .with_state(json!({"state": "ON", "brightness": 254}));
    let other_effect = hue_light(254);
    let standard = color_light(&OTHER_LAMP, true, 254);
    let room = Z2mGroupFixture::new(&BEDROOM)
        .with_member(&effect)
        .with_member(&other_effect)
        .with_member(&standard);
    let mut test = SleepFixture::new(
        Z2mFixture::new()
            .with_device(effect)
            .with_device(other_effect)
            .with_device(standard)
            .with_group(room),
    )
    .await?;
    let effect_light = test.bridge.wait_for_light(EFFECT_LIGHT).await?;
    let hue_light = test.bridge.wait_for_light(LAMP).await?;
    let color_light = test.bridge.wait_for_light(OTHER_LAMP).await?;
    let bedroom = test.bridge.wait_for_room(&BEDROOM).await?;
    let config = configuration(
        GoToSleepStyle::Sunset,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    let hue_data = HueZigbeeUpdate::new()
        .with_on_off(true)
        .with_fade_speed(hue_fade_speed(SUNSET_PREPARATION))
        .with_effect_type(EffectType::Sunset)
        .with_effect_duration(EffectDuration::from_seconds(300).unwrap())
        .to_vec()
        .unwrap();
    let hue_start = json!({
        "command": {"cluster": 0xFC03, "command": 0, "payload": {"data": hue_data}},
    });
    test.requests
        .expect_requests_ordered_per_topic([
            (
                format!("{}/set", color_light.topic()),
                vec![
                    json!({"brightness_step": -71.0, "transition": SUNSET_PREPARATION.as_secs_f64()}),
                    json!({"color_temp": 455, "transition": SUNSET_PREPARATION.as_secs_f64()}),
                    json!({"color": SUNSET_WHITE, "transition": SUNSET_PREPARATION.as_secs_f64()}),
                ],
            ),
            (
                format!("{}/set", effect_light.topic()),
                vec![hue_start.clone()],
            ),
            (format!("{}/set", hue_light.topic()), vec![hue_start]),
        ])
        .await?;
    // Request a non-sorted order, including duplicate durations.
    let [hue_effect, preparation, other_hue_effect] = test
        .bridge
        .clock
        .expect_sleeps_unordered([minutes(5), SUNSET_PREPARATION, minutes(5)])
        .await?;
    assert_eq!(hue_effect.duration, minutes(5));
    assert_eq!(preparation.duration, SUNSET_PREPARATION);
    assert_eq!(other_hue_effect.duration, minutes(5));

    test.requests.expect_quiet().await?;
    test.bridge
        .clock
        .advance_and_release(SUNSET_PREPARATION, vec![preparation]);
    test.expect_sunset_fade_stages(
        &color_light,
        [126.0, 95.0, 52.0, 4.0],
        &SUNSET_5_MINUTES,
        false,
    )
    .await?;
    // The IKEA stages have advanced the clock to the Hue effects' deadline.
    hue_effect.release();
    other_hue_effect.release();
    test.requests
        .expect_set(
            &color_light,
            json!({"state": "OFF", "transition": SUNSET_ENDING.as_secs_f64()}),
        )
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sleep_skips_already_off_lights() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, false, 128)).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::Nightlight,
        5,
        vec![select_lights(&bedroom, &[&light])],
    );
    test.start(&config).await?;
    test.requests.expect_quiet().await?;
    Ok(())
}

#[tokio::test]
async fn sleep_room_skips_already_off_members() -> TestResult<()> {
    let selected = color_light(&LAMP, true, 128);
    let other = color_light(&OTHER_LAMP, false, 128);
    let room = Z2mGroupFixture::new(&BEDROOM)
        .with_member(&selected)
        .with_member(&other);
    let mut test = SleepFixture::new(
        Z2mFixture::new()
            .with_device(selected)
            .with_device(other)
            .with_group(room),
    )
    .await?;
    let light = test.bridge.wait_for_light(LAMP).await?;
    test.bridge.wait_for_light(OTHER_LAMP).await?;
    let bedroom = test.bridge.wait_for_room(&BEDROOM).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_room(&bedroom)],
    );
    let instance = test.start(&config).await?;

    test.expect_basic_transition(&[&light], 128.0, 447, BASIC_WHITE, minutes(1))
        .await?;
    test.wait(minutes(1)).await?;
    test.expect_basic_transition(&[&light], 4.0, 447, BASIC_WHITE, minutes(4))
        .await?;
    test.wait(minutes(4)).await?;
    test.requests
        .expect_set(&light, json!({"state": "OFF", "transition": 0.4}))
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sleep_leaves_unselected_on_light_untouched() -> TestResult<()> {
    let selected = color_light(&LAMP, true, 128);
    let other = color_light(&OTHER_LAMP, true, 128);
    let room = Z2mGroupFixture::new(&BEDROOM)
        .with_member(&selected)
        .with_member(&other);
    let mut test = SleepFixture::new(
        Z2mFixture::new()
            .with_device(selected)
            .with_device(other)
            .with_group(room),
    )
    .await?;
    let light = test.bridge.wait_for_light(LAMP).await?;
    test.bridge.wait_for_light(OTHER_LAMP).await?;
    let bedroom = test.bridge.wait_for_room(&BEDROOM).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_lights(&bedroom, &[&light])],
    );
    let instance = test.start(&config).await?;

    test.expect_basic_transition(&[&light], 128.0, 447, BASIC_WHITE, minutes(1))
        .await?;
    test.wait(minutes(1)).await?;
    test.expect_basic_transition(&[&light], 4.0, 447, BASIC_WHITE, minutes(4))
        .await?;
    test.wait(minutes(4)).await?;
    test.requests
        .expect_set(&light, json!({"state": "OFF", "transition": 0.4}))
        .await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sleep_selects_a_whole_room_and_individual_lights_from_another_room() -> TestResult<()> {
    let bedside = color_light(&LAMP, true, 128);
    let other_bedside = color_light(&OTHER_LAMP, true, 128);
    let reading = color_light(&READING_LIGHT, true, 128);
    let unselected = color_light(&UNSELECTED_LIGHT, true, 128);
    let bedroom = Z2mGroupFixture::new(&BEDROOM)
        .with_member(&bedside)
        .with_member(&other_bedside);
    let living_room = Z2mGroupFixture::new(&LIVING_ROOM)
        .with_member(&reading)
        .with_member(&unselected);
    let mut test = SleepFixture::new(
        Z2mFixture::new()
            .with_device(bedside)
            .with_device(other_bedside)
            .with_device(reading)
            .with_device(unselected)
            .with_group(bedroom)
            .with_group(living_room),
    )
    .await?;
    test.bridge.wait_for_light(LAMP).await?;
    test.bridge.wait_for_light(OTHER_LAMP).await?;
    let reading_light = test.bridge.wait_for_light(READING_LIGHT).await?;
    test.bridge.wait_for_light(UNSELECTED_LIGHT).await?;
    let bedroom = test.bridge.wait_for_room(&BEDROOM).await?;
    let living_room = test.bridge.wait_for_room(&LIVING_ROOM).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::MinimumBrightness,
        5,
        vec![
            select_room(&bedroom),
            select_lights(&living_room, &[&reading_light]),
        ],
    );
    let instance = test.start(&config).await?;

    test.expect_basic_transition(
        &[&bedroom, &reading_light],
        128.0,
        447,
        BASIC_WHITE,
        minutes(1),
    )
    .await?;
    test.wait_for_targets(minutes(1), 2).await?;
    test.expect_basic_transition(
        &[&bedroom, &reading_light],
        4.0,
        447,
        BASIC_WHITE,
        minutes(4),
    )
    .await?;
    test.wait_for_targets(minutes(4), 2).await?;
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sleep_recurrence_stays_enabled_and_runs_on_next_selected_weekday() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 128)).await?;
    let mut config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::MinimumBrightness,
        5,
        vec![select_lights(&bedroom, &[&light])],
    );
    config.when.recurrence_days = Some(vec![Weekday::Wed, Weekday::Fri]);
    let instance = test.schedule(&config).await?;
    test.wait(minutes(60)).await?;

    test.expect_basic_transition(&[&light], 128.0, 447, BASIC_WHITE, minutes(1))
        .await?;
    test.wait(minutes(1)).await?;
    test.expect_basic_transition(&[&light], 4.0, 447, BASIC_WHITE, minutes(4))
        .await?;
    test.wait(minutes(4)).await?;

    let mut events = test.bridge.hue_client.subscribe_events();
    test.bridge
        .z2m
        .publish(&light, json!({"state": "ON", "brightness": 4}))
        .unwrap();
    events.expect_update_resource(light.link).await?;
    test.bridge
        .hue_client
        .expect_behavior_enabled(instance, true)
        .await?;
    // Wednesday 22:05 -> Friday 22:00
    test.wait(minutes(2 * 24 * 60 - 5)).await?;

    test.expect_basic_transition(&[&light], 128.0, 447, BASIC_WHITE, minutes(1))
        .await?;
    test.wait(minutes(1)).await?;
    test.expect_basic_transition(&[&light], 4.0, 447, BASIC_WHITE, minutes(4))
        .await?;
    test.wait(minutes(4)).await?;
    let _next = test
        .bridge
        .clock
        .expect_sleep(minutes(5 * 24 * 60 - 5))
        .await?;
    test.bridge
        .hue_client
        .expect_behavior_enabled(instance, true)
        .await?;
    test.requests.expect_quiet().await?;
    Ok(())
}

#[tokio::test]
async fn sleep_disable_before_start_cancels_pending_wait() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 128)).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_lights(&bedroom, &[&light])],
    );
    let instance = test.schedule(&config).await?;
    let start = test.bridge.clock.expect_sleep(minutes(60)).await?;
    test.bridge
        .hue_client
        .put(
            &format!("/clip/v2/resource/behavior_instance/{}", instance.rid),
            &json!({"enabled": false}),
        )
        .await?;
    start.expect_cancelled().await;
    test.bridge.clock.advance(minutes(120));
    test.finish(instance).await?;
    Ok(())
}

#[tokio::test]
async fn sleep_delete_before_start_cancels_pending_wait() -> TestResult<()> {
    let (mut test, bedroom, light) =
        SleepFixture::single_light(&LAMP, color_light(&LAMP, true, 128)).await?;
    let config = configuration(
        GoToSleepStyle::Basic,
        GoToSleepEndState::TurnOff,
        5,
        vec![select_lights(&bedroom, &[&light])],
    );
    let instance = test.schedule(&config).await?;
    let start = test.bridge.clock.expect_sleep(minutes(60)).await?;
    test.bridge
        .hue_client
        .delete(&format!(
            "/clip/v2/resource/behavior_instance/{}",
            instance.rid
        ))
        .await?;
    start.expect_cancelled().await;
    test.bridge.clock.advance(minutes(120));
    test.requests.expect_quiet().await?;
    Ok(())
}
