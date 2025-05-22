use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::api::{RType, ResourceRecord};
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
    pub fn add(data: Vec<ResourceRecord>) -> Self {
        Self {
            creationtime: Utc::now(),
            id: Uuid::new_v4(),
            event: Event::Add(Add { data }),
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

    pub fn delete(link: ResourceLink, id_v1: Option<u32>) -> HueResult<Self> {
        Ok(Self {
            creationtime: Utc::now(),
            id: Uuid::new_v4(),
            event: Event::Delete(Delete {
                data: vec![json!({
                    "id": link.rid,
                    "id_v1": id_v1,
                    "type": link.rtype,
                })],
            }),
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Add {
    pub data: Vec<ResourceRecord>,
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

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use crate::api::{RType, Resource, ResourceLink, ResourceRecord};
    use crate::event::{Add, Delete, Event, EventBlock, Update};

    // just some uuid for testing
    const ID: Uuid = Uuid::NAMESPACE_DNS;

    #[test]
    fn add() {
        let obj = ResourceRecord::new(
            ID,
            None,
            Resource::AuthV1(ResourceLink {
                rid: ID,
                rtype: RType::AuthV1,
            }),
        );

        let add = EventBlock::add(vec![obj.clone()]);
        let Event::Add(Add { data }) = add.event else {
            panic!("Wrong event type");
        };

        assert!(data.len() == 1);

        assert_eq!(
            serde_json::to_string(&data[0]).unwrap(),
            serde_json::to_string(&obj).unwrap()
        );
    }

    #[test]
    fn update() {
        let diff = json!({"key": "value"});

        let evt = EventBlock::update(&ID, Some("foo".into()), RType::AuthV1, diff.clone()).unwrap();
        let Event::Update(Update { data }) = evt.event else {
            panic!("Wrong event type");
        };

        assert!(data.len() == 1);

        let out = &data[0];
        assert_eq!(out.id_v1, Some("foo".into()));
        assert_eq!(out.rtype, RType::AuthV1);
        assert_eq!(out.data, diff);
    }

    #[test]
    fn delete() {
        let evt = EventBlock::delete(RType::AuthV1.link_to(ID), Some("foo".into())).unwrap();

        let Event::Delete(Delete { data }) = evt.event else {
            panic!("Wrong event type");
        };

        assert!(data.len() == 1);

        let out = &data[0];
        assert_eq!(out.id_v1, Some("foo".into()));
        assert_eq!(out.rtype, RType::AuthV1);
        assert_eq!(out.id, ID);
    }
}
