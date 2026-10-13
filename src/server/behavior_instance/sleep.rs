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
            GoToSleepStyle::Sunset => match config.end_state {
                GoToSleepEndState::TurnOff => target.off(ctx).await,
                GoToSleepEndState::MinimumBrightness => {
                    target.end_with_color(ctx, MINIMUM_LIGHT_COLOR).await
                }
                GoToSleepEndState::Nightlight => target.end_with_color(ctx, NIGHTLIGHT_COLOR).await,
            },
            GoToSleepStyle::HueSunset => match config.end_state {
                GoToSleepEndState::TurnOff => Ok(()),
                GoToSleepEndState::MinimumBrightness => {
                    target.end_with_color(ctx, MINIMUM_LIGHT_COLOR).await
                }
                GoToSleepEndState::Nightlight => target.end_with_color(ctx, NIGHTLIGHT_COLOR).await,
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

    async fn sunset_transition(&self, ctx: &BehaviorContext, duration: Duration) -> ApiResult<()> {
        let TargetResource::Light {
            initial_brightness, ..
        } = self.resource
        else {
            return Ok(());
        };
        let initial_brightness = initial_brightness.unwrap_or(0.0);
        let preparation_duration = Duration::from_secs(2);

        let preparation_requests = if initial_brightness > 56.0 {
            vec![
                self.dimming_delta_request(100.0 * 71.0 / 254.0, preparation_duration),
                self.mirek_request(455, preparation_duration),
                self.xy_request(SUNSET_COLORS[0], preparation_duration),
            ]
        } else {
            vec![
                self.mirek_request(455, preparation_duration),
                self.xy_request(SUNSET_COLORS[0], preparation_duration),
            ]
        };
        for request in preparation_requests {
            ctx.res.lock().await.backend_request(request)?;
        }
        ctx.clock.sleep(preparation_duration).await;

        let stage_duration = duration
            .checked_sub(preparation_duration)
            .ok_or(ApiError::InvalidDuration)?
            / 4;

        let brightness_stages = sunset_stages(initial_brightness);

        for (level, xy, duration) in [
            (brightness_stages[0], SUNSET_COLORS[1], stage_duration),
            (brightness_stages[1], SUNSET_COLORS[2], stage_duration),
            (brightness_stages[2], SUNSET_COLORS[3], stage_duration),
        ] {
            for request in [
                self.on_request(true, None),
                self.xy_request(xy, duration),
                self.brightness_request(100.0 * f64::from(level) / 254.0, duration),
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
            TargetResource::Light {
                link: resource_link,
                ..
            } => {
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
            TargetResource::Light {
                link: resource_link,
                ..
            } => BackendRequest::LightUpdate(
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
            TargetResource::Light {
                link: resource_link,
                ..
            } => BackendRequest::LightUpdate(
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
            TargetResource::Light {
                link: resource_link,
                ..
            } => BackendRequest::LightUpdate(
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
            TargetResource::Light {
                link: resource_link,
                ..
            } => BackendRequest::LightUpdate(
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
            TargetResource::Light {
                link: resource_link,
                ..
            } => BackendRequest::LightUpdate(
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

    async fn end_with_color(&self, ctx: &BehaviorContext, xy: XY) -> ApiResult<()> {
        let duration = Duration::from_millis(400);
        let requests = if self.effect == GoToSleepStyle::HueSunset
            && let TargetResource::Light { link, .. } = self.resource
        {
            let brightness = 100.0 * (4.0 - 1.0) / 253.0;
            vec![
                BackendRequest::LightUpdate(
                    link,
                    LightUpdate {
                        timed_effects: Some(LightTimedEffectsUpdate::no_effect()),
                        ..Default::default()
                    }
                    .with_dynamics(Self::light_dynamics_duration(duration)),
                ),
                BackendRequest::LightUpdate(
                    link,
                    LightUpdate::default()
                        .with_on(On::new(true))
                        .with_color_xy(xy)
                        .with_brightness(Some(brightness))
                        .with_dynamics(Self::light_dynamics_duration(duration)),
                ),
            ]
        } else {
            let brightness = 100.0 * 4.0 / 254.0;
            vec![
                self.on_request(true, None),
                self.xy_request(xy, duration),
                self.brightness_request(brightness, duration),
            ]
        };
        for request in requests {
            ctx.res.lock().await.backend_request(request)?;
        }
        Ok(())
    }
}

fn sunset_stages(brightness: f64) -> [u8; 3] {
    let prepared = 56.0f64.min(brightness - (if brightness > 56.0 { 28.0 } else { 0.0 }));

    #[allow(clippy::cast_sign_loss)]
    #[allow(clippy::cast_possible_truncation)]
    let stage = |rate: f64| (1.0 + (253.0 * prepared * rate / 100.0)).round() as u8;
    [stage(0.88), stage(0.66), stage(0.36)]
}

#[cfg(test)]
mod tests {
    use super::sunset_stages;
    // Measured cases: (brightness %, brightness level, preparation drop, stage1 level)
    const SUNSET_STAGE1_MEASUREMENTS: [(f64, u8, u8, u8); 254] = [
        (0.00, 1, 0, 1),
        (0.40, 2, 0, 2),
        (0.79, 3, 0, 3),
        (1.19, 4, 0, 4),
        (1.58, 5, 0, 5),
        (1.98, 6, 0, 5),
        (2.37, 7, 0, 6),
        (2.77, 8, 0, 7),
        (3.16, 9, 0, 8),
        (3.56, 10, 0, 9),
        (3.95, 11, 0, 10),
        (4.35, 12, 0, 11),
        (4.74, 13, 0, 12),
        (5.14, 14, 0, 12),
        (5.53, 15, 0, 13),
        (5.93, 16, 0, 14),
        (6.32, 17, 0, 15),
        (6.72, 18, 0, 16),
        (7.11, 19, 0, 17),
        (7.51, 20, 0, 18),
        (7.91, 21, 0, 19),
        (8.30, 22, 0, 19),
        (8.70, 23, 0, 20),
        (9.09, 24, 0, 21),
        (9.49, 25, 0, 22),
        (9.88, 26, 0, 23),
        (10.28, 27, 0, 24),
        (10.67, 28, 0, 25),
        (11.07, 29, 0, 26),
        (11.46, 30, 0, 27),
        (11.86, 31, 0, 27),
        (12.25, 32, 0, 28),
        (12.65, 33, 0, 29),
        (13.04, 34, 0, 30),
        (13.44, 35, 0, 31),
        (13.83, 36, 0, 32),
        (14.23, 37, 0, 33),
        (14.62, 38, 0, 34),
        (15.02, 39, 0, 34),
        (15.42, 40, 0, 35),
        (15.81, 41, 0, 36),
        (16.21, 42, 0, 37),
        (16.60, 43, 0, 38),
        (17.00, 44, 0, 39),
        (17.39, 45, 0, 40),
        (17.79, 46, 0, 41),
        (18.18, 47, 0, 41),
        (18.58, 48, 0, 42),
        (18.97, 49, 0, 43),
        (19.37, 50, 0, 44),
        (19.76, 51, 0, 45),
        (20.16, 52, 0, 46),
        (20.55, 53, 0, 47),
        (20.95, 54, 0, 48),
        (21.34, 55, 0, 49),
        (21.74, 56, 0, 49),
        (22.13, 57, 0, 50),
        (22.53, 58, 0, 51),
        (22.92, 59, 0, 52),
        (23.32, 60, 0, 53),
        (23.72, 61, 0, 54),
        (24.11, 62, 0, 55),
        (24.51, 63, 0, 56),
        (24.90, 64, 0, 56),
        (25.30, 65, 0, 57),
        (25.69, 66, 0, 58),
        (26.09, 67, 0, 59),
        (26.48, 68, 0, 60),
        (26.88, 69, 0, 61),
        (27.27, 70, 0, 62),
        (27.67, 71, 0, 63),
        (28.06, 72, 0, 63),
        (28.46, 73, 0, 64),
        (28.85, 74, 0, 65),
        (29.25, 75, 0, 66),
        (29.64, 76, 0, 67),
        (30.04, 77, 0, 68),
        (30.43, 78, 0, 69),
        (30.83, 79, 0, 70),
        (31.23, 80, 0, 71),
        (31.62, 81, 0, 71),
        (32.02, 82, 0, 72),
        (32.41, 83, 0, 73),
        (32.81, 84, 0, 74),
        (33.20, 85, 0, 75),
        (33.60, 86, 0, 76),
        (33.99, 87, 0, 77),
        (34.39, 88, 0, 78),
        (34.78, 89, 0, 78),
        (35.18, 90, 0, 79),
        (35.57, 91, 0, 80),
        (35.97, 92, 0, 81),
        (36.36, 93, 0, 82),
        (36.76, 94, 0, 83),
        (37.15, 95, 0, 84),
        (37.55, 96, 0, 85),
        (37.94, 97, 0, 85),
        (38.34, 98, 0, 86),
        (38.74, 99, 0, 87),
        (39.13, 100, 0, 88),
        (39.53, 101, 0, 89),
        (39.92, 102, 0, 90),
        (40.32, 103, 0, 91),
        (40.71, 104, 0, 92),
        (41.11, 105, 0, 93),
        (41.50, 106, 0, 93),
        (41.90, 107, 0, 94),
        (42.29, 108, 0, 95),
        (42.69, 109, 0, 96),
        (43.08, 110, 0, 97),
        (43.48, 111, 0, 98),
        (43.87, 112, 0, 99),
        (44.27, 113, 0, 100),
        (44.66, 114, 0, 100),
        (45.06, 115, 0, 101),
        (45.45, 116, 0, 102),
        (45.85, 117, 0, 103),
        (46.25, 118, 0, 104),
        (46.64, 119, 0, 105),
        (47.04, 120, 0, 106),
        (47.43, 121, 0, 107),
        (47.83, 122, 0, 107),
        (48.22, 123, 0, 108),
        (48.62, 124, 0, 109),
        (49.01, 125, 0, 110),
        (49.41, 126, 0, 111),
        (49.80, 127, 0, 112),
        (50.20, 128, 0, 113),
        (50.59, 129, 0, 114),
        (50.99, 130, 0, 115),
        (51.38, 131, 0, 115),
        (51.78, 132, 0, 116),
        (52.17, 133, 0, 117),
        (52.57, 134, 0, 118),
        (52.96, 135, 0, 119),
        (53.36, 136, 0, 120),
        (53.75, 137, 0, 121),
        (54.15, 138, 0, 122),
        (54.55, 139, 0, 122),
        (54.94, 140, 0, 123),
        (55.34, 141, 0, 124),
        (55.73, 142, 0, 125),
        (56.13, 143, 71, 64),
        (56.52, 144, 71, 64),
        (56.92, 145, 71, 65),
        (57.31, 146, 71, 66),
        (57.71, 147, 71, 67),
        (58.10, 148, 71, 68),
        (58.50, 149, 71, 69),
        (58.89, 150, 71, 70),
        (59.29, 151, 71, 71),
        (59.68, 152, 71, 72),
        (60.08, 153, 71, 72),
        (60.47, 154, 71, 73),
        (60.87, 155, 71, 74),
        (61.26, 156, 71, 75),
        (61.66, 157, 71, 76),
        (62.06, 158, 71, 77),
        (62.45, 159, 71, 78),
        (62.85, 160, 71, 79),
        (63.24, 161, 71, 79),
        (63.64, 162, 71, 80),
        (64.03, 163, 71, 81),
        (64.43, 164, 71, 82),
        (64.82, 165, 71, 83),
        (65.22, 166, 71, 84),
        (65.61, 167, 71, 85),
        (66.01, 168, 71, 86),
        (66.40, 169, 71, 86),
        (66.80, 170, 71, 87),
        (67.19, 171, 71, 88),
        (67.59, 172, 71, 89),
        (67.98, 173, 71, 90),
        (68.38, 174, 71, 91),
        (68.77, 175, 71, 92),
        (69.17, 176, 71, 93),
        (69.57, 177, 71, 94),
        (69.96, 178, 71, 94),
        (70.36, 179, 71, 95),
        (70.75, 180, 71, 96),
        (71.15, 181, 71, 97),
        (71.54, 182, 71, 98),
        (71.94, 183, 71, 99),
        (72.33, 184, 71, 100),
        (72.73, 185, 71, 101),
        (73.12, 186, 71, 101),
        (73.52, 187, 71, 102),
        (73.91, 188, 71, 103),
        (74.31, 189, 71, 104),
        (74.70, 190, 71, 105),
        (75.10, 191, 71, 106),
        (75.49, 192, 71, 107),
        (75.89, 193, 71, 108),
        (76.28, 194, 71, 108),
        (76.68, 195, 71, 109),
        (77.08, 196, 71, 110),
        (77.47, 197, 71, 111),
        (77.87, 198, 71, 112),
        (78.26, 199, 71, 113),
        (78.66, 200, 71, 114),
        (79.05, 201, 71, 115),
        (79.45, 202, 71, 116),
        (79.84, 203, 71, 116),
        (80.24, 204, 71, 117),
        (80.63, 205, 71, 118),
        (81.03, 206, 71, 119),
        (81.42, 207, 71, 120),
        (81.82, 208, 71, 121),
        (82.21, 209, 71, 122),
        (82.61, 210, 71, 123),
        (83.00, 211, 71, 123),
        (83.40, 212, 71, 124),
        (83.79, 213, 71, 125),
        (84.19, 214, 71, 126),
        (84.58, 215, 71, 126),
        (84.98, 216, 71, 126),
        (85.38, 217, 71, 126),
        (85.77, 218, 71, 126),
        (86.17, 219, 71, 126),
        (86.56, 220, 71, 126),
        (86.96, 221, 71, 126),
        (87.35, 222, 71, 126),
        (87.75, 223, 71, 126),
        (88.14, 224, 71, 126),
        (88.54, 225, 71, 126),
        (88.93, 226, 71, 126),
        (89.33, 227, 71, 126),
        (89.72, 228, 71, 126),
        (90.12, 229, 71, 126),
        (90.51, 230, 71, 126),
        (90.91, 231, 71, 126),
        (91.30, 232, 71, 126),
        (91.70, 233, 71, 126),
        (92.09, 234, 71, 126),
        (92.49, 235, 71, 126),
        (92.89, 236, 71, 126),
        (93.28, 237, 71, 126),
        (93.68, 238, 71, 126),
        (94.07, 239, 71, 126),
        (94.47, 240, 71, 126),
        (94.86, 241, 71, 126),
        (95.26, 242, 71, 126),
        (95.65, 243, 71, 126),
        (96.05, 244, 71, 126),
        (96.44, 245, 71, 126),
        (96.84, 246, 71, 126),
        (97.23, 247, 71, 126),
        (97.63, 248, 71, 126),
        (98.02, 249, 71, 126),
        (98.42, 250, 71, 126),
        (98.81, 251, 71, 126),
        (99.21, 252, 71, 126),
        (99.60, 253, 71, 126),
        (100.00, 254, 71, 126),
    ];

    #[test]
    fn sunset_stage1_exhaustive() {
        for (initial_percentage, _initial_level, _, target) in SUNSET_STAGE1_MEASUREMENTS {
            let [stage1, _, _] = sunset_stages(initial_percentage);
            assert_eq!((target), stage1);
        }
    }

    // Captured cases: (brightness %, brightness level, preparation drop, stage targets).
    const SUNSET_STAGE3_MEASUREMENTS: [(f64, u8, u8, [u8; 3]); 7] = [
        (0.00, 1, 0, [1, 1, 1]),
        (9.88, 26, 0, [23, 17, 10]),
        (24.90, 64, 0, [56, 43, 24]),
        (50.20, 128, 0, [113, 85, 47]),
        (54.94, 140, 0, [123, 93, 51]),
        (56.13, 143, 71, [64, 48, 27]),
        (100.00, 254, 71, [126, 95, 52]),
    ];

    #[test]
    fn sunset_stage3() {
        for (initial_percentage, _initial_level, _, target) in SUNSET_STAGE3_MEASUREMENTS {
            assert_eq!(target, sunset_stages(initial_percentage));
        }
    }
}
