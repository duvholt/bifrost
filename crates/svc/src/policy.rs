//! Implements policies for service behavior (retry count, delay, etc).
use std::time::Duration;

#[cfg(feature = "manager")]
use tokio::time::sleep;

#[derive(Debug, Clone, Copy)]
pub enum Retry {
    No,
    Limit(u32),
    Forever,
}

#[derive(Debug, Clone, Copy)]
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
    #[must_use]
    pub const fn new() -> Self {
        Self {
            retry: Retry::No,
            delay: None,
        }
    }

    #[must_use]
    pub const fn with_retry(self, retry: Retry) -> Self {
        Self { retry, ..self }
    }

    #[must_use]
    pub const fn with_delay(self, delay: Duration) -> Self {
        Self {
            delay: Some(delay),
            ..self
        }
    }

    #[must_use]
    pub const fn without_delay(self) -> Self {
        Self {
            delay: None,
            ..self
        }
    }

    #[cfg(feature = "manager")]
    pub async fn sleep(&self) {
        if let Some(dur) = self.delay {
            sleep(dur).await;
        }
    }

    #[must_use]
    pub const fn should_retry(&self, retry: u32) -> bool {
        match self.retry {
            Retry::No => false,
            Retry::Limit(limit) => retry < limit,
            Retry::Forever => true,
        }
    }
}
