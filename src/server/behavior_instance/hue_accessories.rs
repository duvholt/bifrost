use std::collections::HashMap;
use std::sync::Arc;

use bifrost_api::backend::BackendRequest;
use chrono::{Local, NaiveTime};
use hue::api::{
    Action, Button, ButtonAction, ButtonConfiguration, ButtonEvent, DimmingDeltaAction,
    DimmingDeltaUpdate, GroupedLightDynamicsUpdate, GroupedLightUpdate,
    HueAccessoriesConfiguration, RType, Room, SceneActive, SceneStatus, SceneUpdate,
    TimeBasedExtendedSlot, configuration,
};
use hue::event::Event;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::ApiError;
use crate::resource::Resources;

pub struct HueAccessoriesJob {
    pub configuration: HueAccessoriesConfiguration,
    pub res: Arc<Mutex<Resources>>,
    next_scene_slots: HashMap<Uuid, usize>,
}

impl HueAccessoriesJob {
    const BRIGHTNESS_DELTA: f64 = 20.0;

    pub fn new(configuration: HueAccessoriesConfiguration, res: Arc<Mutex<Resources>>) -> Self {
        Self {
            configuration,
            res,
            next_scene_slots: HashMap::new(),
        }
    }

    pub async fn create(mut self) {
        let mut hue_events = self.res.lock().await.hue_event_stream().subscribe();

        loop {
            let event = hue_events.recv().await;
            match event {
                Ok(event) => match event.block.event {
                    Event::Add(_add) => {}
                    Event::Update(update) => {
                        for obj in update.data {
                            if RType::Button == obj.rtype
                                && let Some(button_configuration) =
                                    self.configuration.buttons.get(&obj.id)
                            {
                                if let Err(err) = self
                                    .handle_button_update(obj.id, button_configuration.clone())
                                    .await
                                {
                                    log::error!("Error while handling button update {}", err);
                                }
                            }
                        }
                    }
                    Event::Delete(_delete) => {}
                    Event::Error(_error) => {}
                },
                Err(err) => {
                    log::error!("Failed to read event {}", err);
                }
            }
        }
    }

    async fn handle_button_update(
        &mut self,
        rid: Uuid,
        button_configuration: ButtonConfiguration,
    ) -> Result<(), ApiError> {
        let button_update = self.res.lock().await.get_id::<Button>(rid)?.clone();
        let Some(button_report) = button_update.button.button_report else {
            return Ok(());
        };
        let action = match button_report.event {
            ButtonEvent::InitialPress => None,
            ButtonEvent::Repeat => {
                if let Some(repeat_action) = &button_configuration.on_repeat {
                    log::debug!("Repeat button! {repeat_action:?}");
                    Some(repeat_action)
                } else {
                    None
                }
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
            ButtonEvent::LongRelease => None,
            ButtonEvent::DoubleShortRelease => None,
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
    ) -> Result<(), ApiError> {
        let actions: Vec<&Action> = match button_action {
            ButtonAction::TimeBasedExtended(time_based_extended) => {
                // todo: implement WithOff
                let current_time = Local::now().time();

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
            ButtonAction::Action(action) => vec![action].repeat(where_field.len()),
        };
        self.run_action(&actions, where_field).await
    }

    async fn run_action(
        &self,
        actions: &[&Action],
        where_configs: &[configuration::Where],
    ) -> Result<(), ApiError> {
        for (i, action) in actions.iter().enumerate() {
            let where_config = where_configs.get(i);
            match action {
                Action::DoNothing => {}
                Action::HomeOff => {
                    log::warn!("Unimplemented HomeOff action triggered");
                }
                Action::AllOff => {
                    log::warn!("Unimplemented AllOff action triggered");
                }
                Action::LastOn => {
                    log::warn!("Unimplemented LastOn action triggered");
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
                        resource_link.clone(),
                        SceneUpdate::new().with_recall_action(Some(SceneStatus {
                            active: SceneActive::Static,
                            last_recall: None,
                        })),
                    );
                    self.res.lock().await.backend_request(request)?;
                }
            }
        }
        Ok(())
    }

    async fn dim_action(
        &self,
        where_config: Option<&configuration::Where>,
        dimming_delta_action: DimmingDeltaAction,
    ) -> Result<(), ApiError> {
        let Some(where_config) = where_config else {
            return Ok(());
        };
        let room = self
            .res
            .lock()
            .await
            .get::<Room>(&where_config.group)?
            .clone();
        if let Some(grouped_light) = room.grouped_light_service() {
            let request = BackendRequest::GroupedLightUpdate(
                grouped_light.clone(),
                GroupedLightUpdate::new()
                    .with_dimming_delta(Some(DimmingDeltaUpdate::new(
                        dimming_delta_action,
                        Self::BRIGHTNESS_DELTA,
                    )))
                    .with_dynamics(Some(
                        GroupedLightDynamicsUpdate::new().with_duration(Some(1000u32)),
                    )),
            );
            self.res.lock().await.backend_request(request)?;
        }
        Ok(())
    }
}

fn find_current_time_slot(
    slots: &[TimeBasedExtendedSlot],
    current_time: NaiveTime,
) -> Option<&TimeBasedExtendedSlot> {
    slots
        .windows(2)
        .find_map(|slots| match slots {
            [slot1, slot2] => {
                let time1 =
                    NaiveTime::from_hms_opt(slot1.start_time.hour, slot1.start_time.minute, 0)?;
                let time2 =
                    NaiveTime::from_hms_opt(slot2.start_time.hour, slot2.start_time.minute, 0)?;
                if time1 <= current_time && current_time < time2 {
                    Some(slot1)
                } else {
                    None
                }
            }
            _ => None,
        })
        .or(slots.last())
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
