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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ButtonEvent {
    InitialPress,
    Repeat,
    ShortRelease,
    LongRelease,
    DoubleShortRelease,
    LongPress,
}
