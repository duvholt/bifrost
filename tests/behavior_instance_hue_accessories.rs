use std::collections::BTreeMap;

use chrono::{Local, TimeZone};
use hue::api::{BehaviorScript, Button, ButtonEvent, RType, Resource, ResourceLink};
use serde_json::{Map, Value, json};

use crate::common::fixture::{self, FixtureDevice, FixtureGroup, Z2mFixture, Z2mGroupFixture};
use crate::common::{TestBridge, TestResult, TestRoom, Z2mRequests};

pub mod common;

const DIMMER: FixtureDevice = FixtureDevice {
    id: 1,
    friendly_name: "dimmer",
};
const LAMP: FixtureDevice = FixtureDevice {
    id: 2,
    friendly_name: "lamp",
};
const OTHER_LAMP: FixtureDevice = FixtureDevice {
    id: 3,
    friendly_name: "other_lamp",
};
const ROOM: FixtureGroup = FixtureGroup {
    id: 1,
    friendly_name: "living_room",
};
const OTHER_ROOM: FixtureGroup = FixtureGroup {
    id: 2,
    friendly_name: "bedroom",
};

struct AccessoriesFixture {
    bridge: TestBridge,
    requests: Z2mRequests,
    device: ResourceLink,
    buttons: BTreeMap<u32, ResourceLink>,
    room: TestRoom<'static>,
    scenes: [ResourceLink; 2],
}

impl AccessoriesFixture {
    async fn new() -> TestResult<Self> {
        let lamp = fixture::ikea::tradfri_warm_white(&LAMP);
        let other_lamp = fixture::ikea::tradfri_warm_white(&OTHER_LAMP);
        let fixture = Z2mFixture::new()
            .with_group(
                Z2mGroupFixture::new(&ROOM)
                    .with_member(&lamp)
                    .with_scene(1, "Bright")
                    .with_scene(2, "Relax"),
            )
            .with_group(Z2mGroupFixture::new(&OTHER_ROOM).with_member(&other_lamp))
            .with_device(fixture::hue::dimmer_switch(&DIMMER))
            .with_device(lamp)
            .with_device(other_lamp);
        let mut bridge = TestBridge::start(fixture.into_state()).await?;
        let device = bridge.wait_for_device(DIMMER.clone()).await?;
        let Resource::Device(dimmer) = &device.obj else {
            panic!("expected a dimmer device");
        };
        let mut buttons = BTreeMap::new();
        for link in dimmer.button_services() {
            let button = bridge.hue_client.get_resource::<Button>(*link).await?;
            buttons.insert(button.metadata.control_id, *link);
        }
        assert_eq!(buttons.len(), 4);
        let room = bridge.wait_for_room(&ROOM).await?;
        bridge.wait_for_room(&OTHER_ROOM).await?;
        let scenes = [
            bridge.wait_for_scene(room.link, "Bright").await?.link,
            bridge.wait_for_scene(room.link, "Relax").await?.link,
        ];
        let requests = bridge.z2m.subscribe_requests();
        Ok(Self {
            bridge,
            requests,
            device: device.link(),
            buttons,
            room,
            scenes,
        })
    }

    async fn create_configuration(
        &mut self,
        enabled: bool,
        buttons: impl IntoIterator<Item = (u32, Value)>,
    ) -> TestResult<ResourceLink> {
        let buttons: Map<_, _> = buttons
            .into_iter()
            .map(|(control_id, mut configuration)| {
                configuration["where"] = json!([{"group": self.room.link}]);
                (self.buttons[&control_id].rid.to_string(), configuration)
            })
            .collect();
        let mut events = self.bridge.hue_client.subscribe_events();
        let instance = self
            .bridge
            .hue_client
            .post(
                "/clip/v2/resource/behavior_instance",
                &json!({
                    "script_id": BehaviorScript::HUE_ACCESSORIES_ID,
                    "enabled": enabled,
                    "metadata": {"name": "Hue Accessories integration test"},
                    "configuration": {
                        "device": self.device,
                        "model_id": "RWL021",
                        "buttons": buttons,
                    },
                }),
            )
            .await?;
        assert_eq!(
            events.expect_add(RType::BehaviorInstance).await?.link(),
            instance
        );
        self.requests.expect_quiet().await?;
        Ok(instance)
    }

    async fn set_enabled(&mut self, instance: ResourceLink, enabled: bool) -> TestResult<()> {
        let mut events = self.bridge.hue_client.subscribe_events();
        self.bridge
            .hue_client
            .put(
                &format!("/clip/v2/resource/behavior_instance/{}", instance.rid),
                &json!({"enabled": enabled}),
            )
            .await?;
        let update = events.expect_update_resource(instance).await?;
        assert_eq!(update.data["enabled"], enabled);
        self.requests.expect_quiet().await
    }

    async fn publish(&self, control_id: u32, action: &str, event: ButtonEvent) -> TestResult<()> {
        let mut events = self.bridge.hue_client.subscribe_events();
        self.bridge
            .z2m
            .publish_topic(format!("{}/action", DIMMER.topic()), json!(action))?;
        let update = events
            .expect_update_resource(self.buttons[&control_id])
            .await?;
        assert_eq!(
            update.data["button"]["button_report"]["event"],
            json!(event)
        );
        Ok(())
    }

    async fn expect_action(
        &mut self,
        control_id: u32,
        action: &str,
        event: ButtonEvent,
        payload: Value,
    ) -> TestResult<()> {
        self.publish(control_id, action, event).await?;
        self.requests.expect_set(&self.room, payload).await?;
        self.requests.expect_quiet().await
    }

    async fn expect_ignored(
        &mut self,
        control_id: u32,
        action: &str,
        event: ButtonEvent,
    ) -> TestResult<()> {
        self.publish(control_id, action, event).await?;
        self.requests.expect_quiet().await
    }
}

#[tokio::test]
async fn on_off_targets_configured_room_and_ignores_unconfigured_buttons() -> TestResult<()> {
    let mut test = AccessoriesFixture::new().await?;
    test.create_configuration(
        true,
        [
            (1, json!({"on_short_release": {"action": "last_on"}})),
            (4, json!({"on_short_release": {"action": "all_off"}})),
        ],
    )
    .await?;

    test.expect_ignored(1, "on_press", ButtonEvent::InitialPress)
        .await?;
    test.expect_action(
        1,
        "on_press_release",
        ButtonEvent::ShortRelease,
        json!({"state": "ON"}),
    )
    .await?;
    test.expect_action(
        4,
        "off_press_release",
        ButtonEvent::ShortRelease,
        json!({"state": "OFF"}),
    )
    .await?;
    test.expect_ignored(2, "up_press_release", ButtonEvent::ShortRelease)
        .await
}

#[tokio::test]
async fn explicit_release_and_long_press_actions_take_precedence_over_repeat() -> TestResult<()> {
    let mut test = AccessoriesFixture::new().await?;
    test.create_configuration(
        true,
        [(
            1,
            json!({
                "on_short_release": {"action": "last_on"},
                "on_long_press": {"action": "all_off"},
                "on_repeat": {"action": "dim_up"},
            }),
        )],
    )
    .await?;

    test.expect_action(
        1,
        "on_press_release",
        ButtonEvent::ShortRelease,
        json!({"state": "ON"}),
    )
    .await?;
    test.expect_ignored(1, "on_press", ButtonEvent::InitialPress)
        .await?;
    test.expect_action(
        1,
        "on_hold",
        ButtonEvent::LongPress,
        json!({"state": "OFF"}),
    )
    .await?;
    test.expect_action(
        1,
        "on_hold",
        ButtonEvent::Repeat,
        json!({
            "brightness_step": 50.8,
            "transition": 1.0,
        }),
    )
    .await?;
    test.expect_ignored(1, "on_hold_release", ButtonEvent::LongRelease)
        .await
}

#[tokio::test]
async fn dimming_uses_repeat_action_for_short_release_long_press_and_repeat() -> TestResult<()> {
    let mut test = AccessoriesFixture::new().await?;
    test.create_configuration(
        true,
        [
            (2, json!({"on_repeat": {"action": "dim_up"}})),
            (3, json!({"on_repeat": {"action": "dim_down"}})),
        ],
    )
    .await?;

    for (control_id, name, step) in [(2, "up", 50.8), (3, "down", -50.8)] {
        let payload = json!({"brightness_step": step, "transition": 1.0});
        test.expect_action(
            control_id,
            &format!("{name}_press_release"),
            ButtonEvent::ShortRelease,
            payload.clone(),
        )
        .await?;
        test.expect_ignored(
            control_id,
            &format!("{name}_press"),
            ButtonEvent::InitialPress,
        )
        .await?;
        test.expect_action(
            control_id,
            &format!("{name}_hold"),
            ButtonEvent::LongPress,
            payload.clone(),
        )
        .await?;
        test.expect_action(
            control_id,
            &format!("{name}_hold"),
            ButtonEvent::Repeat,
            payload,
        )
        .await?;
        test.expect_ignored(
            control_id,
            &format!("{name}_hold_release"),
            ButtonEvent::LongRelease,
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn scene_cycle_wraps_and_tracks_each_button_independently() -> TestResult<()> {
    let mut test = AccessoriesFixture::new().await?;
    let cycle = json!({"on_short_release": {"scene_cycle_extended": {
        "slots": [
            [{"action": {"recall": test.scenes[0]}}],
            [{"action": {"recall": test.scenes[1]}}],
        ],
        "with_off": {"enabled": false},
    }}});
    test.create_configuration(true, [(1, cycle.clone()), (4, cycle)])
        .await?;

    for (control_id, action, scene_id) in [
        (1, "on_press_release", 1),
        (4, "off_press_release", 1),
        (1, "on_press_release", 2),
        (1, "on_press_release", 1),
        (4, "off_press_release", 2),
    ] {
        test.expect_action(
            control_id,
            action,
            ButtonEvent::ShortRelease,
            json!({"scene_recall": scene_id}),
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn time_based_recall_uses_current_time_at_slot_boundary() -> TestResult<()> {
    let mut test = AccessoriesFixture::new().await?;
    let action = json!({"on_short_release": {"time_based_extended": {
        "slots": [
            {
                "start_time": {"hour": 7, "minute": 0},
                "actions": [{"action": {"recall": test.scenes[0]}}],
            },
            {
                "start_time": {"hour": 10, "minute": 0},
                "actions": [{"action": {"recall": test.scenes[1]}}],
            },
        ],
        "with_off": {"enabled": false},
    }}});
    test.create_configuration(true, [(1, action)]).await?;

    for (hour, minute, scene_id) in [(9, 59, 1), (10, 0, 2)] {
        test.bridge.clock.set_now(
            Local
                .with_ymd_and_hms(2020, 1, 1, hour, minute, 0)
                .single()
                .unwrap(),
        );
        test.expect_action(
            1,
            "on_press_release",
            ButtonEvent::ShortRelease,
            json!({"scene_recall": scene_id}),
        )
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn disabled_instances_ignore_buttons_and_can_be_reenabled() -> TestResult<()> {
    let mut test = AccessoriesFixture::new().await?;
    let instance = test
        .create_configuration(
            false,
            [(1, json!({"on_short_release": {"action": "last_on"}}))],
        )
        .await?;

    test.expect_ignored(1, "on_press_release", ButtonEvent::ShortRelease)
        .await?;
    test.set_enabled(instance, true).await?;
    test.expect_action(
        1,
        "on_press_release",
        ButtonEvent::ShortRelease,
        json!({"state": "ON"}),
    )
    .await?;

    test.set_enabled(instance, false).await?;
    test.expect_ignored(1, "on_press_release", ButtonEvent::ShortRelease)
        .await?;
    test.expect_ignored(1, "on_press_release", ButtonEvent::ShortRelease)
        .await?;
    test.set_enabled(instance, true).await?;
    test.expect_action(
        1,
        "on_press_release",
        ButtonEvent::ShortRelease,
        json!({"state": "ON"}),
    )
    .await
}
