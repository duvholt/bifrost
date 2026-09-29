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
    pub async fn expect_cancelled(mut self) {
        timeout(Duration::from_secs(2), self.release.closed())
            .await
            .expect("behavior sleep should be cancelled");
    }

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

    pub fn advance(&self, duration: Duration) {
        let mut now = self.clock.now.lock().unwrap();
        *now += chrono::Duration::from_std(duration).expect("test duration fits chrono");
    }

    pub async fn next_sleep(&mut self) -> TestResult<TestSleep> {
        timeout(Duration::from_secs(2), self.sleeps.recv())
            .await
            .map_err(|_| TestError::BehaviorSleepTimeout)?
            .ok_or(TestError::BehaviorSleepTimeout)
    }

    pub async fn expect_sleep(&mut self, duration: Duration) -> TestResult<TestSleep> {
        let sleep = self.next_sleep().await?;
        assert_eq!(
            sleep.duration, duration,
            "unexpected behavior sleep duration"
        );
        Ok(sleep)
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
