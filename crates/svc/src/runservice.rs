use std::error::Error;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;
use uuid::Uuid;

use crate::error::RunSvcError;
use crate::traits::{Service, ServiceState};

struct State {
    id: Uuid,
    state: ServiceState,
    tx: mpsc::Sender<(Uuid, ServiceState)>,
}

impl State {
    pub fn new(id: Uuid, state: ServiceState, tx: mpsc::Sender<(Uuid, ServiceState)>) -> Self {
        Self { id, state, tx }
    }

    pub async fn set<E>(&mut self, next: ServiceState) -> Result<(), RunSvcError<E>> {
        self.state = next;
        Ok(self.tx.send((self.id, self.state)).await?)
    }

    pub fn get(&self) -> ServiceState {
        self.state
    }
}

pub async fn run_service<S, E>(
    id: Uuid,
    name: String,
    mut rx: watch::Receiver<ServiceState>,
    tx: mpsc::Sender<(Uuid, ServiceState)>,
    mut svc: S,
) -> Result<(), RunSvcError<E>>
where
    E: Error + Send,
    S: Service<E> + Send,
    RunSvcError<E>: From<E>,
{
    log::debug!("[{name}] Registered");
    svc.configure().await?;

    let mut state = State::new(id, ServiceState::Registered, tx);

    loop {
        match state.get() {
            ServiceState::Registered => match svc.configure().await {
                Ok(()) => {
                    log::debug!("[{name}] Configured");
                    state.set(ServiceState::Configured).await?;
                }
                Err(err) => {
                    log::error!("[{name}] Failed to configure service: {err}");
                    sleep(Duration::from_secs(3)).await;
                }
            },

            ServiceState::Configured => {
                log::debug!("[{name}] Service configured, and is ready start.");
                if *rx.borrow() == ServiceState::Running {
                    state.set(ServiceState::Starting).await?;
                } else {
                    rx.changed().await?
                }
            }

            ServiceState::Starting => match svc.start().await {
                Ok(()) => {
                    log::debug!("[{name}] Started");
                    state.set(ServiceState::Running).await?;
                }
                Err(err) => {
                    log::error!("[{name}] Failed to start service: {err}");
                    sleep(Duration::from_secs(3)).await;
                }
            },

            ServiceState::Running => match svc.run().await {
                Ok(()) => {
                    log::debug!("[{name}] Service completed successfully");
                    state.set(ServiceState::Stopping).await?;
                    /* sleep(Duration::from_secs(1)).await; */
                }
                Err(err) => {
                    log::error!("[{name}] Failed to start service: {err}");
                    sleep(Duration::from_secs(3)).await;
                }
            },

            ServiceState::Stopping => match svc.stop().await {
                Ok(()) => {
                    log::debug!("[{name}] Stopping");
                    state.set(ServiceState::Stopped).await?;
                }
                Err(err) => {
                    log::error!("[{name}] Failed to stop service: {err}");
                    sleep(Duration::from_secs(3)).await;
                }
            },

            ServiceState::Stopped => {
                if rx.has_changed()? {
                    log::debug!("[{name}] Service stopped.");
                }
                rx.changed().await?;
            }
        }
    }
}
