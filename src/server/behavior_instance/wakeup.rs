use std::sync::Arc;
use std::time::Duration;

use bifrost_api::backend::BackendRequest;
use chrono::offset::LocalResult;
use chrono::{DateTime, Days, Local, NaiveTime, Timelike, Weekday};
use itertools::Itertools;
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_schedule::{Job, every};

use hue::api::{
    Device, GroupedLightDynamicsUpdate, GroupedLightUpdate, Light, LightDynamicsUpdate,
    LightTimedEffect, LightTimedEffectsUpdate, LightUpdate, On, RType, Resource, ResourceLink,
    Room, WakeupConfiguration, WakeupStyle,
};
use hue::effect_duration::EffectDuration;
use uuid::Uuid;

use crate::error::ApiError;
use crate::server::behavior_instance::service::{ScheduleType, disable_behavior_instance};
use crate::{error::ApiResult, resource::Resources};

pub struct WakeupJob {
    pub rid: Uuid,
    pub schedule_type: ScheduleType,
    pub configuration: WakeupConfiguration,
    pub res: Arc<Mutex<Resources>>,
}

impl WakeupJob {
    fn start_datetime(&self, now: DateTime<Local>) -> ApiResult<DateTime<Local>> {
        let start_time = self.start_time()?;
        let next = match now.with_time(start_time) {
            LocalResult::Single(time) => time,
            LocalResult::Ambiguous(_, latest) => latest,
            LocalResult::None => {
                return Err(ApiError::InvalidDateTimeConversion(now));
            }
        };
        let wakeup_datetime = if next < now {
            next.checked_add_days(Days::new(1))
                .ok_or(ApiError::InvalidDateTimeConversion(next))?
        } else {
            next
        };
        Ok(wakeup_datetime)
    }

    fn start_time(&self) -> ApiResult<NaiveTime> {
        let job_time = self.configuration.when.time_point.time();
        let scheduled_wakeup_time = NaiveTime::from_hms_opt(job_time.hour, job_time.minute, 0)
            .ok_or(ApiError::InvalidNaiveTime)?;
        // although the scheduled time in the Hue app is the time when lights are at full brightness
        // the job start time is considered to be when the fade in effects starts
        let fade_in_duration = self.configuration.fade_in_duration.to_std();
        Ok(scheduled_wakeup_time - fade_in_duration)
    }

    pub async fn create(self) {
        let now = Local::now();
        let config = self.configuration.clone();
        let result = match &self.schedule_type {
            ScheduleType::Recurring(weekday) => self.create_recurring(*weekday).await,
            ScheduleType::Once() => self.run_once(now),
        };
        if let Err(err) = result {
            log::error!(
                "Failed to create wake up job: {}, using configuration {:?}",
                err,
                config
            );
        }
    }

    async fn create_recurring(&self, weekday: Weekday) -> ApiResult<()> {
        let fade_in_start = self.start_time()?;
        every(1)
            .week()
            .on(weekday)
            .at(
                fade_in_start.hour(),
                fade_in_start.minute(),
                fade_in_start.second(),
            )
            .perform(|| {
                let wakeup_configuration = self.configuration.clone();
                async move {
                    spawn(run_wake_up(wakeup_configuration.clone(), self.res.clone()));
                }
            })
            .await;
        Ok(())
    }

    fn run_once(self, now: DateTime<Local>) -> ApiResult<()> {
        let fade_in_datetime = self.start_datetime(now)?;
        let time_until_fade_in = (fade_in_datetime - now).to_std()?;
        spawn(async move {
            sleep(time_until_fade_in).await;
            run_wake_up(self.configuration.clone(), self.res.clone()).await;
            disable_behavior_instance(self.rid, self.res.clone()).await;
        });

        Ok(())
    }
}

async fn run_wake_up(config: WakeupConfiguration, res: Arc<Mutex<Resources>>) {
    log::debug!("Running scheduled behavior instance:, {:#?}", config);
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
        let room_requests = |room: &Room| {
            let lights_in_room: Vec<_> = room
                .children
                .iter()
                .filter_map(|rl| lock.get::<Device>(rl).ok())
                .filter_map(Device::light_service)
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
                    .map(|(resource_link, _)| WakeupRequest::Light(*resource_link))
                    .collect()
            } else {
                room.grouped_light_service()
                    .map_or_else(Vec::new, |grouped_light| {
                        vec![WakeupRequest::Group(*grouped_light)]
                    })
            }
        };
        resource_links
            .into_iter()
            .filter_map(|resource_link| {
                let resource = lock.get_resource(&resource_link);
                match resource {
                    Ok(resource) => Some((resource_link, resource)),
                    Err(err) => {
                        log::warn!("Failed to get resource: {}", err);
                        None
                    }
                }
            })
            .flat_map(|(resource_link, resource)| match resource.obj {
                Resource::Room(room) => room_requests(&room),
                Resource::Light(_light) => {
                    vec![WakeupRequest::Light(resource_link)]
                }
                Resource::BridgeHome(_bridge_home) => {
                    let all_rooms = lock.get_resources_by_type(RType::Room);
                    all_rooms
                        .into_iter()
                        .filter_map(|room_resource| match room_resource.obj {
                            Resource::Room(room) => Some(room_requests(&room)),
                            _ => None,
                        })
                        .concat()
                }
                _ => Vec::new(),
            })
            .collect::<Vec<_>>()
    };

    for request in &requests {
        if let Err(err) = request.on(res.clone(), config.clone()).await {
            log::warn!("Failed to turn on wake up light: {}", err);
        }
    }

    // wait until fade in has completed
    // otherwise the behavior instance can be disabled before it has actually finished
    sleep(config.fade_in_duration.to_std()).await;

    if let Some(duration) = config.turn_lights_off_after {
        sleep(duration.to_std()).await;

        for request in &requests {
            if let Err(err) = request.off(res.clone()).await {
                log::warn!("Failed to turn off wake up light: {}", err);
            }
        }
    }
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
                let effect_duration =
                    EffectDuration::from_seconds(config.fade_in_duration.seconds)?;
                payload.timed_effects = Some(LightTimedEffectsUpdate {
                    effect: Some(LightTimedEffect::Sunrise),
                    duration: Some(effect_duration.0 as u32 * 1000),
                });
                res.lock()
                    .await
                    .backend_request(BackendRequest::LightUpdate(*resource_link, payload))?;
            }
            Self::Group(_resource_link) => {}
        };
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
