use std::ops::AddAssign;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::api::ResourceLink;
use crate::date_format;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Button {
    pub owner: ResourceLink,
    pub metadata: ButtonMetadata,
    pub button: ButtonData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ButtonMetadata {
    pub control_id: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ButtonData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub button_report: Option<ButtonReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<ButtonEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_interval: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_values: Option<Vec<ButtonEvent>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ButtonReport {
    #[serde(with = "date_format::utc_ms")]
    pub updated: DateTime<Utc>,
    pub event: ButtonEvent,
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ButtonEvent {
    InitialPress,
    Repeat,
    ShortRelease,
    LongRelease,
    DoubleShortRelease,
    LongPress,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ButtonUpdate {
    pub button: Option<ButtonDataUpdate>,
}

impl ButtonUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_button(self, button: ButtonDataUpdate) -> Self {
        Self {
            button: Some(button),
            ..self
        }
    }
}

impl AddAssign<ButtonUpdate> for Button {
    fn add_assign(&mut self, upd: ButtonUpdate) {
        if let Some(button) = upd.button {
            self.button += button;
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ButtonDataUpdate {
    pub button_report: Option<ButtonReport>,
    pub last_event: Option<ButtonEvent>,
}

impl ButtonDataUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_button_report(self, button_report: ButtonReport) -> Self {
        Self {
            button_report: Some(button_report),
            ..self
        }
    }

    #[must_use]
    pub fn with_last_event(self, last_event: ButtonEvent) -> Self {
        Self {
            last_event: Some(last_event),
            ..self
        }
    }
}

impl AddAssign<ButtonDataUpdate> for ButtonData {
    fn add_assign(&mut self, upd: ButtonDataUpdate) {
        if let Some(button_report) = upd.button_report {
            self.button_report = Some(button_report);
        }
        if let Some(last_event) = upd.last_event {
            self.last_event = Some(last_event);
        }
    }
}
