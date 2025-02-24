use std::error::Error;

use async_trait::async_trait;
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

use crate::error::RunSvcError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub trait Service<E: Error + Send>: Send {
    async fn configure(&mut self) -> Result<(), E> {
        Ok(())
    }

    async fn start(&mut self) -> Result<(), E> {
        Ok(())
    }

    async fn run(&mut self) -> Result<(), E>;

    async fn stop(&mut self) -> Result<(), E> {
        Ok(())
    }
}

#[async_trait]
pub trait ServiceRunner<S, E>
where
    E: Error + Send,
    S: Service<E>,
    RunSvcError<E>: From<E>,
{
    fn new(id: Uuid, name: impl AsRef<str>, svc: S) -> Self;
    fn uuid(&self) -> Uuid;
    fn name(&self) -> &str;

    async fn run(
        mut self,
        rx: watch::Receiver<ServiceState>,
        tx: mpsc::Sender<(Uuid, ServiceState)>,
    ) -> Result<(), RunSvcError<E>>;
}
