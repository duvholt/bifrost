use hue::event::EventBlock;
use serde::{Deserialize, Serialize};

use crate::backend::BackendRequest;
use crate::config::AppConfig;
use crate::service::Service;

#[derive(Debug, Serialize, Deserialize)]
pub enum Update {
    AppConfig(AppConfig),
    HueEvent(EventBlock),
    BackendRequest(BackendRequest),
    ServiceUpdate(Service),
}
