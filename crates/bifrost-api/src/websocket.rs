use hue::event::EventBlock;
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;
use crate::service::Service;

#[derive(Debug, Serialize, Deserialize)]
pub enum Update {
    AppConfig(AppConfig),
    HueEvent(EventBlock),
    ServiceUpdate(Service),
}
