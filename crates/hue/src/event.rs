use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::api::RType;
use crate::date_format;

#[cfg(feature = "rng")]
use crate::api::ResourceLink;
#[cfg(feature = "rng")]
use crate::error::HueResult;
#[cfg(feature = "rng")]
use serde_json::json;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase", tag = "type")]
pub enum Event {
    Add(Add),
    Update(Update),
    Delete(Delete),
    Error(Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EventBlock {
    #[serde(with = "date_format::utc")]
    pub creationtime: DateTime<Utc>,
    pub id: Uuid,
    #[serde(flatten)]
    pub event: Event,
}

#[cfg(feature = "rng")]
impl EventBlock {
    #[must_use]
    pub fn add(data: Value) -> Self {
        Self {
            creationtime: Utc::now(),
            id: Uuid::new_v4(),
            event: Event::Add(Add { data: vec![data] }),
        }
    }

    pub fn update(id: &Uuid, id_v1: Option<String>, rtype: RType, data: Value) -> HueResult<Self> {
        Ok(Self {
            creationtime: Utc::now(),
            id: Uuid::new_v4(),
            event: Event::Update(Update {
                data: vec![ObjectUpdate {
                    id: *id,
                    id_v1,
                    rtype,
                    data,
                }],
            }),
        })
    }

    pub fn delete(link: &ResourceLink) -> HueResult<Self> {
        Ok(Self {
            creationtime: Utc::now(),
            id: Uuid::new_v4(),
            event: Event::Delete(Delete {
                data: vec![json!({
                    "id": link.rid,
                    "id_v1": format!("/legacy/{}", link.rid.as_simple()),
                    "type": link.rtype,
                })],
            }),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Add {
    pub data: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ObjectUpdate {
    pub id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_v1: Option<String>,
    #[serde(rename = "type")]
    pub rtype: RType,
    #[serde(flatten)]
    pub data: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Update {
    pub data: Vec<ObjectUpdate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Delete {
    pub data: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Error {}
