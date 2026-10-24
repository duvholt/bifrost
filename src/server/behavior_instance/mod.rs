mod hue_accessories;
mod schedule;
mod service;
mod sleep;
mod target;
mod trigger;
mod wakeup;

use std::time::Duration;
use std::{fmt::Debug, sync::Arc};

use crate::resource::Resources;
use async_trait::async_trait;
use chrono::{DateTime, Local, Utc};
use hue::api::BehaviorInstance;
pub use service::BehaviorInstanceService;
use tokio::sync::Mutex;
pub use trigger::{BehaviorTriggerAction, BehaviorTriggerRequest};

#[async_trait]
pub trait BehaviorClock: Debug + Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn local_now(&self) -> DateTime<Local>;
    async fn sleep(&self, duration: Duration);
}

#[derive(Debug)]
struct SystemClock;

#[async_trait]
impl BehaviorClock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn local_now(&self) -> DateTime<Local> {
        Local::now()
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

#[derive(Debug)]
struct BehaviorContext {
    res: Arc<Mutex<Resources>>,
    clock: Arc<dyn BehaviorClock>,
}

impl BehaviorContext {
    pub fn new(res: Arc<Mutex<Resources>>, clock: Arc<dyn BehaviorClock>) -> Self {
        Self { res, clock }
    }
}

#[derive(Debug)]
pub enum BehaviorCommand {
    Update(Box<BehaviorInstance>),
    Trigger(BehaviorTriggerAction),
    Shutdown,
}
