use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use bifrost::server::behavior_instance::BehaviorClock;
use chrono::{DateTime, Local, TimeZone, Utc};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

use super::{TestError, TestResult};

pub struct TestClock {
    clock: Arc<Clock>,
    sleeps: mpsc::UnboundedReceiver<TestSleep>,
}

pub struct TestSleep {
    pub duration: Duration,
    release: oneshot::Sender<()>,
}

impl TestSleep {
    pub fn release(self) {
        self.release.send(()).expect("behavior sleep was cancelled");
    }
}

impl TestClock {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let now = Local
            .with_ymd_and_hms(2000, 1, 1, 6, 0, 0)
            .single()
            .unwrap();
        let clock = Clock {
            now: Mutex::new(now),
            sleeps: tx,
        };
        Self {
            clock: Arc::new(clock),
            sleeps: rx,
        }
    }

    pub fn behavior_clock(&self) -> Arc<dyn BehaviorClock> {
        self.clock.clone()
    }

    pub fn set_now(&self, now: DateTime<Local>) {
        *self.clock.now.lock().unwrap() = now;
    }

    pub async fn next_sleep(&mut self) -> TestResult<TestSleep> {
        timeout(Duration::from_secs(2), self.sleeps.recv())
            .await
            .map_err(|_| TestError::BehaviorSleepTimeout)?
            .ok_or(TestError::BehaviorSleepTimeout)
    }
}

#[derive(Debug)]
struct Clock {
    now: Mutex<DateTime<Local>>,
    sleeps: mpsc::UnboundedSender<TestSleep>,
}

#[async_trait]
impl BehaviorClock for Clock {
    fn now(&self) -> DateTime<Utc> {
        self.local_now().with_timezone(&Utc)
    }

    fn local_now(&self) -> DateTime<Local> {
        *self.now.lock().unwrap()
    }

    async fn sleep(&self, duration: Duration) {
        let (release, rx) = oneshot::channel();
        self.sleeps
            .send(TestSleep { duration, release })
            .expect("test clock dropped");
        let _ = rx.await;
    }
}
