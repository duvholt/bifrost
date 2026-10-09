use std::sync::Arc;

use async_trait::async_trait;
use bifrost_api::backend::BackendRequest;
use chrono::NaiveTime;
use tokio::sync::Mutex;

use hue::api::{
    GroupedLightDynamicsUpdate, GroupedLightUpdate, LightDynamicsUpdate, LightTimedEffect,
    LightTimedEffectsUpdate, LightUpdate, On, WakeupConfiguration, WakeupStyle,
};

use crate::error::ApiError;
use crate::server::behavior_instance::BehaviorContext;
use crate::server::behavior_instance::schedule::{Schedule, ScheduledBehavior};
use crate::server::behavior_instance::service::ScheduleType;
use crate::server::behavior_instance::target::{
    ExecutionTarget, ResolvedLight, SelectionPolicy, TargetResource, resolve_resources,
    select_targets,
};
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
        let config = &self.configuration;
        log::debug!("Running scheduled behavior instance:, {config:#?}");
        let lock = ctx.res.lock().await;
        let resolved_resources = resolve_resources(&lock, &config.where_field);
        drop(lock);
        let targets = select_targets(&resolved_resources, &self);

        for target in &targets {
            target
                .on(ctx.res.clone(), config.clone())
                .await
                .inspect_err(|err| {
                    log::warn!("Failed to turn on wake up light: {err}");
                })?;
        }

        // wait until fade in has completed
        // otherwise the behavior instance can be disabled before it has actually finished
        ctx.clock.sleep(config.fade_in_duration.to_std()).await;

        if let Some(duration) = &config.turn_lights_off_after {
            ctx.clock.sleep(duration.to_std()).await;

            for target in &targets {
                target.off(ctx.res.clone()).await.inspect_err(|err| {
                    log::warn!("Failed to turn off wake up light: {err}");
                })?;
            }
        }
        Ok(())
    }
}

impl SelectionPolicy for &WakeupJob {
    type Effect = WakeupStyle;

    fn effect_for_light(&self, light: &ResolvedLight) -> Option<Self::Effect> {
        if light.state.on.on {
            return None;
        }
        let style = &self.configuration.style;
        match style {
            Some(WakeupStyle::Sunrise) => {
                if light.state.effects.is_some() {
                    Some(WakeupStyle::Sunrise)
                } else {
                    Some(WakeupStyle::Basic)
                }
            }
            Some(WakeupStyle::Basic) | None => Some(WakeupStyle::Basic),
        }
    }

    fn supports_grouped_light(&self, effect: &Self::Effect) -> bool {
        match effect {
            WakeupStyle::Sunrise => false,
            WakeupStyle::Basic => true,
        }
    }
}

impl ExecutionTarget<WakeupStyle> {
    async fn on(&self, res: Arc<Mutex<Resources>>, config: WakeupConfiguration) -> ApiResult<()> {
        match self.effect {
            WakeupStyle::Sunrise => self.sunrise_on(&res, &config).await,
            WakeupStyle::Basic => self.transition_to_bright_on(res, &config).await,
        }
    }

    async fn transition_to_bright_on(
        &self,
        res: Arc<Mutex<Resources>>,
        config: &WakeupConfiguration,
    ) -> Result<(), ApiError> {
        // As reported by the Hue bridge
        const WAKEUP_FADE_MIREK: u16 = 447;

        let initial_backend_requests = match self.resource {
            TargetResource::Light(resource_link) => {
                let on_brightness = LightUpdate::default()
                    .with_on(On::new(true))
                    .with_brightness(Some(1.0));
                let color_temperature =
                    LightUpdate::default().with_color_temperature(WAKEUP_FADE_MIREK);

                vec![
                    BackendRequest::LightUpdate(resource_link, on_brightness),
                    BackendRequest::LightUpdate(resource_link, color_temperature),
                ]
            }
            TargetResource::GroupedLight(resource_link) => {
                let brightness = GroupedLightUpdate::default()
                    .with_on(On::new(true))
                    .with_brightness(Some(1.0));
                let color_temperature =
                    GroupedLightUpdate::default().with_color_temperature(WAKEUP_FADE_MIREK);
                vec![
                    BackendRequest::GroupedLightUpdate(resource_link, brightness),
                    BackendRequest::GroupedLightUpdate(resource_link, color_temperature),
                ]
            }
        };
        for request in initial_backend_requests {
            res.lock().await.backend_request(request)?;
        }

        // Start fade in to set brightness
        let on_backend_requests = match self.resource {
            TargetResource::Light(resource_link) => {
                let brightness = LightUpdate::default()
                    .with_brightness(Some(config.end_brightness))
                    .with_dynamics(Some(
                        LightDynamicsUpdate::new()
                            .with_duration(Some((config.fade_in_duration.seconds) * 1000)),
                    ));
                vec![BackendRequest::LightUpdate(resource_link, brightness)]
            }
            TargetResource::GroupedLight(resource_link) => {
                let brightness = GroupedLightUpdate::default()
                    .with_brightness(Some(config.end_brightness))
                    .with_dynamics(Some(
                        GroupedLightDynamicsUpdate::new()
                            .with_duration(Some((config.fade_in_duration.seconds) * 1000)),
                    ));
                vec![BackendRequest::GroupedLightUpdate(
                    resource_link,
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
        match self.resource {
            TargetResource::Light(resource_link) => {
                let mut payload = LightUpdate::default()
                    .with_on(Some(On::new(true)))
                    .with_brightness(Some(config.end_brightness));
                payload.timed_effects = Some(LightTimedEffectsUpdate {
                    effect: Some(LightTimedEffect::Sunrise),
                    duration: Some(config.fade_in_duration.seconds * 1000),
                });
                res.lock()
                    .await
                    .backend_request(BackendRequest::LightUpdate(resource_link, payload))?;
            }
            TargetResource::GroupedLight(_resource_link) => {}
        }
        Ok(())
    }

    async fn off(&self, res: Arc<Mutex<Resources>>) -> ApiResult<()> {
        let backend_request = match self.resource {
            TargetResource::Light(resource_link) => {
                let payload = LightUpdate::default().with_on(Some(On::new(false)));
                BackendRequest::LightUpdate(resource_link, payload)
            }
            TargetResource::GroupedLight(resource_link) => {
                let payload = GroupedLightUpdate::default().with_on(Some(On::new(false)));
                BackendRequest::GroupedLightUpdate(resource_link, payload)
            }
        };

        res.lock().await.backend_request(backend_request)
    }
}
