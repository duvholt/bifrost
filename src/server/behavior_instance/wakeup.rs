use std::sync::Arc;

use bifrost_api::backend::BackendRequest;
use chrono::NaiveTime;
use tokio::sync::{Mutex, mpsc};

use hue::api::{
    BehaviorInstance, GroupedLightDynamicsUpdate, GroupedLightUpdate, LightDynamicsUpdate,
    LightTimedEffect, LightTimedEffectsUpdate, LightUpdate, On, WakeupConfiguration, WakeupStyle,
};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::ApiError;
use crate::server::behavior_instance::schedule::{Schedule, wait_for_schedule};
use crate::server::behavior_instance::service::{ScheduleType, disable_behavior_instance};
use crate::server::behavior_instance::target::{
    ExecutionTarget, ResolvedLight, SelectionPolicy, TargetResource, resolve_resources,
    select_targets,
};
use crate::server::behavior_instance::{BehaviorCommand, BehaviorContext, BehaviorTriggerAction};
use crate::{error::ApiResult, resource::Resources};

#[derive(Debug)]
pub struct WakeupBehavior {
    id: Uuid,
    instance: BehaviorInstance,
    configuration: WakeupConfiguration,
    context: Arc<BehaviorContext>,
    commands: mpsc::Receiver<BehaviorCommand>,
    execution: Option<JoinHandle<ApiResult<()>>>,
}

impl Drop for WakeupBehavior {
    fn drop(&mut self) {
        self.stop_execution();
    }
}

impl WakeupBehavior {
    pub fn new(
        id: Uuid,
        instance: BehaviorInstance,
        context: Arc<BehaviorContext>,
        commands: mpsc::Receiver<BehaviorCommand>,
    ) -> ApiResult<Self> {
        let config = parse_config(&instance)?;
        Ok(Self {
            id,
            instance,
            configuration: config,
            context,
            commands,
            execution: None,
        })
    }

    pub async fn run(mut self) {
        loop {
            let schedule = self.schedule_if_idle();

            tokio::select! {
                command = self.commands.recv() => {
                    match command {
                        Some(BehaviorCommand::Update(instance)) => {
                            self.handle_update(*instance);
                        }
                        Some(BehaviorCommand::Trigger(trigger)) => {
                            self.handle_trigger(trigger).await;
                        }
                        Some(BehaviorCommand::Shutdown) | None => break,
                    }
                }

                () = wait_for_schedule(schedule, self.context.clock.clone())  => {
                    self.handle_trigger(BehaviorTriggerAction::ScheduledStart).await;
                }

                result = wait_for_execution(&mut self.execution) => {
                    self.execution_finished(result).await;
                }
            }
        }
    }

    fn schedule_if_idle(&self) -> Option<Schedule> {
        if !self.instance.enabled || self.execution.is_some() {
            return None;
        }

        self.schedule()
            .inspect_err(|err| {
                log::error!("Failed to create wakeup schedule {}: {err}", self.id);
            })
            .ok()
    }

    fn handle_update(&mut self, instance: BehaviorInstance) {
        let config = match parse_config(&instance) {
            Ok(config) => config,
            Err(err) => {
                log::error!("Failed to parse wakeup config {instance:?}: {err}");
                return;
            }
        };
        if self.configuration != config {
            self.configuration = config;
            self.stop_execution();
        }
        self.instance = instance;
    }

    async fn execution_finished(&mut self, result: ApiResult<()>) {
        self.execution = None;

        match result {
            Ok(()) => {}
            Err(ApiError::JoinError(err)) if err.is_cancelled() => {
                return;
            }
            Err(err) => {
                log::error!("Wakeup {} task failed: {err}", self.id);
                return;
            }
        }

        let Ok(schedule) = self.schedule() else {
            return;
        };
        match schedule.recurrence {
            ScheduleType::Recurring(_) => {}
            ScheduleType::Once() => {
                disable_behavior_instance(self.id, self.context.res.clone()).await;
            }
        }
    }

    async fn handle_trigger(&mut self, trigger: BehaviorTriggerAction) {
        match trigger {
            BehaviorTriggerAction::ScheduledStart | BehaviorTriggerAction::ManualStart => {
                self.start_execution().await;
            }
            BehaviorTriggerAction::Stop => {
                self.stop_execution();
            }
        }
    }

    fn stop_execution(&mut self) {
        if let Some(execution) = self.execution.take() {
            execution.abort();
        }
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

        let fade_in_duration = self.configuration.fade_in_duration.to_std();

        Ok(Schedule {
            recurrence: schedule_type,
            scheduled_time: scheduled_wakeup_time,
            lead_time: fade_in_duration,
        })
    }

    async fn start_execution(&mut self) {
        if self.execution.is_some() {
            return;
        }
        let config = self.configuration.clone();
        let ctx = self.context.clone();
        log::debug!("Running scheduled behavior instance:, {config:#?}");
        let lock = ctx.res.lock().await;
        let resolved_resources = resolve_resources(&lock, &config.where_field);
        drop(lock);
        let targets = select_targets(&resolved_resources, self);

        let handle = tokio::spawn(async move { run_wakeup(ctx, config, targets).await });
        self.execution = Some(handle);
    }
}

async fn run_wakeup(
    ctx: Arc<BehaviorContext>,
    config: WakeupConfiguration,
    targets: Vec<ExecutionTarget<WakeupStyle>>,
) -> ApiResult<()> {
    for target in &targets {
        target.on(&ctx.res, &config).await.inspect_err(|err| {
            log::warn!("Failed to turn on wake up light: {err}");
        })?;
    }

    // wait until fade in has completed
    // otherwise the behavior instance can be disabled before it has actually finished
    ctx.clock.sleep(config.fade_in_duration.to_std()).await;

    if let Some(duration) = &config.turn_lights_off_after {
        ctx.clock.sleep(duration.to_std()).await;

        for target in &targets {
            target.off(&ctx.res).await.inspect_err(|err| {
                log::warn!("Failed to turn off wake up light: {err}");
            })?;
        }
    }
    Ok(())
}

async fn wait_for_execution(execution: &mut Option<JoinHandle<ApiResult<()>>>) -> ApiResult<()> {
    match execution {
        Some(task) => task.await?,
        None => std::future::pending().await,
    }
}

fn parse_config(instance: &BehaviorInstance) -> ApiResult<WakeupConfiguration> {
    Ok(serde_json::from_value::<WakeupConfiguration>(
        instance.configuration.clone(),
    )?)
}

impl SelectionPolicy for WakeupBehavior {
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
    async fn on(&self, res: &Mutex<Resources>, config: &WakeupConfiguration) -> ApiResult<()> {
        match self.effect {
            WakeupStyle::Sunrise => self.sunrise_on(res, config).await,
            WakeupStyle::Basic => self.basic_on(res, config).await,
        }
    }

    async fn basic_on(
        &self,
        res: &Mutex<Resources>,
        config: &WakeupConfiguration,
    ) -> ApiResult<()> {
        // As reported by the Hue bridge
        const WAKEUP_FADE_MIREK: u16 = 447;

        let initial_backend_requests = match self.resource {
            TargetResource::Light {
                link: resource_link,
                ..
            } => {
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
        let fade_request = match self.resource {
            TargetResource::Light {
                link: resource_link,
                ..
            } => {
                let brightness = LightUpdate::default()
                    .with_brightness(Some(config.end_brightness))
                    .with_dynamics(Some(
                        LightDynamicsUpdate::new()
                            .with_duration(Some((config.fade_in_duration.seconds) * 1000)),
                    ));
                BackendRequest::LightUpdate(resource_link, brightness)
            }
            TargetResource::GroupedLight(resource_link) => {
                let brightness = GroupedLightUpdate::default()
                    .with_brightness(Some(config.end_brightness))
                    .with_dynamics(Some(
                        GroupedLightDynamicsUpdate::new()
                            .with_duration(Some((config.fade_in_duration.seconds) * 1000)),
                    ));
                BackendRequest::GroupedLightUpdate(resource_link, brightness)
            }
        };
        res.lock().await.backend_request(fade_request)?;

        Ok(())
    }

    async fn sunrise_on(
        &self,
        res: &Mutex<Resources>,
        config: &WakeupConfiguration,
    ) -> ApiResult<()> {
        match self.resource {
            TargetResource::Light {
                link: resource_link,
                ..
            } => {
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
            TargetResource::GroupedLight(_resource_link) => {
                // The sunrise effect does not support grouped light requests
            }
        }
        Ok(())
    }

    async fn off(&self, res: &Mutex<Resources>) -> ApiResult<()> {
        let backend_request = match self.resource {
            TargetResource::Light {
                link: resource_link,
                ..
            } => {
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
