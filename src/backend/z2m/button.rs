use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, TimeDelta, Utc};
use hue::api::{
    Button, ButtonData, ButtonDataUpdate, ButtonEvent, ButtonMetadata, ButtonReport, ButtonUpdate,
    Device, ResourceLink,
};
use tokio::{sync::Mutex, task::JoinHandle, time::sleep};

use crate::{error::ApiResult, resource::Resources};

pub struct Z2mButtonHandler {
    data: Z2mButtonData,
    res: Arc<Mutex<Resources>>,
    prev_button_press: Arc<Mutex<Option<DateTime<Utc>>>>,
    button_repeat_task: Option<JoinHandle<()>>,
}

impl Z2mButtonHandler {
    pub fn from_model_id(res: Arc<Mutex<Resources>>, model_id: &str) -> Option<Self> {
        let button_data = Z2mButtonData::from_model_id(model_id);
        button_data.map(|data| Self {
            data,
            res,
            prev_button_press: Arc::new(Mutex::new(None)),
            button_repeat_task: None,
        })
    }

    pub async fn handle_action(&mut self, device: &Device, action: &str) -> ApiResult<()> {
        if let Some(button_repeat_task) = &self.button_repeat_task {
            button_repeat_task.abort();
            self.button_repeat_task = None;
        }
        let Some(button_controller_id) = self.data.get_controller_id(action) else {
            log::warn!("Unknown button pressed {}", action);
            return Ok(());
        };

        let lock = self.res.lock().await;
        let Some((button_link, button_controller)) =
            device.button_services().into_iter().find_map(|link| {
                if let Some(button) = lock.get::<Button>(link).ok() {
                    if button.metadata.control_id == button_controller_id {
                        return Some((link.clone(), button.clone()));
                    }
                }
                return None;
            })
        else {
            log::error!(
                "Unable to find button controller for {} with controller id {}",
                device.metadata.name,
                button_controller_id
            );
            return Ok(());
        };
        drop(lock);

        let Some(button_event) = self
            .data
            .next_button_event(&button_controller.button, action)
        else {
            return Ok(());
        };
        log::trace!(
            "Recevied button action {} {} {:?}",
            button_controller.metadata.control_id,
            device.metadata.name,
            button_event
        );

        if self.data.required_longpress_workaround {
            // The Friends of Hue switch (e.g. EnOcean PTM 215Z) only sends press and release events
            // This is an attempt to emulate the Hue dimmer switch behavior using only these two actions
            // It's worth noting that the real bridge doesn't send button state updates for the FoH switch,
            // but it's easier in Bifrost to do it properly, so we do it anyway.
            match button_event {
                ButtonEvent::InitialPress => {
                    log::trace!("Button long press workaround: setting last button press");
                    *self.prev_button_press.lock().await = Some(Utc::now());
                    Self::update_button(self.res.clone(), button_link, ButtonEvent::InitialPress)
                        .await?;
                    let prev_button_press = self.prev_button_press.clone();
                    let res = self.res.clone();
                    self.button_repeat_task = Some(tokio::spawn(Self::send_fake_repeat(
                        res,
                        button_link,
                        prev_button_press,
                    )));
                }
                ButtonEvent::ShortRelease => {
                    let prev_button_press = *self.prev_button_press.lock().await;
                    log::trace!(
                        "Button long press workaround: last button pressed {:?}",
                        prev_button_press
                    );
                    if let Some(prev_short_press) = prev_button_press {
                        let event = if (Utc::now() - prev_short_press) > TimeDelta::seconds(1) {
                            ButtonEvent::LongRelease
                        } else {
                            ButtonEvent::ShortRelease
                        };
                        *self.prev_button_press.lock().await = None;
                        Self::update_button(self.res.clone(), button_link, event).await?;
                    }
                }
                ButtonEvent::Repeat
                | ButtonEvent::LongRelease
                | ButtonEvent::DoubleShortRelease
                | ButtonEvent::LongPress => {
                    *self.prev_button_press.lock().await = None;
                    // These events are not possible for FOHSWITCH
                    Self::update_button(self.res.clone(), button_link, button_event).await?;
                }
            }
        } else {
            Self::update_button(self.res.clone(), button_link, button_event).await?;
        }

        Ok(())
    }

    async fn send_fake_repeat(
        res: Arc<Mutex<Resources>>,
        button_link: ResourceLink,
        prev_button_press: Arc<Mutex<Option<DateTime<Utc>>>>,
    ) {
        // Send up to 10 repeat events
        for i in 1..10 {
            sleep(Duration::from_secs(1)).await;
            match *prev_button_press.lock().await {
                Some(timestamp) => {
                    if (Utc::now() + TimeDelta::seconds(i)) < timestamp {
                        // Button has likely been pressed again
                        return;
                    }
                }
                None => {
                    // Button already released
                    return;
                }
            }
            let event = if i == 1 {
                ButtonEvent::LongPress
            } else {
                ButtonEvent::Repeat
            };
            if let Err(err) = Self::update_button(res.clone(), button_link, event).await {
                log::error!("Failed to update button state {err}");
            }
        }
        log::debug!("Timed out waiting for button release");
        if let Err(err) =
            Self::update_button(res.clone(), button_link, ButtonEvent::LongRelease).await
        {
            log::error!("Failed to update button state {err}");
        }
    }

    async fn update_button(
        res: Arc<Mutex<Resources>>,
        button_link: ResourceLink,
        button_event: ButtonEvent,
    ) -> ApiResult<()> {
        log::debug!("Setting button state to {button_event:?} for {button_link:?}");
        // The actual handling of button events is done in the hue accessories behavior instance which listens for button updates
        res.lock()
            .await
            .update::<Button>(&button_link.rid, |button| {
                *button += ButtonUpdate::new().with_button(
                    ButtonDataUpdate::new()
                        .with_button_report(ButtonReport {
                            updated: Utc::now(),
                            event: button_event,
                        })
                        .with_last_event(button_event),
                );
            })?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Z2mButtonData {
    pub buttons: Vec<Z2mButton>,
    mappings: HashMap<&'static str, Z2mButtonMapping>,
    required_longpress_workaround: bool,
}

#[derive(Debug)]
pub struct Z2mButton {
    pub name: String,
    pub metadata: ButtonMetadata,
    pub data: ButtonData,
}

#[derive(Debug, Clone)]
pub struct Z2mButtonMapping {
    pub control_id: u32,
    pub action: ButtonEvent,
}

impl Z2mButtonData {
    pub fn from_model_id(model_id: &str) -> Option<Self> {
        match model_id {
            "RWL021" | "RWL022" => Some(hue_dimmer_switch()),
            "GreenPower_2" => Some(friends_of_hue_switch()),
            _ => None,
        }
    }

    pub fn get_controller_id(&self, action: &str) -> Option<u32> {
        self.mappings.get(&action).map(|m| m.control_id)
    }

    fn next_button_event(&mut self, button_data: &ButtonData, action: &str) -> Option<ButtonEvent> {
        let mapped_button_event = self.mappings.get(&action).cloned()?.action;
        let Some(current_button_report) = &button_data.button_report else {
            return Some(mapped_button_event);
        };
        Some(match mapped_button_event {
            ButtonEvent::LongPress => match current_button_report.event {
                ButtonEvent::LongPress | ButtonEvent::Repeat => ButtonEvent::Repeat,
                _ => mapped_button_event,
            },
            _ => mapped_button_event,
        })
    }
}

fn friends_of_hue_switch() -> Z2mButtonData {
    let events = vec![ButtonEvent::InitialPress, ButtonEvent::ShortRelease];
    Z2mButtonData {
        required_longpress_workaround: true,
        buttons: vec![
            Z2mButton {
                name: "1".to_string(),
                data: ButtonData {
                    button_report: None,
                    last_event: None,
                    repeat_interval: Some(0),
                    event_values: Some(events.clone()),
                },
                metadata: ButtonMetadata { control_id: 1 },
            },
            Z2mButton {
                name: "2".to_string(),
                data: ButtonData {
                    button_report: None,
                    last_event: None,
                    repeat_interval: Some(0),
                    event_values: Some(events.clone()),
                },
                metadata: ButtonMetadata { control_id: 2 },
            },
            Z2mButton {
                name: "3".to_string(),
                data: ButtonData {
                    button_report: None,
                    last_event: None,
                    repeat_interval: Some(0),
                    event_values: Some(events.clone()),
                },
                metadata: ButtonMetadata { control_id: 3 },
            },
            Z2mButton {
                name: "4".to_string(),
                data: ButtonData {
                    button_report: None,
                    last_event: None,
                    repeat_interval: Some(0),
                    event_values: Some(events.clone()),
                },
                metadata: ButtonMetadata { control_id: 4 },
            },
        ],
        mappings: maplit::hashmap! {
            "press_1" => Z2mButtonMapping { control_id: 1, action: ButtonEvent::InitialPress},
            "release_1" => Z2mButtonMapping { control_id: 1, action: ButtonEvent::ShortRelease},

            "press_2" => Z2mButtonMapping { control_id: 2, action: ButtonEvent::InitialPress},
            "release_2" => Z2mButtonMapping { control_id: 2, action: ButtonEvent::ShortRelease},

            "press_3" => Z2mButtonMapping { control_id: 3, action: ButtonEvent::InitialPress},
            "release_3" => Z2mButtonMapping { control_id: 3, action: ButtonEvent::ShortRelease},

            "press_4" => Z2mButtonMapping { control_id: 4, action: ButtonEvent::InitialPress},
            "release_4" => Z2mButtonMapping { control_id: 4, action: ButtonEvent::ShortRelease},
        },
    }
}

fn hue_dimmer_switch() -> Z2mButtonData {
    let events = vec![
        ButtonEvent::InitialPress,
        ButtonEvent::Repeat,
        ButtonEvent::ShortRelease,
        ButtonEvent::LongRelease,
        ButtonEvent::LongPress,
    ];
    Z2mButtonData {
        required_longpress_workaround: false,
        buttons: vec![
            Z2mButton {
                name: "on".to_string(),
                data: ButtonData {
                    button_report: None,
                    last_event: None,
                    repeat_interval: Some(800),
                    event_values: Some(events.clone()),
                },
                metadata: ButtonMetadata { control_id: 1 },
            },
            Z2mButton {
                name: "up".to_string(),
                data: ButtonData {
                    button_report: None,
                    last_event: None,
                    repeat_interval: Some(800),
                    event_values: Some(events.clone()),
                },
                metadata: ButtonMetadata { control_id: 2 },
            },
            Z2mButton {
                name: "down".to_string(),
                data: ButtonData {
                    button_report: None,
                    last_event: None,
                    repeat_interval: Some(800),
                    event_values: Some(events.clone()),
                },
                metadata: ButtonMetadata { control_id: 3 },
            },
            Z2mButton {
                name: "off".to_string(),
                data: ButtonData {
                    button_report: None,
                    last_event: None,
                    repeat_interval: Some(800),
                    event_values: Some(events.clone()),
                },
                metadata: ButtonMetadata { control_id: 4 },
            },
        ],
        mappings: maplit::hashmap! {
            "on_press" => Z2mButtonMapping { control_id: 1, action: ButtonEvent::InitialPress},
            "on_hold" => Z2mButtonMapping { control_id: 1, action: ButtonEvent::LongPress},
            "on_press_release" => Z2mButtonMapping { control_id: 1, action: ButtonEvent::ShortRelease},
            "on_hold_release" => Z2mButtonMapping { control_id: 1, action: ButtonEvent::LongRelease},

            "up_press" => Z2mButtonMapping { control_id: 2, action: ButtonEvent::InitialPress},
            "up_hold" => Z2mButtonMapping { control_id: 2, action: ButtonEvent::LongPress},
            "up_press_release" => Z2mButtonMapping { control_id: 2, action: ButtonEvent::ShortRelease},
            "up_hold_release" => Z2mButtonMapping { control_id: 2, action: ButtonEvent::LongRelease},

            "down_press" => Z2mButtonMapping { control_id: 3, action: ButtonEvent::InitialPress},
            "down_hold" => Z2mButtonMapping { control_id: 3, action: ButtonEvent::LongPress},
            "down_press_release" => Z2mButtonMapping { control_id: 3, action: ButtonEvent::ShortRelease},
            "down_hold_release" => Z2mButtonMapping { control_id: 3, action: ButtonEvent::LongRelease},

            "off_press" => Z2mButtonMapping { control_id: 4, action: ButtonEvent::InitialPress},
            "off_hold" => Z2mButtonMapping { control_id: 4, action: ButtonEvent::LongPress},
            "off_press_release" => Z2mButtonMapping { control_id: 4, action: ButtonEvent::ShortRelease},
            "off_hold_release" => Z2mButtonMapping { control_id: 4, action: ButtonEvent::LongRelease},
        },
    }
}
