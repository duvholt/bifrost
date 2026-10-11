use std::time::Duration;

use async_trait::async_trait;
use bifrost_api::backend::BackendRequest;
use chrono::NaiveTime;
use futures::future::try_join_all;
use hue::api::{
    DimmingDeltaUpdate, GoToSleepConfiguration, GoToSleepEndState, GroupedLightDynamicsUpdate,
    GroupedLightUpdate, LightDynamicsUpdate, LightTimedEffect, LightTimedEffectsUpdate,
    LightUpdate, On,
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

const NIGHTLIGHT_COLOR: XY = XY::new(0.5610, 0.4042);
const MINIMUM_LIGHT_COLOR: XY = XY::new(0.5018, 0.4152);
const SUNSET_COLORS: [XY; 4] = [
    XY::new(0.5056, 0.4141),
    XY::new(0.51591, 0.41461),
    XY::new(0.53781, 0.4111),
    XY::new(0.58571, 0.39311),
];

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

        let duration = config.fade_out_duration.to_std();
        try_join_all(
            targets
                .iter()
                .map(
                    async |target: &ExecutionTarget<GoToSleepStyle>| match target.effect {
                        GoToSleepStyle::Basic => {
                            let (xy, mirek) = match config.end_state {
                                GoToSleepEndState::MinimumBrightness
                                | GoToSleepEndState::TurnOff => (MINIMUM_LIGHT_COLOR, 447),
                                GoToSleepEndState::Nightlight => (NIGHTLIGHT_COLOR, 454),
                            };
                            target.basic_transition(ctx, duration, xy, mirek).await
                        }
                        GoToSleepStyle::Sunset => target.sunset_transition(ctx, duration).await,
                        GoToSleepStyle::HueSunset => target.sunset_hue_effect(ctx, duration).await,
                    },
                ),
        )
        .await?;

        try_join_all(targets.iter().map(async |target| match target.effect {
            GoToSleepStyle::Basic => match config.end_state {
                GoToSleepEndState::TurnOff => target.off(ctx).await,
                GoToSleepEndState::MinimumBrightness | GoToSleepEndState::Nightlight => Ok(()),
            },
            GoToSleepStyle::Sunset | GoToSleepStyle::HueSunset => match config.end_state {
                GoToSleepEndState::TurnOff => target.off(ctx).await,
                GoToSleepEndState::MinimumBrightness => target.minimum_brightness(ctx).await,
                GoToSleepEndState::Nightlight => target.nightlight(ctx).await,
            },
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
            GoToSleepStyle::Basic => true,
            // Sunset: uses different steps depending on initial light level
            // Hue sunset: uses Hue effects which doesn't support groups
            GoToSleepStyle::Sunset | GoToSleepStyle::HueSunset => false,
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
            self.basic_fade_requests(xy, mirek, initial_duration, initial_brightness);
        for request in initial_backend_requests {
            ctx.res.lock().await.backend_request(request)?;
        }
        ctx.clock.sleep(initial_duration).await;

        let main_duration = fade_out_duration
            .checked_sub(initial_duration)
            .ok_or(ApiError::InvalidDuration)?;
        let main_brightness = 100.0 * 4.0 / 254.0;

        let main_backend_requests =
            self.basic_fade_requests(xy, mirek, main_duration, main_brightness);
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
    ) -> Vec<BackendRequest> {
        vec![
            self.on_request(true, None),
            self.mirek_request(mirek, duration),
            self.xy_request(xy, duration),
            self.brightness_request(brightness, duration),
        ]
    }

    async fn sunset_transition(&self, ctx: &BehaviorContext, _duration: Duration) -> ApiResult<()> {
        let preparation_duration = Duration::from_secs(2);

        let preparation_requests = vec![
            self.dimming_delta_request(100.0 * 71.0 / 254.0, preparation_duration),
            self.mirek_request(455, preparation_duration),
            self.xy_request(SUNSET_COLORS[0], preparation_duration),
        ];
        for request in preparation_requests {
            ctx.res.lock().await.backend_request(request)?;
        }
        ctx.clock.sleep(preparation_duration).await;

        let stage_duration = Duration::from_millis(74_500);

        for (level, xy, duration) in [
            (126.0, SUNSET_COLORS[1], stage_duration),
            (95.0, SUNSET_COLORS[2], stage_duration),
            (52.0, SUNSET_COLORS[3], stage_duration),
        ] {
            for request in [
                self.on_request(true, None),
                self.xy_request(xy, duration),
                self.brightness_request(100.0 * level / 254.0, duration),
            ] {
                ctx.res.lock().await.backend_request(request)?;
            }
            ctx.clock.sleep(duration).await;
        }

        let final_duration = stage_duration
            .checked_sub(preparation_duration)
            .ok_or(ApiError::InvalidDuration)?;
        ctx.res
            .lock()
            .await
            .backend_request(self.brightness_request(100.0 * 4.0 / 254.0, final_duration))?;
        ctx.clock.sleep(stage_duration).await;

        Ok(())
    }

    async fn sunset_hue_effect(&self, ctx: &BehaviorContext, duration: Duration) -> ApiResult<()> {
        match self.resource {
            TargetResource::Light(resource_link) => {
                let mut payload = LightUpdate::default()
                    .with_on(Some(On::new(true)))
                    .with_dynamics(Self::light_dynamics_duration(Duration::from_secs(2)));
                payload.timed_effects = Some(LightTimedEffectsUpdate {
                    effect: Some(LightTimedEffect::Sunset),
                    duration: Some(u32::try_from(duration.as_millis())?),
                });
                let request = BackendRequest::LightUpdate(resource_link, payload);

                ctx.res.lock().await.backend_request(request)?;
                ctx.clock.sleep(duration).await;
            }
            TargetResource::GroupedLight(_) => {
                // The sunset effect does not support grouped light requests
            }
        }

        Ok(())
    }

    fn mirek_request(&self, mirek: u16, duration: Duration) -> BackendRequest {
        match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default()
                    .with_color_temperature(mirek)
                    .with_dynamics(Self::light_dynamics_duration(duration)),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default()
                    .with_color_temperature(mirek)
                    .with_dynamics(Self::grouped_light_dynamics_duration(duration)),
            ),
        }
    }

    fn xy_request(&self, xy: XY, duration: Duration) -> BackendRequest {
        match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default()
                    .with_color_xy(xy)
                    .with_dynamics(Self::light_dynamics_duration(duration)),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default()
                    .with_color_xy(xy)
                    .with_dynamics(Self::grouped_light_dynamics_duration(duration)),
            ),
        }
    }

    fn brightness_request(&self, brightness: f64, duration: Duration) -> BackendRequest {
        match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default()
                    .with_brightness(Some(brightness))
                    .with_dynamics(Self::light_dynamics_duration(duration)),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default()
                    .with_brightness(Some(brightness))
                    .with_dynamics(Self::grouped_light_dynamics_duration(duration)),
            ),
        }
    }

    fn on_request(&self, on: bool, duration: Option<Duration>) -> BackendRequest {
        match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default()
                    .with_on(On::new(on))
                    .with_dynamics(duration.and_then(Self::light_dynamics_duration)),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default()
                    .with_on(On::new(on))
                    .with_dynamics(duration.and_then(Self::grouped_light_dynamics_duration)),
            ),
        }
    }

    fn dimming_delta_request(&self, brightness_delta: f64, duration: Duration) -> BackendRequest {
        let dimming_delta =
            DimmingDeltaUpdate::new(hue::api::DimmingDeltaAction::Down, brightness_delta);

        match self.resource {
            TargetResource::Light(resource_link) => BackendRequest::LightUpdate(
                resource_link,
                LightUpdate::default()
                    .with_dimming_delta(Some(dimming_delta))
                    .with_dynamics(Self::light_dynamics_duration(duration)),
            ),
            TargetResource::GroupedLight(resource_link) => BackendRequest::GroupedLightUpdate(
                resource_link,
                GroupedLightUpdate::default()
                    .with_dimming_delta(Some(dimming_delta))
                    .with_dynamics(Self::grouped_light_dynamics_duration(duration)),
            ),
        }
    }

    fn light_dynamics_duration(duration: Duration) -> Option<LightDynamicsUpdate> {
        let millis = u32::try_from(duration.as_millis()).ok()?;
        Some(LightDynamicsUpdate::new().with_duration(Some(millis)))
    }

    fn grouped_light_dynamics_duration(duration: Duration) -> Option<GroupedLightDynamicsUpdate> {
        let millis = u32::try_from(duration.as_millis()).ok()?;
        Some(GroupedLightDynamicsUpdate::new().with_duration(Some(millis)))
    }

    async fn off(&self, ctx: &BehaviorContext) -> ApiResult<()> {
        let duration = Duration::from_millis(400);
        let initial_backend_request = self.on_request(false, Some(duration));
        ctx.res
            .lock()
            .await
            .backend_request(initial_backend_request)?;
        Ok(())
    }

    async fn nightlight(&self, ctx: &BehaviorContext) -> ApiResult<()> {
        let duration = Duration::from_millis(400);
        let requests = vec![
            self.on_request(true, None),
            self.xy_request(NIGHTLIGHT_COLOR, duration),
            self.brightness_request(100.0 * 4.0 / 254.0, duration),
        ];
        for request in requests {
            ctx.res.lock().await.backend_request(request)?;
        }
        Ok(())
    }

    async fn minimum_brightness(&self, ctx: &BehaviorContext) -> ApiResult<()> {
        let duration = Duration::from_millis(400);
        let requests = vec![
            self.on_request(true, None),
            self.xy_request(MINIMUM_LIGHT_COLOR, duration),
            self.brightness_request(100.0 * 4.0 / 254.0, duration),
        ];
        for request in requests {
            ctx.res.lock().await.backend_request(request)?;
        }
        Ok(())
    }
}
