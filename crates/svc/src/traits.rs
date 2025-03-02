use std::error::Error;
use std::future::Future;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::error::RunSvcError;

/**
State of a [`Service`] running on a [`crate::manager::ServiceManager`].

Transition diagram for [`ServiceState`]:

```text
    ┌────────────────┐
    │ Registered     ├──┐
    │                │  │
    └───────────┬────┘  │
    ┌───────────▼────┐  │
    │ Configured     │  │
    │                │  │
    └───────────┬────┘  │
    ┌───────────▼────┐  │
 ┌─►│ Starting       ├──┤
 │  │                │  │
 │  └───────────┬────┘  │
 │  ┌───────────▼────┐  │
 │  │ Running        ├──┤
 │  │                │  │
 │  └───────────┬────┘  │
 │  ┌───────────▼────┐  │
 │  │ Stopping       ├──┤
 │  │                │  │
 │  └───────────┬────┘  │
 │  ┌───────────▼────┐  │
 └──┤ Stopped        │  │
 ┌─►│                │  │
 │  └────────────────┘  │
 │  ┌────────────────┐  │
 │  │ Failed         │  │
 └──┤                │◄─┘
    └────────────────┘
```
*/

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceState {
    /// Service is registered with the service manager, but not configured yet
    Registered,
    /// Service is registered, and has finished one-time setup in preparation for running
    Configured,
    /// Service is in the starting phase. If successfull, it will then be in [`ServiceState::Running`].
    Starting,
    /// Service is running normally
    Running,
    /// Servic is in the shutdown phase. If successfull, it will then be in [`ServiceState::Stopped`].
    Stopping,
    /// Service is not running, but is ready to start up again
    Stopped,
    /// Service has failed
    Failed,
}

#[async_trait]
pub trait Service: Send {
    type Error: Error + Send + 'static;
    const SIGNAL_STOP: bool = false;

    async fn configure(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn run(&mut self) -> Result<(), Self::Error>;

    async fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn signal_stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[async_trait]
pub trait ServiceRunner {
    async fn run(
        mut self,
        id: Uuid,
        rx: watch::Receiver<ServiceState>,
        tx: mpsc::Sender<(Uuid, ServiceState)>,
    ) -> Result<(), RunSvcError>;
}

#[async_trait]
impl<E: Error + Send + 'static, F> Service for F
where
    F: Future<Output = Result<(), E>> + Send + Unpin,
{
    type Error = E;

    async fn configure(&mut self) -> Result<(), E> {
        Ok(())
    }

    async fn start(&mut self) -> Result<(), E> {
        Ok(())
    }

    async fn run(&mut self) -> Result<(), E> {
        self.await
    }

    async fn stop(&mut self) -> Result<(), E> {
        Ok(())
    }
}
