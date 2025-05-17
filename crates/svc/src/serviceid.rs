use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub struct ServiceName {
    name: String,
    instance: Option<String>,
}

impl From<ServiceName> for String {
    fn from(value: ServiceName) -> Self {
        value.to_string()
    }
}

impl ServiceName {
    // suppress clippy false-positive
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn instance(&self) -> Option<&str> {
        self.instance.as_deref()
    }
}

impl Display for ServiceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self {
                name,
                instance: None,
            } => write!(f, "{name}"),
            Self {
                name,
                instance: Some(instance),
            } => write!(f, "{name}@{instance}"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ServiceId {
    Name(ServiceName),
    Id(Uuid),
}

impl Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Name(name) => write!(f, "{name}"),
            Self::Id(uuid) => write!(f, "{uuid}"),
        }
    }
}

pub trait IntoServiceId: Display + Debug + Clone {
    fn service_id(self) -> ServiceId;
}

impl IntoServiceId for ServiceId {
    fn service_id(self) -> ServiceId {
        self
    }
}

impl<I: IntoServiceId> IntoServiceId for &I {
    fn service_id(self) -> ServiceId {
        self.clone().service_id()
    }
}

impl IntoServiceId for Uuid {
    fn service_id(self) -> ServiceId {
        ServiceId::Id(self)
    }
}

impl IntoServiceId for String {
    fn service_id(self) -> ServiceId {
        ServiceId::Name(ServiceName::from(self))
    }
}

impl IntoServiceId for &str {
    fn service_id(self) -> ServiceId {
        ServiceId::Name(ServiceName::from(self))
    }
}

impl From<Uuid> for ServiceId {
    fn from(value: Uuid) -> Self {
        Self::Id(value)
    }
}

impl From<String> for ServiceId {
    fn from(value: String) -> Self {
        value.service_id()
    }
}

impl From<String> for ServiceName {
    fn from(value: String) -> Self {
        if let Some((name, instance)) = value.split_once('@') {
            Self {
                name: name.to_string(),
                instance: Some(instance.to_string()),
            }
        } else {
            Self {
                name: value,
                instance: None,
            }
        }
    }
}

impl From<&str> for ServiceName {
    fn from(value: &str) -> Self {
        if let Some((name, instance)) = value.split_once('@') {
            Self {
                name: name.to_string(),
                instance: Some(instance.to_string()),
            }
        } else {
            Self {
                name: value.to_string(),
                instance: None,
            }
        }
    }
}
