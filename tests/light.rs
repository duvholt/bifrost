use hue::api::{
    ColorGamut, ColorTemperature, ColorTemperatureUpdate, ColorUpdate, ContentConfiguration,
    ContentConfigurationOrder, ContentConfigurationOrientation, ContentConfigurationStatusType,
    DeviceArchetype, Dimming, DimmingUpdate, GamutType, Identify, Light, LightAlert, LightColor,
    LightDynamics, LightDynamicsStatus, LightEffect, LightEffectParameters, LightEffectStatus,
    LightEffects, LightEffectsV2, LightFunction, LightGradient, LightGradientMode,
    LightGradientPoint, LightMetadata, LightMode, LightPowerup, LightPowerupColor,
    LightPowerupDimming, LightPowerupOn, LightPowerupPreset, LightProductData, LightSignal,
    LightSignaling, LightTimedEffect, LightTimedEffects, MirekSchema, On, OrderType,
    OrientationType, RType, ResourceLink, Stub,
};
use hue::xy::XY;
#[cfg(test)]
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::common::fixture::{self, FixtureDevice, Z2mFixture, Z2mGroupFixture};
use crate::common::{TestBridge, TestResult};

pub mod common;

const IKEA_WARM_WHITE: FixtureDevice = FixtureDevice {
    id: 1,
    friendly_name: "ikea_warm_white",
};

const IKEA_COLOR: FixtureDevice = FixtureDevice {
    id: 2,
    friendly_name: "ikea_color",
};

const HUE_WHITE_AMBIANCE: FixtureDevice = FixtureDevice {
    id: 3,
    friendly_name: "hue_white_ambiance",
};

const HUE_FLUX_LIGHTSTRIP: FixtureDevice = FixtureDevice {
    id: 4,
    friendly_name: "hue_flux_lightstrip",
};

const HUE_PLAY_GRADIENT_LIGHTSTRIP: FixtureDevice = FixtureDevice {
    id: 5,
    friendly_name: "hue_play_gradient_lightstrip",
};

const MISC_DIMMER_LIGHT: FixtureDevice = FixtureDevice {
    id: 6,
    friendly_name: "misc_dimmer_light",
};

fn expected_dimmable_light(
    owner: ResourceLink,
    fixture: &FixtureDevice,
    archetype: DeviceArchetype,
    on: bool,
    brightness: f64,
) -> Light {
    Light {
        owner,
        metadata: LightMetadata {
            name: fixture.friendly_name.to_string(),
            archetype,
            function: Some(LightFunction::Decorative),
            fixed_mired: None,
        },
        product_data: Some(LightProductData {
            function: Some(LightFunction::Decorative),
        }),
        alert: Some(LightAlert {
            action_values: BTreeSet::from(["breathe".to_string()]),
        }),
        color: None,
        color_temperature: None,
        color_temperature_delta: Some(Stub),
        content_configuration: None,
        dimming: Some(Dimming {
            brightness,
            min_dim_level: Some(0.01),
        }),
        dimming_delta: Some(Stub),
        dynamics: Some(LightDynamics {
            status: LightDynamicsStatus::None,
            status_values: vec![
                LightDynamicsStatus::None,
                LightDynamicsStatus::DynamicPalette,
            ],
            speed: 0.0,
            speed_valid: false,
        }),
        effects: None,
        effects_v2: None,
        service_id: Some(0),
        gradient: None,
        identify: Identify {},
        timed_effects: None,
        mode: LightMode::Normal,
        on: On::new(on),
        powerup: Some(LightPowerup {
            preset: LightPowerupPreset::Safety,
            configured: true,
            on: LightPowerupOn::On { on: On::new(true) },
            dimming: LightPowerupDimming::Dimming {
                dimming: DimmingUpdate { brightness: 100.0 },
            },
            color: LightPowerupColor::ColorTemperature {
                color_temperature: ColorTemperatureUpdate::new(366),
            },
        }),
        signaling: Some(LightSignaling {
            signal_values: vec![
                LightSignal::NoSignal,
                LightSignal::OnOff,
                LightSignal::OnOffColor,
                LightSignal::Alternating,
            ],
            status: Value::Null,
        }),
    }
}

const fn color_temperature(mirek_minimum: u32, mirek_maximum: u32) -> ColorTemperature {
    ColorTemperature {
        mirek: None,
        mirek_schema: MirekSchema {
            mirek_minimum,
            mirek_maximum,
        },
        mirek_valid: true,
    }
}

const fn color() -> LightColor {
    LightColor {
        gamut: Some(ColorGamut::GAMUT_C),
        gamut_type: GamutType::C,
        xy: XY::D65_WHITE_POINT,
    }
}

fn add_hue_effects(light: &mut Light) {
    light.effects = Some(LightEffects::all());
    light.effects_v2 = Some(LightEffectsV2::all());
    light.timed_effects = Some(LightTimedEffects {
        status_values: Vec::from(LightTimedEffect::ALL),
        status: LightTimedEffect::NoEffect,
        effect_values: Vec::from(LightTimedEffect::ALL),
    });
}

fn add_gradient(light: &mut Light) {
    light.content_configuration = Some(ContentConfiguration {
        orientation: Some(ContentConfigurationOrientation {
            configurable: true,
            orientation: OrientationType::Horizontal,
            status: ContentConfigurationStatusType::Set,
        }),
        order: Some(ContentConfigurationOrder {
            configurable: true,
            order: OrderType::Forward,
            status: ContentConfigurationStatusType::Set,
        }),
    });
    light.gradient = Some(LightGradient {
        mode: LightGradientMode::InterpolatedPalette,
        mode_values: BTreeSet::from([
            LightGradientMode::InterpolatedPalette,
            LightGradientMode::InterpolatedPaletteMirrored,
            LightGradientMode::RandomPixelated,
            LightGradientMode::SegmentedPalette,
        ]),
        points_capable: 5,
        points: vec![],
        pixel_count: 18,
    });
}

fn z2m_state() -> BTreeMap<String, Value> {
    let ikea_warm_white = fixture::ikea::tradfri_warm_white(&IKEA_WARM_WHITE).with_state(json!({
        "state": "OFF",
        "brightness": 127
    }));

    let ikea_color = fixture::ikea::tradfri_color(&IKEA_COLOR).with_state(json!({
        "state": "OFF",
        "brightness": 127
    }));

    let hue_white_ambiance = fixture::hue::white_ambience(&HUE_WHITE_AMBIANCE).with_state(json!({
        "state": "OFF",
        "brightness": 127
    }));

    let hue_flux_lightstrip =
        fixture::hue::flux_lightstrip(&HUE_FLUX_LIGHTSTRIP).with_state(json!({
            "state": "OFF",
            "brightness": 127
        }));

    let hue_play_gradient_lightstrip =
        fixture::hue::play_gradient_lightstrip(&HUE_PLAY_GRADIENT_LIGHTSTRIP).with_state(json!({
            "state": "OFF",
            "brightness": 127
        }));

    let misc_dimmer_light = fixture::misc::dimmer_light(&MISC_DIMMER_LIGHT).with_state(json!({
        "state": "OFF",
        "brightness": 127
    }));

    let living_room = Z2mGroupFixture::new(1, "livingroom")
        .with_member(&ikea_warm_white)
        .with_member(&ikea_color)
        .with_member(&hue_white_ambiance)
        .with_member(&hue_flux_lightstrip)
        .with_member(&hue_play_gradient_lightstrip)
        .with_member(&misc_dimmer_light)
        .with_state(json!({
            "state": "OFF",
            "brightness": 100
        }));

    let fixture = Z2mFixture::new()
        .with_device(ikea_warm_white)
        .with_device(ikea_color)
        .with_device(hue_white_ambiance)
        .with_device(hue_flux_lightstrip)
        .with_device(hue_play_gradient_lightstrip)
        .with_device(misc_dimmer_light)
        .with_group(living_room);

    fixture.into_state()
}

#[tokio::test]
async fn get_lights() -> TestResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let mut test_bridge = TestBridge::start(z2m_state()).await?;

    test_bridge.wait_for_light(IKEA_WARM_WHITE).await?;
    test_bridge.wait_for_light(IKEA_COLOR).await?;
    test_bridge.wait_for_light(HUE_FLUX_LIGHTSTRIP).await?;
    test_bridge
        .wait_for_light(HUE_PLAY_GRADIENT_LIGHTSTRIP)
        .await?;
    test_bridge.wait_for_light(HUE_WHITE_AMBIANCE).await?;
    test_bridge.wait_for_light(MISC_DIMMER_LIGHT).await?;

    let owner = ResourceLink::new(uuid::Uuid::nil(), RType::Device);
    let mut lights: Vec<Light> = test_bridge
        .hue_client
        .get_lights()
        .await?
        .into_iter()
        .map(|light| Light { owner, ..light })
        .collect();
    lights.sort_by(|a, b| a.metadata.name.cmp(&b.metadata.name));

    let mut hue_flux_lightstrip = expected_dimmable_light(
        owner,
        &HUE_FLUX_LIGHTSTRIP,
        DeviceArchetype::HueLightstrip,
        false,
        50.0,
    );
    hue_flux_lightstrip.color = Some(color());
    hue_flux_lightstrip.color_temperature = Some(color_temperature(50, 1000));
    add_gradient(&mut hue_flux_lightstrip);
    add_hue_effects(&mut hue_flux_lightstrip);

    let mut hue_play_gradient_lightstrip = expected_dimmable_light(
        owner,
        &HUE_PLAY_GRADIENT_LIGHTSTRIP,
        DeviceArchetype::HueLightstripTv,
        false,
        50.0,
    );
    hue_play_gradient_lightstrip.color = Some(color());
    hue_play_gradient_lightstrip.color_temperature = Some(color_temperature(153, 500));
    add_gradient(&mut hue_play_gradient_lightstrip);
    add_hue_effects(&mut hue_play_gradient_lightstrip);

    let mut hue_white_ambiance = expected_dimmable_light(
        owner,
        &HUE_WHITE_AMBIANCE,
        DeviceArchetype::UnknownArchetype,
        false,
        50.0,
    );
    hue_white_ambiance.color_temperature = Some(color_temperature(153, 454));
    add_hue_effects(&mut hue_white_ambiance);

    let mut ikea_color = expected_dimmable_light(
        owner,
        &IKEA_COLOR,
        DeviceArchetype::UnknownArchetype,
        false,
        50.0,
    );
    ikea_color.color = Some(color());
    ikea_color.color_temperature = Some(color_temperature(250, 454));

    let expected = vec![
        hue_flux_lightstrip,
        hue_play_gradient_lightstrip,
        hue_white_ambiance,
        ikea_color,
        expected_dimmable_light(
            owner,
            &IKEA_WARM_WHITE,
            DeviceArchetype::UnknownArchetype,
            false,
            50.0,
        ),
        expected_dimmable_light(
            owner,
            &MISC_DIMMER_LIGHT,
            DeviceArchetype::UnknownArchetype,
            false,
            50.0,
        ),
    ];

    assert_eq!(lights, expected);

    Ok(())
}

#[tokio::test]
async fn turn_on_light() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(IKEA_WARM_WHITE).await?;
    let baseline = test.hue_client.get_light(&light).await?;
    assert_eq!(baseline.on, On::new(false));
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_light(&light, &json!({"on": {"on": true}}))
        .await?;
    z2m_requests
        .expect_set(&light, json!({"state": "ON"}))
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m.publish(&light, json!({"state": "ON"}))?;

    let update = hue_events.expect_update(&light).await?;
    assert_eq!(
        update.data,
        json!({
            "owner": baseline.owner,
            "service_id": baseline.service_id,
            "on": {"on": true},
        })
    );

    let updated = test.hue_client.get_light(&light).await?;
    assert_eq!(
        updated,
        Light {
            on: On::new(true),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
async fn change_brightness() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(IKEA_COLOR).await?;
    let baseline = test.hue_client.get_light(&light).await?;
    assert_eq!(baseline.dimming.map(|d| d.brightness), Some(50.0));
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_light(
            &light,
            &json!({
              "dimming": {
                "brightness": 25
              }
            }),
        )
        .await?;
    z2m_requests
        .expect_set(&light, json!({"brightness": 63.5, "transition": 0.4}))
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m
        .publish(&light, json!({"brightness": 63.5, "state":"ON"}))?;

    let update = hue_events.expect_update(&light).await?;
    assert_eq!(
        update.data,
        json!({
            "owner": baseline.owner,
            "service_id": baseline.service_id,
            "on": {"on": true},
            "dimming": {"brightness": 25.0, "min_dim_level": 0.01},
        })
    );

    let updated = test.hue_client.get_light(&light).await?;
    assert_eq!(
        updated,
        Light {
            on: On::new(true),
            dimming: Some(Dimming {
                brightness: 25.0,
                min_dim_level: Some(0.01),
            }),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
async fn dimming_delta_up() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(IKEA_COLOR).await?;
    let baseline = test.hue_client.get_light(&light).await?;
    assert_eq!(baseline.dimming.map(|d| d.brightness), Some(50.0));
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_light(
            &light,
            &json!({
              "dimming_delta": {
                "action": "up",
                "brightness_delta": 25
              }
            }),
        )
        .await?;
    z2m_requests
        .expect_set(&light, json!({"brightness_step": 63.5}))
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m
        .publish(&light, json!({"brightness": 190.5, "state":"ON"}))?;

    let update = hue_events.expect_update(&light).await?;
    assert_eq!(
        update.data,
        json!({
            "owner": baseline.owner,
            "service_id": baseline.service_id,
            "on": {"on": true},
            "dimming": {"brightness": 75.0, "min_dim_level": 0.01},
        })
    );

    let updated = test.hue_client.get_light(&light).await?;
    assert_eq!(
        updated,
        Light {
            on: On::new(true),
            dimming: Some(Dimming {
                brightness: 75.0,
                min_dim_level: Some(0.01),
            }),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
async fn dimming_delta_down() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(IKEA_COLOR).await?;
    let baseline = test.hue_client.get_light(&light).await?;
    assert_eq!(baseline.dimming.map(|d| d.brightness), Some(50.0));
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_light(
            &light,
            &json!({
              "dimming_delta": {
                "action": "down",
                "brightness_delta": 25
              }
            }),
        )
        .await?;
    z2m_requests
        .expect_set(&light, json!({"brightness_step": -63.5}))
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m
        .publish(&light, json!({"brightness": 63.5, "state":"ON"}))?;

    let update = hue_events.expect_update(&light).await?;
    assert_eq!(
        update.data,
        json!({
            "owner": baseline.owner,
            "service_id": baseline.service_id,
            "on": {"on": true},
            "dimming": {"brightness": 25.0, "min_dim_level": 0.01},
        })
    );

    let updated = test.hue_client.get_light(&light).await?;
    assert_eq!(
        updated,
        Light {
            on: On::new(true),
            dimming: Some(Dimming {
                brightness: 25.0,
                min_dim_level: Some(0.01),
            }),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
async fn identify() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(IKEA_COLOR).await?;
    let baseline = test.hue_client.get_light(&light).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_light(
            &light,
            &json!({
              "identify": {
                "action": "identify"
              }
            }),
        )
        .await?;
    z2m_requests
        .expect_set(&light, json!({"effect": "breathe"}))
        .await?;

    hue_events.expect_quiet().await?;

    // z2m doesn't report anything back for effects

    let updated = test.hue_client.get_light(&light).await?;
    assert_eq!(updated, baseline);
    Ok(())
}

#[tokio::test]
async fn hue_effects_v2() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(HUE_FLUX_LIGHTSTRIP).await?;
    let baseline = test.hue_client.get_light(&light).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_light(
            &light,
            &json!({
              "effects_v2": {
                "action": {
                  "effect": "opal",
                  "parameters": {
                    "color": {
                      "xy": {
                        "x": 0.1,
                        "y": 0.3
                      }
                    },
                    "speed": 0.4
                  }
                }
              }
            }),
        )
        .await?;
    z2m_requests
        .expect_set(
            &light,
            json!(
                {
                    "command": {
                        "cluster": 0xFC03,
                        "command": 0,
                        "payload": {"data": vec![184, 0, 153, 25, 204, 76, 1, 0, 11, 102]}
                    }
                }
            ),
        )
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m.publish(
        &light,
        json!(
            {
                "philips_raw": "ab0001fedf28674c0b66"
            }
        ),
    )?;

    let updated = test.hue_client.get_light(&light).await?;
    assert_eq!(
        updated,
        Light {
            on: On::new(true),
            dimming: baseline.dimming.map(|d| Dimming {
                brightness: 100.0,
                ..d
            }),
            color: baseline.color.map(|c| LightColor {
                xy: XY::new(0.159_655_146_105_134_65, 0.298_451_209_277_485_3),
                ..c
            }),
            effects: baseline.effects.map(|e| LightEffects {
                status: LightEffect::Opal,
                ..e
            }),
            effects_v2: baseline.effects_v2.map(|e| LightEffectsV2 {
                status: LightEffectStatus {
                    effect: LightEffect::Opal,
                    parameters: Some(LightEffectParameters {
                        color: Some(ColorUpdate::new(XY::new(
                            0.159_655_146_105_134_65,
                            0.298_451_209_277_485_3
                        ))),
                        color_temperature: None,
                        speed: Some(0.401_574_8),
                    }),
                    ..e.status
                },
                ..e
            }),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
async fn hue_timed_effects() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(HUE_FLUX_LIGHTSTRIP).await?;
    let baseline = test.hue_client.get_light(&light).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_light(
            &light,
            &json!(
            {
                "on": {
                    "on": true
                },
                "timed_effects": {
                    "effect": "sunrise",
                    "duration": 3000
                }
            }
            ),
        )
        .await?;
    z2m_requests
        .expect_set(
            &light,
            json!(
                {
                    "command": {
                        "cluster": 0xFC03,
                        "command": 0,
                        "payload": {"data": vec![177, 0, 1, 1, 0, 9, 249]}
                    }
                }
            ),
        )
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m.publish(
        &light,
        json!(
            {
                "philips_raw": "ab0001fe9172556809f9"
            }
        ),
    )?;

    let updated = test.hue_client.get_light(&light).await?;
    assert_eq!(
        updated,
        Light {
            on: On::new(true),
            dimming: baseline.dimming.map(|d| Dimming {
                brightness: 100.0,
                ..d
            }),
            color: baseline.color.map(|c| LightColor {
                xy: XY::new(0.447_531_853_208_209_4, 0.407_553_215_838_864_7),
                ..c
            }),
            effects_v2: baseline.effects_v2.map(|e| LightEffectsV2 {
                status: LightEffectStatus {
                    effect: LightEffect::NoEffect,
                    parameters: Some(LightEffectParameters {
                        color: None,
                        color_temperature: None,
                        speed: None,
                    },),
                    ..e.status
                },
                ..e
            }),
            timed_effects: baseline.timed_effects.map(|t| LightTimedEffects {
                status: LightTimedEffect::Sunrise,
                ..t
            }),
            ..baseline
        }
    );
    Ok(())
}

#[tokio::test]
#[allow(clippy::unreadable_literal, clippy::too_many_lines)]
async fn gradient() -> TestResult<()> {
    let mut test = TestBridge::start(z2m_state()).await?;
    let light = test.wait_for_light(HUE_FLUX_LIGHTSTRIP).await?;
    let baseline = test.hue_client.get_light(&light).await?;
    let mut z2m_requests = test.z2m.subscribe_requests();
    let mut hue_events = test.hue_client.subscribe_events();

    test.hue_client
        .put_light(
            &light,
            &json!(
            {
              "gradient": {
                "mode": "interpolated_palette",
                "points": [
                  {
                    "color": {
                      "xy": {
                        "x": 0.24830886446886452,
                        "y": 0.10594871794871805
                      }
                    }
                  },
                  {
                    "color": {
                      "xy": {
                        "x": 0.4463776541837811,
                        "y": 0.20146154827394697
                      }
                    }
                  },
                  {
                    "color": {
                      "xy": {
                        "x": 0.6616785347985349,
                        "y": 0.3241021733821735
                      }
                    }
                  },
                  {
                    "color": {
                      "xy": {
                        "x": 0.482289711059222,
                        "y": 0.44590997007402094
                      }
                    }
                  },
                  {
                    "color": {
                      "xy": {
                        "x": 0.3946169290977102,
                        "y": 0.5072043233088303
                      }
                    }
                  }
                ]
              },
              "on": {
                "on": true
              }
            }
                                    ),
        )
        .await?;
    z2m_requests
        .expect_set(
            &light,
            json!(
                {
                    "command": {
                        "cluster": 0xFC03,
                        "command": 0,
                        "payload": {"data": vec![81,1,1,1,0,19,80,0,0,0,104,213,32,183,105,62,104,110,100,128,26,138,151,24,157,40,0]}
                    }
                }
            ),
        )
        .await?;

    hue_events.expect_quiet().await?;
    test.z2m.publish(
        &light,
        json!(
            {
                "philips_raw": "4b0101fe913fac1b135000000068d520b7693e686e64801a8a97189d2800"
            }
        ),
    )?;

    let updated = test.hue_client.get_light(&light).await?;
    assert_eq!(
        updated,
        Light {
            on: On::new(true),
            dimming: baseline.dimming.map(|d| Dimming {
                brightness: 100.0,
                ..d
            }),
            color: baseline.color.map(|c| LightColor {
                xy: XY::new(0.24831006332494088, 0.10809491111619746),
                ..c
            }),
            effects_v2: baseline.effects_v2.map(|e| LightEffectsV2 {
                status: LightEffectStatus {
                    effect: LightEffect::NoEffect,
                    parameters: Some(LightEffectParameters {
                        color: None,
                        color_temperature: None,
                        speed: None,
                    },),
                    ..e.status
                },
                ..e
            }),
            gradient: baseline.gradient.map(|g| LightGradient {
                points: vec![
                    LightGradientPoint {
                        color: ColorUpdate {
                            xy: XY::new(0.2483088644688645, 0.10594871794871796,),
                        },
                    },
                    LightGradientPoint {
                        color: ColorUpdate {
                            xy: XY::new(0.4462024175824176, 0.20140346764346764,),
                        },
                    },
                    LightGradientPoint {
                        color: ColorUpdate {
                            xy: XY::new(0.6616785347985349, 0.3241021733821734,),
                        },
                    },
                    LightGradientPoint {
                        color: ColorUpdate {
                            xy: XY::new(0.4822646153846154, 0.4457918437118437,),
                        },
                    },
                    LightGradientPoint {
                        color: ColorUpdate {
                            xy: XY::new(0.3945312087912088, 0.5071411965811966,),
                        },
                    },
                ],
                ..g
            }),
            ..baseline
        }
    );
    Ok(())
}
