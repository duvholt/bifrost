use std::ops::AddAssign;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::{uuid, Uuid};

use super::DollarRef;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BehaviorScript {
    pub configuration_schema: DollarRef,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_number_instances: Option<u32>,
    pub metadata: BehaviorScriptMetadata,
    pub state_schema: DollarRef,
    pub supported_features: Vec<String>,
    pub trigger_schema: DollarRef,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BehaviorScriptMetadata {
    pub name: String,
    pub category: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct BehaviorInstance {
    #[serde(default)]
    pub dependees: Vec<Value>,
    pub enabled: bool,
    pub last_error: Option<String>,
    pub metadata: BehaviorInstanceMetadata,
    pub script_id: Uuid,
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migrated_from: Option<Value>,
    pub configuration: BehaviorInstanceConfiguration,
}

// TODO: refer to const in one place
const WAKEUP: Uuid = uuid!("ff8957e3-2eb9-4699-a0c8-ad2cb3ede704");

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum BehaviorInstanceConfiguration {
    #[serde(rename = "ff8957e3-2eb9-4699-a0c8-ad2cb3ede704")]
    Wakeup(WakeupConfiguration),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeupConfiguration {
    pub end_brightness: f64,
    pub fade_in_duration: configuration::FadeInDuration,
    pub style: Option<String>,
    pub when: configuration::When,
    #[serde(rename = "where")]
    pub where_field: Vec<configuration::Where>,
}

pub mod configuration {
    use chrono::Weekday;
    use serde::{Deserialize, Serialize};

    use crate::hue::api::ResourceLink;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct FadeInDuration {
        pub seconds: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct When {
        #[serde(rename = "recurrence_days")]
        pub recurrence_days: Option<Vec<String>>,
        #[serde(rename = "time_point")]
        pub time_point: TimePoint,
    }

    impl When {
        pub fn weekdays(&self) -> Option<Vec<Weekday>> {
            self.recurrence_days
                .as_ref()
                .map(|days| days.iter().filter_map(|w| w.parse().ok()).collect())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct TimePoint {
        pub time: Time,
        #[serde(rename = "type")]
        // time
        pub type_field: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Time {
        pub hour: u32,
        pub minute: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Where {
        pub group: ResourceLink,
        pub items: Option<Vec<ResourceLink>>,
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct BehaviorInstanceMetadata {
    pub name: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BehaviorInstanceUpdate {
    pub configuration: Option<Value>,
    pub enabled: Option<bool>,
    pub metadata: Option<BehaviorInstanceMetadata>,
    // trigger
}

impl BehaviorInstanceUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_metadata(self, metadata: BehaviorInstanceMetadata) -> Self {
        Self {
            metadata: Some(metadata),
            ..self
        }
    }

    #[must_use]
    pub fn with_enabled(self, enabled: bool) -> Self {
        Self {
            enabled: Some(enabled),
            ..self
        }
    }

    #[must_use]
    pub fn with_configuration(self, configuration: Value) -> Self {
        Self {
            configuration: Some(configuration),
            ..self
        }
    }
}

impl AddAssign<BehaviorInstanceUpdate> for BehaviorInstance {
    fn add_assign(&mut self, upd: BehaviorInstanceUpdate) {
        if let Some(md) = upd.metadata {
            self.metadata = md;
        }

        if let Some(enabled) = upd.enabled {
            self.enabled = enabled;
        }

        if let Some(configuration) = upd.configuration {
            if self.script_id == WAKEUP {
                if let Ok(parsed) = serde_json::from_value(configuration) {
                    self.configuration = parsed;
                } else {
                    // todo: log
                }
            }
        }
    }
}
