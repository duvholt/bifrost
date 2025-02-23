use chrono::{DateTime, Duration, Utc};

pub struct Throttle {
    interval: Duration,
    last_update: DateTime<Utc>,
}

impl Throttle {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_update: Utc::now(),
        }
    }

    pub fn from_fps(fps: u32) -> Self {
        let interval = Duration::microseconds(1_000_000 / i64::from(fps));
        Self::new(interval)
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed_since(Utc::now())
    }

    pub fn elapsed_since(&self, now: DateTime<Utc>) -> Duration {
        now - self.last_update
    }

    pub fn tick(&mut self) -> bool {
        let now = Utc::now();
        let ready = self.elapsed_since(now) >= self.interval;
        if ready {
            self.last_update = now;
        }

        ready
    }
}
