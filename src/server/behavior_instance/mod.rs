mod hue_accessories;
mod service;
mod wakeup;

use std::fmt::Debug;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Local, Utc};
pub use service::BehaviorInstanceService;

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
