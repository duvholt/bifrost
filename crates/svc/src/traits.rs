use std::error::Error;

use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

#[cfg(feature = "manager")]
use crate::error::RunSvcError;
#[cfg(feature = "manager")]
use crate::manager::ServiceEvent;
#[cfg(feature = "manager")]
use crate::template::ErrorAdapter;
#[cfg(feature = "manager")]
use std::future::Future;
#[cfg(feature = "manager")]
use tokio::sync::{mpsc, watch};
#[cfg(feature = "manager")]
use uuid::Uuid;

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

pub enum StopResult {
    Delivered,
    NotSupported,
}

#[async_trait]
pub trait Service: Send {
    type Error: Error + Send + 'static;

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

    async fn signal_stop(&mut self) -> Result<StopResult, Self::Error> {
        Ok(StopResult::NotSupported)
    }

    #[cfg(feature = "manager")]
    fn boxed(self) -> BoxDynService
    where
        Self: Sized + Unpin + 'static,
    {
        Box::new(ErrorAdapter::new(self)) as BoxDynService
    }
}

#[cfg(feature = "manager")]
pub type BoxDynService = Box<dyn Service<Error = RunSvcError> + Unpin + 'static>;

#[cfg(feature = "manager")]
impl Service for BoxDynService {
    type Error = RunSvcError;

    fn run<'a: 'b, 'b>(&'a mut self) -> BoxFuture<'b, Result<(), Self::Error>> {
        (**self).run()
    }

    fn configure<'a: 'b, 'b>(&'a mut self) -> BoxFuture<'b, Result<(), Self::Error>> {
        (**self).configure()
    }

    fn start<'a: 'b, 'b>(&'a mut self) -> BoxFuture<'b, Result<(), Self::Error>> {
        (**self).start()
    }

    fn stop<'a: 'b, 'b>(&'a mut self) -> BoxFuture<'b, Result<(), Self::Error>> {
        (**self).stop()
    }

    fn signal_stop<'a: 'b, 'b>(&'a mut self) -> BoxFuture<'b, Result<StopResult, Self::Error>> {
        (**self).signal_stop()
    }
}

#[cfg(feature = "manager")]
#[async_trait]
pub trait ServiceRunner {
    async fn run(
        mut self,
        id: Uuid,
        rx: watch::Receiver<ServiceState>,
        tx: mpsc::UnboundedSender<ServiceEvent>,
    ) -> Result<(), RunSvcError>;
}

#[cfg(feature = "manager")]
#[async_trait]
impl<E, F> Service for F
where
    E: Error + Send + 'static,
    F: Future<Output = Result<(), E>> + Send + Unpin,
{
    type Error = E;

    async fn run(&mut self) -> Result<(), E> {
        self.await
    }
}
