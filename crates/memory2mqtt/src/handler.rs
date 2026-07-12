use std::collections::BTreeMap;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use z2m::api::{DeviceRemove, GroupMemberChange, PermitJoin, RawMessage};

const BRIDGE_DEVICES_TOPIC: &str = "bridge/devices";
const BRIDGE_GROUPS_TOPIC: &str = "bridge/groups";

const GROUP_MEMBER_ADD_TOPIC: &str = "bridge/request/group/members/add";
const GROUP_MEMBER_REMOVE_TOPIC: &str = "bridge/request/group/members/remove";
const PERMIT_JOIN_TOPIC: &str = "bridge/request/permit_join";
const DEVICE_REMOVE_TOPIC: &str = "bridge/request/device/remove";

fn message(topic: impl Into<String>, payload: impl Serialize) -> RawMessage {
    RawMessage {
        topic: topic.into(),
        payload: serde_json::to_value(payload).expect("test Z2M payload is serializable"),
    }
}

fn decode<T: DeserializeOwned>(payload: Value) -> Option<T> {
    serde_json::from_value(payload).ok()
}

pub struct Memory2Mqtt {
    state: BTreeMap<String, Value>,
}

impl Memory2Mqtt {
    #[must_use]
    pub fn new(initial_state: BTreeMap<String, Value>) -> Self {
        let mut state = initial_state;
        for (topic, payload) in [
            (BRIDGE_DEVICES_TOPIC, json!([])),
            (BRIDGE_GROUPS_TOPIC, json!([])),
        ] {
            state.entry(topic.to_string()).or_insert(payload);
        }

        Self { state }
    }

    #[must_use]
    pub fn startup_messages(&self) -> Vec<RawMessage> {
        self.state
            .keys()
            .filter_map(|topic| self.handle_get(topic))
            .collect()
    }

    fn update_state(&mut self, topic: &str, update: &Value) {
        let topic_state = self
            .state
            .entry(topic.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let (Some(topic_state), Some(update)) = (topic_state.as_object_mut(), update.as_object())
        {
            topic_state.extend(update.clone());
        } else {
            *topic_state = update.clone();
        }
    }

    pub fn handle(&mut self, request: RawMessage) -> Vec<RawMessage> {
        let RawMessage { topic, payload } = request;
        match topic.as_str() {
            GROUP_MEMBER_ADD_TOPIC => decode(payload)
                .map(|change| self.change_group_member(change, true))
                .unwrap_or_default(),
            GROUP_MEMBER_REMOVE_TOPIC => decode(payload)
                .map(|change| self.change_group_member(change, false))
                .unwrap_or_default(),
            PERMIT_JOIN_TOPIC => decode(payload)
                .map(|request| self.permit_join(request))
                .unwrap_or_default(),
            DEVICE_REMOVE_TOPIC => decode(payload)
                .map(|request| self.remove_device(request))
                .unwrap_or_default(),
            topic if let Some(topic) = topic.strip_suffix("/get") => {
                self.handle_get(topic).into_iter().collect()
            }
            topic if let Some(topic) = topic.strip_suffix("/set") => {
                self.handle_set(topic, &payload)
            }
            _ => {
                log::debug!("Ignoring unsupported test Z2M request topic {topic}");
                Vec::new()
            }
        }
    }

    fn change_group_member(&self, _change: GroupMemberChange, _add: bool) -> Vec<RawMessage> {
        todo!()
    }

    fn permit_join(&self, _request: PermitJoin) -> Vec<RawMessage> {
        todo!()
    }

    fn remove_device(&self, _request: DeviceRemove) -> Vec<RawMessage> {
        todo!()
    }

    fn handle_get(&self, topic: &str) -> Option<RawMessage> {
        self.state
            .get(topic)
            .cloned()
            .map(|payload| message(topic, payload))
    }

    fn handle_set(&mut self, topic: &str, payload: &Value) -> Vec<RawMessage> {
        self.update_state(topic, payload);
        self.handle_get(topic).into_iter().collect()
    }
}
