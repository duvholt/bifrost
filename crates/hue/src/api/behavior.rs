use std::ops::AddAssign;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BehaviorInstance {
    pub configuration: Value,
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
            self.configuration = configuration;
        }
    }
}
