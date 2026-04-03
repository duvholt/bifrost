use std::collections::HashMap;

use hue::api::{ButtonData, ButtonEvent, ButtonMetadata};

#[derive(Debug)]
pub struct Z2mButtonDevice {
    pub buttons: Vec<Z2mButton>,
    mappings: HashMap<&'static str, Z2mButtonMapping>,
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

impl Z2mButtonDevice {
    pub fn from_model_id(model_id: &str) -> Option<Self> {
        match model_id {
            "RWL021" | "RWL022" => Some(hue_dimmer_switch()),
            "GreenPower_2" => Some(friends_of_hue_switch()),
            _ => None,
        }
    }

    pub fn map_button(&self, action: &str) -> Option<Z2mButtonMapping> {
        self.mappings.get(&action).cloned()
    }
}

fn friends_of_hue_switch() -> Z2mButtonDevice {
    let events = vec![ButtonEvent::InitialPress, ButtonEvent::ShortRelease];
    Z2mButtonDevice {
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

fn hue_dimmer_switch() -> Z2mButtonDevice {
    let events = vec![
        ButtonEvent::InitialPress,
        ButtonEvent::Repeat,
        ButtonEvent::ShortRelease,
        ButtonEvent::LongRelease,
        ButtonEvent::LongPress,
    ];
    Z2mButtonDevice {
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
