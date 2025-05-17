use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use svc::serviceid::ServiceName;
use svc::traits::ServiceState;

use crate::Client;
use crate::error::BifrostResult;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Service {
    pub id: Uuid,
    pub name: ServiceName,
    pub state: ServiceState,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, Eq, PartialEq)]
pub struct ServiceList {
    pub services: BTreeMap<Uuid, Service>,
}

impl Client {
    pub async fn service_list(&self) -> BifrostResult<ServiceList> {
        self.get("service").await
    }

    pub async fn service_stop(&self, id: Uuid) -> BifrostResult<Uuid> {
        self.put(&format!("service/{id}"), ServiceState::Stopped)
            .await
    }

    pub async fn service_start(&self, id: Uuid) -> BifrostResult<Uuid> {
        self.put(&format!("service/{id}"), ServiceState::Running)
            .await
    }
}
