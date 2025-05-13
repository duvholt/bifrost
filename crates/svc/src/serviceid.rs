use std::fmt::{Debug, Display};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ServiceName {
    name: String,
    instance: Option<String>,
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
    Name(String),
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
        ServiceId::Name(self)
    }
}

impl IntoServiceId for &str {
    fn service_id(self) -> ServiceId {
        ServiceId::Name(self.to_string())
    }
}

impl From<Uuid> for ServiceId {
    fn from(value: Uuid) -> Self {
        Self::Id(value)
    }
}

impl From<String> for ServiceId {
    fn from(value: String) -> Self {
        Self::Name(value)
    }
}
