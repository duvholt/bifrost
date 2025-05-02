use serde::Serialize;
use serde_json::Value;

use crate::api::GroupMemberChange;
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

    Write {
        cluster: u16,
        payload: Value,
    },

    #[serde(untagged)]
    GroupMemberAdd(GroupMemberChange),

    #[serde(untagged)]
    GroupMemberRemove(GroupMemberChange),

    #[serde(untagged)]
    Update(&'a DeviceUpdate),

    #[serde(untagged)]
    Raw(Value),
}
