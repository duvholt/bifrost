use std::ops::AddAssign;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use uuid::{Uuid, uuid};

use super::{DollarRef, ResourceLink};

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

impl BehaviorScript {
    pub const WAKE_UP_ID: Uuid = uuid!("ff8957e3-2eb9-4699-a0c8-ad2cb3ede704");

    #[must_use]
    pub fn wake_up() -> Self {
        Self {
            configuration_schema: DollarRef {
                dref: Some("basic_wake_up_config.json#".to_string()),
            },
            description:
                "Get your body in the mood to wake up by fading on the lights in the morning."
                    .to_string(),
            max_number_instances: None,
            metadata: BehaviorScriptMetadata {
                name: "Basic wake up routine".to_string(),
                category: "automation".to_string(),
            },
            state_schema: DollarRef { dref: None },
            supported_features: vec!["style_sunrise".to_string(), "intensity".to_string()],
            trigger_schema: DollarRef {
                dref: Some("trigger.json#".to_string()),
            },
            version: "0.0.1".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BehaviorScriptMetadata {
    pub name: String,
    pub category: String,
}

fn deserialize_optional_field<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(Value::deserialize(deserializer)?))
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct BehaviorInstance {
    #[serde(default)]
    pub dependees: Vec<BehaviorInstanceDependee>,
    pub enabled: bool,
    pub last_error: Option<String>,
    pub metadata: BehaviorInstanceMetadata,
    pub script_id: Uuid,
    pub status: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_field",
        skip_serializing_if = "Option::is_none"
    )]
    pub state: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migrated_from: Option<Value>,
    pub configuration: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum BehaviorInstanceConfiguration {
    Wakeup(WakeupConfiguration),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeupConfiguration {
    pub end_brightness: f64,
    pub fade_in_duration: configuration::Duration,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_lights_off_after: Option<configuration::Duration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<WakeupStyle>,
    pub when: configuration::When,
    #[serde(rename = "where")]
    pub where_field: Vec<configuration::Where>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WakeupStyle {
    Sunrise,
    Basic,
}

pub mod configuration {
    use std::time::Duration as StdDuration;

    use chrono::Weekday;
    use serde::{Deserialize, Serialize};

    use crate::api::ResourceLink;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Duration {
        pub seconds: u32,
    }

    impl Duration {
        pub fn to_std(&self) -> StdDuration {
            StdDuration::from_secs(self.seconds.into())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct When {
        pub recurrence_days: Option<Vec<Weekday>>,
        pub time_point: TimePoint,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum TimePoint {
        Time { time: Time },
    }

    impl TimePoint {
        pub const fn time(&self) -> &Time {
            match self {
                Self::Time { time } => time,
            }
        }
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
pub struct BehaviorInstanceDependee {
    #[serde(rename = "type")]
    pub type_field: Option<String>,
    pub target: ResourceLink,
    pub level: BehaviorInstanceDependeeLevel,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorInstanceDependeeLevel {
    Critical,
    NonCritical,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct BehaviorInstanceMetadata {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BehaviorInstanceNew {
    pub configuration: Value,
    pub enabled: bool,
    pub metadata: BehaviorInstanceMetadata,
    pub script_id: Uuid,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BehaviorInstanceUpdate {
    pub configuration: Option<Value>,
    pub enabled: Option<bool>,
    pub metadata: Option<BehaviorInstanceMetadata>,
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
            self.configuration = configuration;
        }
    }
}
