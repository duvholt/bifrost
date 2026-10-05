use std::sync::Arc;

use async_trait::async_trait;
use bifrost_api::backend::BackendRequest;
use chrono::NaiveTime;
use itertools::Itertools;
use tokio::sync::Mutex;

use hue::api::{
    Device, Group, GroupedLightDynamicsUpdate, GroupedLightUpdate, Light, LightDynamicsUpdate,
    LightTimedEffect, LightTimedEffectsUpdate, LightUpdate, On, RType, Resource, ResourceLink,
    WakeupConfiguration, WakeupStyle,
};

use crate::error::ApiError;
use crate::server::behavior_instance::schedule::{Schedule, ScheduledBehavior};
use crate::server::behavior_instance::service::ScheduleType;
use crate::server::behavior_instance::{BehaviorClock, BehaviorContext};
use crate::{error::ApiResult, resource::Resources};

pub struct WakeupJob {
    pub configuration: WakeupConfiguration,
}

#[async_trait]
impl ScheduledBehavior for WakeupJob {
    fn schedule(&self) -> ApiResult<Schedule> {
        let weekdays = self.configuration.when.recurrence_days.as_ref();

        #[allow(clippy::option_if_let_else)]
        let schedule_type = match weekdays {
            Some(weekdays) => ScheduleType::Recurring(weekdays.iter().copied().collect()),
            None => ScheduleType::Once(),
        };

        let job_time = self.configuration.when.time_point.time();
        let scheduled_wakeup_time = NaiveTime::from_hms_opt(job_time.hour, job_time.minute, 0)
            .ok_or(ApiError::InvalidNaiveTime)?;

        let fade_in_duration = self.configuration.fade_in_duration.to_std();

        Ok(Schedule {
            recurrence: schedule_type,
            scheduled_time: scheduled_wakeup_time,
            lead_time: fade_in_duration,
        })
    }

    async fn execute(&self, ctx: &BehaviorContext) -> ApiResult<()> {
        run_wake_up(
            self.configuration.clone(),
            ctx.res.clone(),
            ctx.clock.clone(),
        )
        .await
    }
}

fn wakeup_requests_for_group(
    config: &WakeupConfiguration,
    lock: &Resources,
    group: &impl Group,
) -> Vec<WakeupRequest> {
    let lights_in_room: Vec<_> = group
        .children()
        .filter_map(|rl| match rl.rtype {
            RType::Device => lock.get::<Device>(rl).ok().and_then(Device::light_service),
            RType::Light => Some(rl),
            _ => None,
        })
        .filter_map(|light_rl| lock.get::<Light>(light_rl).ok().map(|l| (light_rl, l)))
        .collect();
    if config.style == Some(WakeupStyle::Sunrise)
        && lights_in_room
            .iter()
            .any(|(_, light)| light.effects.is_some())
    {
        // Hue effects do not support grouped lights so we need to send indivial requests to each light
        lights_in_room
            .into_iter()
            .filter_map(|(resource_link, light)| {
                if light.on.on {
                    None
                } else {
                    Some(WakeupRequest::Light(*resource_link))
                }
            })
            .collect()
    } else {
        group
            .grouped_light_service()
            .map_or_else(Vec::new, |grouped_light| {
                vec![WakeupRequest::Group(*grouped_light)]
            })
    }
}

async fn run_wake_up(
    config: WakeupConfiguration,
    res: Arc<Mutex<Resources>>,
    clock: Arc<dyn BehaviorClock>,
) -> ApiResult<()> {
    log::debug!("Running scheduled behavior instance:, {config:#?}");
    #[allow(clippy::option_if_let_else)]
    let resource_links = config.where_field.iter().flat_map(|room| {
        if let Some(items) = &room.items {
            items.clone()
        } else {
            vec![room.group]
        }
    });

    let requests = {
        let lock = res.lock().await;
        resource_links
            .into_iter()
            .filter_map(|resource_link| {
                let resource = lock.get_resource(&resource_link);
                match resource {
                    Ok(resource) => Some((resource_link, resource)),
                    Err(err) => {
                        log::warn!("Failed to get resource: {err}");
                        None
                    }
                }
            })
            .flat_map(|(resource_link, resource)| match resource.obj {
                Resource::Room(room) => wakeup_requests_for_group(&config, &lock, &room),
                Resource::Zone(zone) => wakeup_requests_for_group(&config, &lock, &zone),
                Resource::Light(light) => {
                    if light.on.on {
                        vec![]
                    } else {
                        vec![WakeupRequest::Light(resource_link)]
                    }
                }
                Resource::BridgeHome(_bridge_home) => {
                    let all_rooms = lock.get_resources_by_type(RType::Room);
                    all_rooms
                        .into_iter()
                        .filter_map(|room_resource| match room_resource.obj {
                            Resource::Room(room) => {
                                Some(wakeup_requests_for_group(&config, &lock, &room))
                            }
                            _ => None,
                        })
                        .concat()
                }
                _ => Vec::new(),
            })
            .collect::<Vec<_>>()
    };

    for request in &requests {
        request
            .on(res.clone(), config.clone())
            .await
            .inspect_err(|err| {
                log::warn!("Failed to turn on wake up light: {err}");
            })?;
    }

    // wait until fade in has completed
    // otherwise the behavior instance can be disabled before it has actually finished
    clock.sleep(config.fade_in_duration.to_std()).await;

    if let Some(duration) = config.turn_lights_off_after {
        clock.sleep(duration.to_std()).await;

        for request in &requests {
            request.off(res.clone()).await.inspect_err(|err| {
                log::warn!("Failed to turn off wake up light: {err}");
            })?;
        }
    }
    Ok(())
}

enum WakeupRequest {
    Light(ResourceLink),
    Group(ResourceLink),
}

impl WakeupRequest {
    async fn on(&self, res: Arc<Mutex<Resources>>, config: WakeupConfiguration) -> ApiResult<()> {
        let light_supports_effects = match self {
            Self::Light(resource_link) => res
                .lock()
                .await
                .get::<Light>(resource_link)?
                .effects
                .is_some(),
            Self::Group(_) => false,
        };
        let use_sunrise_effect =
            light_supports_effects && config.style == Some(WakeupStyle::Sunrise);

        if use_sunrise_effect {
            self.sunrise_on(&res, &config).await?;
        } else {
            self.transition_to_bright_on(res, &config).await?;
        }
        Ok(())
    }

    async fn transition_to_bright_on(
        &self,
        res: Arc<Mutex<Resources>>,
        config: &WakeupConfiguration,
    ) -> Result<(), ApiError> {
        // As reported by the Hue bridge
        const WAKEUP_FADE_MIREK: u16 = 447;

        let initial_backend_requests = match self {
            Self::Light(resource_link) => {
                let on_brightness = LightUpdate::default()
                    .with_on(On::new(true))
                    .with_brightness(Some(1.0));
                let color_temperature =
                    LightUpdate::default().with_color_temperature(WAKEUP_FADE_MIREK);

                vec![
                    BackendRequest::LightUpdate(*resource_link, on_brightness),
                    BackendRequest::LightUpdate(*resource_link, color_temperature),
                ]
            }
            Self::Group(resource_link) => {
                let brightness = GroupedLightUpdate::default()
                    .with_on(On::new(true))
                    .with_brightness(Some(1.0));
                let color_temperature =
                    GroupedLightUpdate::default().with_color_temperature(WAKEUP_FADE_MIREK);
                vec![
                    BackendRequest::GroupedLightUpdate(*resource_link, brightness),
                    BackendRequest::GroupedLightUpdate(*resource_link, color_temperature),
                ]
            }
        };
        for request in initial_backend_requests {
            res.lock().await.backend_request(request)?;
        }

        // Start fade in to set brightness
        let on_backend_requests = match self {
            Self::Light(resource_link) => {
                let brightness = LightUpdate::default()
                    .with_brightness(Some(config.end_brightness))
                    .with_dynamics(Some(
                        LightDynamicsUpdate::new()
                            .with_duration(Some((config.fade_in_duration.seconds) * 1000)),
                    ));
                vec![BackendRequest::LightUpdate(*resource_link, brightness)]
            }
            Self::Group(resource_link) => {
                let brightness = GroupedLightUpdate::default()
                    .with_brightness(Some(config.end_brightness))
                    .with_dynamics(Some(
                        GroupedLightDynamicsUpdate::new()
                            .with_duration(Some((config.fade_in_duration.seconds) * 1000)),
                    ));
                vec![BackendRequest::GroupedLightUpdate(
                    *resource_link,
                    brightness,
                )]
            }
        };
        for request in on_backend_requests {
            res.lock().await.backend_request(request)?;
        }
        Ok(())
    }

    async fn sunrise_on(
        &self,
        res: &Arc<Mutex<Resources>>,
        config: &WakeupConfiguration,
    ) -> Result<(), ApiError> {
        match self {
            Self::Light(resource_link) => {
                let mut payload = LightUpdate::default()
                    .with_on(Some(On::new(true)))
                    .with_brightness(Some(config.end_brightness));
                payload.timed_effects = Some(LightTimedEffectsUpdate {
                    effect: Some(LightTimedEffect::Sunrise),
                    duration: Some(config.fade_in_duration.seconds * 1000),
                });
                res.lock()
                    .await
                    .backend_request(BackendRequest::LightUpdate(*resource_link, payload))?;
            }
            Self::Group(_resource_link) => {}
        }
        Ok(())
    }

    async fn off(&self, res: Arc<Mutex<Resources>>) -> ApiResult<()> {
        let backend_request = match self {
            Self::Light(resource_link) => {
                let payload = LightUpdate::default().with_on(Some(On::new(false)));
                BackendRequest::LightUpdate(*resource_link, payload)
            }
            Self::Group(resource_link) => {
                let payload = GroupedLightUpdate::default().with_on(Some(On::new(false)));
                BackendRequest::GroupedLightUpdate(*resource_link, payload)
            }
        };

        res.lock().await.backend_request(backend_request)
    }
}
