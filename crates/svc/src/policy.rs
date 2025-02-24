use std::time::Duration;

use tokio::time::sleep;

pub enum Retry {
    No,
    Limit(u32),
    Forever,
}

pub struct Policy {
    pub retry: Retry,
    pub delay: Option<Duration>,
}

impl Default for Policy {
    fn default() -> Self {
        Self::new()
    }
}

impl Policy {
    pub const fn new() -> Self {
        Self {
            retry: Retry::No,
            delay: None,
        }
    }

    pub fn with_retry(self, retry: Retry) -> Self {
        Self { retry, ..self }
    }

    pub fn with_delay(self, delay: Duration) -> Self {
        Self {
            delay: Some(delay),
            ..self
        }
    }

    pub fn without_delay(self) -> Self {
        Self {
            delay: None,
            ..self
        }
    }

    pub async fn sleep(&self) {
        if let Some(dur) = self.delay {
            sleep(dur).await
        }
    }

    pub fn should_retry(&self, retry: u32) -> bool {
        match self.retry {
            Retry::No => false,
            Retry::Limit(limit) => retry < limit,
            Retry::Forever => true,
        }
    }
}
