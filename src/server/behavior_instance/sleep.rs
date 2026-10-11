use std::time::Duration;

use async_trait::async_trait;
use bifrost_api::backend::BackendRequest;
use chrono::NaiveTime;
use futures::future::try_join_all;
use hue::api::{
    GoToSleepConfiguration, GoToSleepEndState, GroupedLightDynamicsUpdate, GroupedLightUpdate,
    LightDynamicsUpdate, LightUpdate, On,
};
use hue::xy::XY;

use crate::error::{ApiError, ApiResult};
use crate::server::behavior_instance::schedule::{Schedule, ScheduledBehavior};
use crate::server::behavior_instance::target::{
    ExecutionTarget, ResolvedLight, SelectionPolicy, TargetResource, resolve_resources,
    select_targets,
};
use crate::server::behavior_instance::{BehaviorContext, service::ScheduleType};

pub struct GoToSleepBehavior {
    pub configuration: GoToSleepConfiguration,
}

#[async_trait]
impl ScheduledBehavior for GoToSleepBehavior {
    fn name(&self) -> &'static str {
        "go to sleep"
    }

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

        Ok(Schedule {
            recurrence: schedule_type,
            scheduled_time: scheduled_wakeup_time,
            lead_time: Duration::ZERO,
        })
    }

    async fn execute(&self, ctx: &BehaviorContext) -> ApiResult<()> {
        let config = &self.configuration;
        log::debug!("Running scheduled behavior instance:, {config:#?}");
        let lock = ctx.res.lock().await;
        let resolved_resources = resolve_resources(&lock, &config.where_field);
        drop(lock);
        let targets = select_targets(&resolved_resources, self);

        let (xy, mirek) = match config.end_state {
            GoToSleepEndState::MinimumBrightness | GoToSleepEndState::TurnOff => {
                (XY::new(0.5018, 0.4152), 447)
            }
            GoToSleepEndState::Nightlight => (XY::new(0.5610, 0.4042), 454),
        };

        try_join_all(
            targets.iter().map(
                |target: &ExecutionTarget<GoToSleepStyle>| match target.effect {
                    GoToSleepStyle::Basic => {
                        target.basic_transition(ctx, config.fade_out_duration.to_std(), xy, mirek)
                    }
                    GoToSleepStyle::Sunset => todo!(),
                    GoToSleepStyle::HueSunset => todo!(),
                },
            ),
        )
        .await?;

        try_join_all(targets.iter().map(async |target| match config.end_state {
            GoToSleepEndState::TurnOff => target.off(ctx).await,
            GoToSleepEndState::MinimumBrightness | GoToSleepEndState::Nightlight => Ok(()),
        }))
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoToSleepStyle {
    Basic,
    Sunset,
    HueSunset,
}

impl SelectionPolicy for GoToSleepBehavior {
    type Effect = GoToSleepStyle;

    fn effect_for_light(&self, light: &ResolvedLight) -> Option<Self::Effect> {
        if !light.state.on.on {
            return None;
        }
        let style = &self.configuration.style;
        match style {
            hue::api::GoToSleepStyle::Sunset => {
                if light.state.effects.is_some() {
                    Some(GoToSleepStyle::HueSunset)
                } else {
                    Some(GoToSleepStyle::Sunset)
                }
            }
            hue::api::GoToSleepStyle::Basic => Some(GoToSleepStyle::Basic),
        }
    }

    fn supports_grouped_light(&self, effect: &Self::Effect) -> bool {
        match effect {
            GoToSleepStyle::Sunset | GoToSleepStyle::Basic => true,
            GoToSleepStyle::HueSunset => false,
        }
    }
}

impl ExecutionTarget<GoToSleepStyle> {
    async fn basic_transition(
        &self,
        ctx: &BehaviorContext,
        fade_out_duration: Duration,
        xy: XY,
        mirek: u16,
    ) -> ApiResult<()> {
        let initial_duration = Duration::from_secs(60);
        let initial_brightness = 100.0 * 128.0 / 254.0;

        let initial_backend_requests =
            self.basic_fade_requests(xy, mirek, initial_duration, initial_brightness)?;
        for request in initial_backend_requests {
            ctx.res.lock().await.backend_request(request)?;
        }

        ctx.clock.sleep(initial_duration).await;

        let main_duration = fade_out_duration
            .checked_sub(Duration::from_secs(60))
            .ok_or(ApiError::InvalidDuration)?;
        let main_brightness = 100.0 * 4.0 / 254.0;

        let main_backend_requests =
            self.basic_fade_requests(xy, mirek, main_duration, main_brightness)?;
        for request in main_backend_requests {
            ctx.res.lock().await.backend_request(request)?;
        }
        ctx.clock.sleep(main_duration).await;

        Ok(())
    }

    fn basic_fade_requests(
        &self,
        xy: XY,
        mirek: u16,
        duration: Duration,
        brightness: f64,
    ) -> ApiResult<Vec<BackendRequest>> {
        let millis = u32::try_from(duration.as_millis())?;
        let light_dynamics = Some(LightDynamicsUpdate::new().with_duration(Some(millis)));
        let grouped_light_dynamics =
            Some(GroupedLightDynamicsUpdate::new().with_duration(Some(millis)));

        let on_request = match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default().with_on(On::new(true)),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default().with_on(On::new(true)),
            ),
        };

        let color_temp_request = match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default()
                    .with_color_temperature(mirek)
                    .with_dynamics(light_dynamics.clone()),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default()
                    .with_color_temperature(mirek)
                    .with_dynamics(grouped_light_dynamics.clone()),
            ),
        };

        let xy_request = match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default()
                    .with_color_xy(xy)
                    .with_dynamics(light_dynamics.clone()),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default()
                    .with_color_xy(xy)
                    .with_dynamics(grouped_light_dynamics.clone()),
            ),
        };

        let brightness_request = match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default()
                    .with_brightness(Some(brightness))
                    .with_dynamics(light_dynamics),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default()
                    .with_brightness(Some(brightness))
                    .with_dynamics(grouped_light_dynamics),
            ),
        };
        Ok(vec![
            on_request,
            color_temp_request,
            xy_request,
            brightness_request,
        ])
    }

    async fn off(&self, ctx: &BehaviorContext) -> ApiResult<()> {
        let initial_backend_request = match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default().with_on(On::new(false)),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default().with_on(On::new(false)),
            ),
        };
        ctx.res
            .lock()
            .await
            .backend_request(initial_backend_request)?;
        Ok(())
    }
}
