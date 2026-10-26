use std::collections::HashMap;
use std::sync::Arc;

use bifrost_api::backend::BackendRequest;
use chrono::NaiveTime;
use hue::api::{
    Action, BehaviorInstance, BridgeHome, Button, ButtonAction, ButtonConfiguration, ButtonEvent,
    DimmingDeltaAction, DimmingDeltaUpdate, Group, GroupedLightDynamicsUpdate, GroupedLightUpdate,
    HueAccessoriesConfiguration, On, RType, Resource, ResourceLink, SceneActive, SceneStatus,
    SceneUpdate, TimeBasedExtendedSlot, configuration,
};
use hue::event::Event;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::server::behavior_instance::{BehaviorCommand, BehaviorContext};
use crate::server::hueevents::HueEventRecord;

#[derive(Debug)]
pub struct HueAccessoriesBehavior {
    enabled: bool,
    configuration: HueAccessoriesConfiguration,
    context: Arc<BehaviorContext>,
    commands: mpsc::Receiver<BehaviorCommand>,
    next_scene_slots: HashMap<Uuid, usize>,
}

impl HueAccessoriesBehavior {
    const BRIGHTNESS_DELTA: f64 = 20.0;

    pub fn new(
        instance: &BehaviorInstance,
        context: Arc<BehaviorContext>,
        commands: mpsc::Receiver<BehaviorCommand>,
    ) -> ApiResult<Self> {
        let configuration = parse_config(instance)?;
        Ok(Self {
            enabled: instance.enabled,
            configuration,
            context,
            commands,
            next_scene_slots: HashMap::new(),
        })
    }

    pub async fn run(mut self) {
        let mut hue_events = self.context.res.lock().await.hue_event_stream().subscribe();
        loop {
            tokio::select! {
                command = self.commands.recv() => {
                    match command {
                        Some(BehaviorCommand::Update(instance)) => {
                            self.handle_update(&instance);
                        }
                        Some(BehaviorCommand::Trigger(_)) => {}
                        Some(BehaviorCommand::Shutdown) | None => break,
                    }
                },
                event = hue_events.recv() => {
                    if self.enabled {
                        self.handle_hue_event(event).await;
                    }
                }
            }
        }
    }

    fn handle_update(&mut self, instance: &BehaviorInstance) {
        self.enabled = instance.enabled;
        match parse_config(instance) {
            Ok(configuration) => {
                self.configuration = configuration;
                self.next_scene_slots = HashMap::new();
            }
            Err(err) => {
                log::error!("Failed to parse Hue accessories {err}. Keeping previous config");
            }
        }
    }

    async fn handle_hue_event(
        &mut self,
        event: Result<HueEventRecord, broadcast::error::RecvError>,
    ) {
        match event {
            Ok(event) => match event.block.event {
                Event::Add(_add) => {}
                Event::Update(update) => {
                    for obj in update.data {
                        if RType::Button == obj.rtype
                            && let Some(button_configuration) =
                                self.configuration.buttons.get(&obj.id)
                            && let Err(err) = self
                                .handle_button_update(obj.id, button_configuration.clone())
                                .await
                        {
                            log::error!("Error while handling button update {err}");
                        }
                    }
                }
                Event::Delete(_delete) => {}
                Event::Error(_error) => {}
            },
            Err(err) => {
                log::error!("Failed to read event {err}");
            }
        }
    }

    async fn handle_button_update(
        &mut self,
        rid: Uuid,
        button_configuration: ButtonConfiguration,
    ) -> ApiResult<()> {
        let button_update = self.context.res.lock().await.get_id::<Button>(rid)?.clone();
        let Some(button_report) = button_update.button.button_report else {
            return Ok(());
        };
        let action = match button_report.event {
            ButtonEvent::Repeat => {
                button_configuration
                    .on_repeat
                    .as_ref()
                    .inspect(|&repeat_action| {
                        log::debug!("Repeat button! {repeat_action:?}");
                    })
            }
            ButtonEvent::ShortRelease => {
                if let Some(short_release_action) = &button_configuration.on_short_release {
                    log::debug!("Short release button! {short_release_action:?}");
                    Some(short_release_action)
                } else if let Some(repeat_action) = &button_configuration.on_repeat {
                    log::debug!("Short release  repeat button! {repeat_action:?}");
                    Some(repeat_action)
                } else {
                    None
                }
            }
            ButtonEvent::LongPress => {
                if let Some(long_press_action) = &button_configuration.on_long_press {
                    log::debug!("Long pressed button! {long_press_action:?}");
                    Some(long_press_action)
                } else if let Some(repeat_action) = &button_configuration.on_repeat {
                    log::debug!("Long press repeat button! {repeat_action:?}");
                    Some(repeat_action)
                } else {
                    None
                }
            }
            ButtonEvent::InitialPress
            | ButtonEvent::LongRelease
            | ButtonEvent::DoubleShortRelease => None,
        };

        if let Some(action) = action {
            self.handle_button_action(rid, action, &button_configuration.where_field)
                .await?;
        }
        Ok(())
    }

    async fn handle_button_action(
        &mut self,
        rid: Uuid,
        button_action: &ButtonAction,
        where_field: &[configuration::Where],
    ) -> ApiResult<()> {
        let actions: Vec<&Action> = match button_action {
            ButtonAction::TimeBasedExtended(time_based_extended) => {
                // todo: implement WithOff
                let current_time = self.context.clock.local_now().time();

                let Some(current_slot) =
                    find_current_time_slot(&time_based_extended.slots, current_time)
                else {
                    log::warn!("No time slot found for time based action: {time_based_extended:?}");
                    return Ok(());
                };
                current_slot.actions.iter().map(|aw| &aw.action).collect()
            }
            ButtonAction::RecallSingleExtended(recall_single_extended) => {
                // todo: implement WithOff
                recall_single_extended
                    .actions
                    .iter()
                    .map(|aw| &aw.action)
                    .collect()
            }
            ButtonAction::SceneCycleExtended(scene_cycle_extended) => {
                // todo: implement WithOff, repeat_timeout
                let scene_slot = self.next_scene_slots.entry(rid).or_insert(0);
                let max_scene_slots = scene_cycle_extended.slots.len();
                let actions = &scene_cycle_extended.slots[*scene_slot % max_scene_slots];
                *scene_slot = (*scene_slot + 1) % max_scene_slots;

                actions.iter().map(|aw| &aw.action).collect()
            }
            ButtonAction::Action(action) => [action].repeat(where_field.len()),
        };
        self.run_action(&actions, where_field).await
    }

    async fn run_action(
        &self,
        actions: &[&Action],
        where_configs: &[configuration::Where],
    ) -> ApiResult<()> {
        for (i, action) in actions.iter().enumerate() {
            let where_config = where_configs.get(i);
            match action {
                Action::DoNothing => {}
                Action::HomeOff => {
                    let lock = self.context.res.lock().await;
                    for bridge_home in lock.get_resources_by_type(RType::BridgeHome) {
                        let bridge_home: BridgeHome = bridge_home.obj.try_into()?;
                        if let Some(grouped_light_link) = bridge_home.grouped_light_service() {
                            let request = BackendRequest::GroupedLightUpdate(
                                *grouped_light_link,
                                GroupedLightUpdate::new().with_on(On::new(false)),
                            );
                            lock.backend_request(request)?;
                        }
                    }
                    drop(lock);
                }
                Action::AllOff => {
                    if let Some(grouped_light_link) =
                        self.get_grouped_light_link(where_config).await?
                    {
                        let request = BackendRequest::GroupedLightUpdate(
                            grouped_light_link,
                            GroupedLightUpdate::new().with_on(On::new(false)),
                        );
                        self.context.res.lock().await.backend_request(request)?;
                    }
                }
                Action::LastOn => {
                    if let Some(grouped_light_link) =
                        self.get_grouped_light_link(where_config).await?
                    {
                        let request = BackendRequest::GroupedLightUpdate(
                            grouped_light_link,
                            GroupedLightUpdate::new().with_on(On::new(true)),
                        );
                        self.context.res.lock().await.backend_request(request)?;
                    }
                }
                Action::DimDown => {
                    self.dim_action(where_config, DimmingDeltaAction::Down)
                        .await?;
                }
                Action::DimUp => {
                    self.dim_action(where_config, DimmingDeltaAction::Up)
                        .await?;
                }
                Action::DimAlternate => {
                    log::warn!("Unimplemented DimAlternate action triggered");
                }
                Action::Recall(resource_link) => {
                    let request = BackendRequest::SceneUpdate(
                        *resource_link,
                        SceneUpdate::new().with_recall_action(Some(SceneStatus {
                            active: SceneActive::Static,
                            last_recall: None,
                        })),
                    );
                    self.context.res.lock().await.backend_request(request)?;
                }
            }
        }
        Ok(())
    }

    async fn dim_action(
        &self,
        where_config: Option<&configuration::Where>,
        dimming_delta_action: DimmingDeltaAction,
    ) -> ApiResult<()> {
        if let Some(grouped_light) = self.get_grouped_light_link(where_config).await? {
            let request = BackendRequest::GroupedLightUpdate(
                grouped_light,
                GroupedLightUpdate::new()
                    .with_dimming_delta(Some(DimmingDeltaUpdate::new(
                        dimming_delta_action,
                        Self::BRIGHTNESS_DELTA,
                    )))
                    .with_dynamics(Some(
                        GroupedLightDynamicsUpdate::new().with_duration(Some(1000u32)),
                    )),
            );
            self.context.res.lock().await.backend_request(request)?;
        }
        Ok(())
    }

    async fn get_grouped_light_link(
        &self,
        where_config: Option<&configuration::Where>,
    ) -> Result<Option<ResourceLink>, ApiError> {
        let Some(where_config) = where_config else {
            return Ok(None);
        };
        let resource = self
            .context
            .res
            .lock()
            .await
            .get_resource(&where_config.group)?;
        match resource.obj {
            Resource::Room(room) => Ok(room.grouped_light_service().copied()),
            Resource::Zone(zone) => Ok(zone.grouped_light_service().copied()),
            _ => Ok(None),
        }
    }
}

fn parse_config(instance: &BehaviorInstance) -> ApiResult<HueAccessoriesConfiguration> {
    Ok(serde_json::from_value::<HueAccessoriesConfiguration>(
        instance.configuration.clone(),
    )?)
}

fn find_current_time_slot(
    slots: &[TimeBasedExtendedSlot],
    current_time: NaiveTime,
) -> Option<&TimeBasedExtendedSlot> {
    slots
        .windows(2)
        .find_map(|slots| match slots {
            [slot_1, slot_2] => {
                let time1 =
                    NaiveTime::from_hms_opt(slot_1.start_time.hour, slot_1.start_time.minute, 0)?;
                let time2 =
                    NaiveTime::from_hms_opt(slot_2.start_time.hour, slot_2.start_time.minute, 0)?;
                if time1 <= current_time && current_time < time2 {
                    Some(slot_1)
                } else {
                    None
                }
            }
            _ => None,
        })
        .or_else(|| slots.last())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveTime;
    use hue::api::{TimeBasedExtendedSlot, configuration::Time};

    use crate::server::behavior_instance::hue_accessories::find_current_time_slot;

    fn create_slots() -> Vec<TimeBasedExtendedSlot> {
        vec![
            TimeBasedExtendedSlot {
                actions: vec![],
                start_time: Time { hour: 7, minute: 0 },
            },
            TimeBasedExtendedSlot {
                actions: vec![],
                start_time: Time {
                    hour: 10,
                    minute: 10,
                },
            },
            TimeBasedExtendedSlot {
                actions: vec![],
                start_time: Time {
                    hour: 20,
                    minute: 0,
                },
            },
        ]
    }

    #[test]
    fn find_current_time_slot_single_slot() {
        let slots = vec![TimeBasedExtendedSlot {
            actions: vec![],
            start_time: Time {
                hour: 10,
                minute: 10,
            },
        }];
        let current_time = NaiveTime::from_hms_opt(15, 0, 0).unwrap();

        let slot = find_current_time_slot(&slots, current_time).unwrap();
        assert_eq!(slot.start_time, slots[0].start_time);
    }

    #[test]
    fn find_current_time_slot_middle() {
        let slots = create_slots();

        let current_time = NaiveTime::from_hms_opt(15, 0, 0).unwrap();

        let slot = find_current_time_slot(&slots, current_time).unwrap();
        assert_eq!(slot.start_time, slots[1].start_time);
    }

    #[test]
    fn find_current_time_slot_after_last() {
        let slots = create_slots();

        let current_time = NaiveTime::from_hms_opt(22, 0, 0).unwrap();

        let slot = find_current_time_slot(&slots, current_time).unwrap();
        assert_eq!(slot.start_time, slots[2].start_time);
    }

    #[test]
    fn find_current_time_slot_before_first() {
        let slots = create_slots();

        let current_time = NaiveTime::from_hms_opt(2, 0, 0).unwrap();

        let slot = find_current_time_slot(&slots, current_time).unwrap();
        assert_eq!(slot.start_time, slots[2].start_time);
    }

    #[test]
    fn find_current_time_slot_exactly_second() {
        let slots = create_slots();

        let current_time = NaiveTime::from_hms_opt(10, 10, 0).unwrap();

        let slot = find_current_time_slot(&slots, current_time).unwrap();
        assert_eq!(slot.start_time, slots[1].start_time);
    }

    #[test]
    fn find_current_time_slot_right_before_second() {
        let slots = create_slots();

        let current_time = NaiveTime::from_hms_opt(10, 9, 0).unwrap();

        let slot = find_current_time_slot(&slots, current_time).unwrap();
        assert_eq!(slot.start_time, slots[0].start_time);
    }
}
