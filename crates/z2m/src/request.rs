use serde::Serialize;
use serde_json::Value;

use crate::update::DeviceUpdate;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Z2mRequest<'a> {
    SceneStore {
        name: &'a str,
        #[serde(rename = "ID")]
        id: u32,
    },

    SceneRecall(u32),

    SceneRemove(u32),

    #[serde(untagged)]
    Update(&'a DeviceUpdate),

    #[serde(untagged)]
    Raw(Value),

    #[serde(untagged)]
    RawWrite(Value),

    #[serde(untagged)]
    Untyped {
        endpoint: u32,
        value: &'a Value,
    },
}
