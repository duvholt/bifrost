use hue::api::{
    EntertainmentConfiguration, EntertainmentConfigurationStatus, Resource, ResourceLink,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::time::sleep;

use crate::common::fixture::{self, FixtureDevice, Z2mFixture};
use crate::common::{TestBridge, TestResult};

pub mod common;

const ACTIVE: EntertainmentConfigurationStatus = EntertainmentConfigurationStatus::Active;
const INACTIVE: EntertainmentConfigurationStatus = EntertainmentConfigurationStatus::Inactive;

const HUE_WHITE_AMBIANCE: FixtureDevice = FixtureDevice {
    id: 1,
    friendly_name: "hue_white_ambiance",
};

fn z2m_state() -> BTreeMap<String, Value> {
    Z2mFixture::new()
        .with_device(fixture::hue::white_ambience(&HUE_WHITE_AMBIANCE))
        .into_state()
}

/// Two entertainment areas on the same light, so that starting either one
/// opens a stream in the z2m backend
async fn start_with_two_areas() -> TestResult<(TestBridge, ResourceLink, ResourceLink)> {
    let mut bridge = TestBridge::start(z2m_state()).await?;
    let device = bridge.wait_for_device(HUE_WHITE_AMBIANCE).await?;
    let Resource::Device(device) = &device.obj else {
        panic!("expected a light device");
    };
    let service = *device
        .entertainment_service()
        .expect("light has an entertainment service");

    let mut areas = vec![];
    for name in ["first", "second"] {
        let area = bridge
            .hue_client
            .post(
                "/clip/v2/resource/entertainment_configuration",
                &json!({
                    "configuration_type": "music",
                    "metadata": { "name": name },
                    "locations": { "service_locations": [{
                        "service": service,
                        "positions": [{ "x": 0.0, "y": 0.0, "z": 0.0 }],
                    }]},
                }),
            )
            .await?;
        areas.push(area);
    }

    Ok((bridge, areas[0], areas[1]))
}

async fn send_action(bridge: &TestBridge, area: ResourceLink, action: &str) -> TestResult<()> {
    bridge
        .hue_client
        .put(
            &format!("/clip/v2/resource/entertainment_configuration/{}", area.rid),
            &json!({ "action": action }),
        )
        .await
}

async fn status(
    bridge: &TestBridge,
    area: ResourceLink,
) -> TestResult<EntertainmentConfigurationStatus> {
    Ok(bridge
        .hue_client
        .get_resource::<EntertainmentConfiguration>(area)
        .await?
        .status)
}

#[tokio::test]
async fn start_is_ignored_while_another_area_streams() -> TestResult<()> {
    let (bridge, first, second) = start_with_two_areas().await?;

    send_action(&bridge, first, "start").await?;
    send_action(&bridge, second, "start").await?;

    assert_eq!(status(&bridge, first).await?, ACTIVE);
    assert_eq!(status(&bridge, second).await?, INACTIVE);
    Ok(())
}

#[tokio::test]
async fn stop_is_ignored_for_an_area_that_is_not_streaming() -> TestResult<()> {
    let (bridge, first, second) = start_with_two_areas().await?;

    send_action(&bridge, first, "start").await?;
    send_action(&bridge, second, "stop").await?;

    // the z2m backend handles a stop in the background
    sleep(Duration::from_millis(200)).await;

    assert_eq!(status(&bridge, first).await?, ACTIVE);
    Ok(())
}

#[tokio::test]
async fn another_area_can_start_after_a_stop() -> TestResult<()> {
    let (bridge, first, second) = start_with_two_areas().await?;

    send_action(&bridge, first, "start").await?;
    send_action(&bridge, first, "stop").await?;
    send_action(&bridge, second, "start").await?;

    assert_eq!(status(&bridge, first).await?, INACTIVE);
    assert_eq!(status(&bridge, second).await?, ACTIVE);
    Ok(())
}
