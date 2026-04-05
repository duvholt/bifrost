use std::sync::Arc;

use hue::api::{Button, ButtonConfiguration, ButtonEvent, HueAccessoriesConfiguration, RType};
use hue::event::Event;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::resource::Resources;

pub struct HueAccessoriesJob {
    pub configuration: HueAccessoriesConfiguration,
    pub res: Arc<Mutex<Resources>>,
}
impl HueAccessoriesJob {
    pub async fn create(self) {
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
                                self.handle_button_update(obj.id, button_configuration.clone())
                                    .await;
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

    async fn handle_button_update(&self, rid: Uuid, button_configuration: ButtonConfiguration) {
        let button_update = match self.res.lock().await.get_id::<Button>(rid).cloned() {
            Ok(button_update) => button_update,
            Err(err) => {
                log::error!("Failed to get button {}: {:?}", rid, err);
                return;
            }
        };
        let Some(button_report) = button_update.button.button_report else {
            return;
        };
        match button_report.event {
            ButtonEvent::InitialPress => {}
            ButtonEvent::Repeat => {
                if let Some(repeat_action) = &button_configuration.on_repeat {
                    log::info!("Repeat button! {repeat_action:?}");
                }
            }
            ButtonEvent::ShortRelease => {
                if let Some(short_release_action) = &button_configuration.on_short_release {
                    log::info!("Short release button! {short_release_action:?}");
                }
                if let Some(repeat_action) = &button_configuration.on_repeat {
                    log::info!("Short release  repeat button! {repeat_action:?}");
                }
            }
            ButtonEvent::LongRelease => {}
            ButtonEvent::DoubleShortRelease => {}
            ButtonEvent::LongPress => {
                if let Some(long_press_action) = &button_configuration.on_long_press {
                    log::info!("Long pressed button! {long_press_action:?}");
                }
                if let Some(repeat_action) = &button_configuration.on_repeat {
                    log::info!("Long press repeat button! {repeat_action:?}");
                }
            }
        }
    }
}
