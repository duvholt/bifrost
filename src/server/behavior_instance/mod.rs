mod hue_accessories;
mod schedule;
mod service;
mod target;
mod wakeup;

use std::time::Duration;
use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;
use chrono::{DateTime, Local, Utc};
pub use service::BehaviorInstanceService;
use tokio::sync::Mutex;

use crate::resource::Resources;

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

struct BehaviorContext {
    res: Arc<Mutex<Resources>>,
    clock: Arc<dyn BehaviorClock>,
}

impl BehaviorContext {
    pub fn new(res: Arc<Mutex<Resources>>, clock: Arc<dyn BehaviorClock>) -> Self {
        Self { res, clock }
    }
}
