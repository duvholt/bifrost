use async_trait::async_trait;
use std::error::Error;
use std::marker::PhantomData;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;
use uuid::Uuid;

use crate::error::RunSvcError;
use crate::policy::{Policy, Retry};
use crate::traits::{Service, ServiceRunner, ServiceState};

struct State {
    id: Uuid,
    retry: u32,
    state: ServiceState,
    tx: mpsc::Sender<(Uuid, ServiceState)>,
}

impl State {
    pub fn new(id: Uuid, state: ServiceState, tx: mpsc::Sender<(Uuid, ServiceState)>) -> Self {
        Self {
            id,
            retry: 0,
            state,
            tx,
        }
    }

    pub async fn set<E>(&mut self, next: ServiceState) -> Result<(), RunSvcError<E>> {
        self.state = next;
        self.retry = 0;
        Ok(self.tx.send((self.id, self.state)).await?)
    }

    pub fn get(&self) -> ServiceState {
        self.state
    }

    pub fn retry(&mut self) -> u32 {
        let res = self.retry;
        self.retry += 1;
        res
    }
}

pub struct StandardService<S, E>
where
    S: Service<E>,
    E: Error + Send,
{
    id: Uuid,
    name: String,
    svc: S,
    p: PhantomData<E>,
}

#[async_trait]
impl<S, E> ServiceRunner<S, E> for StandardService<S, E>
where
    E: Error + Send,
    S: Service<E>,
    RunSvcError<E>: From<E>,
{
    fn new(id: Uuid, name: String, svc: S) -> Self
    where
        E: Error + Send,
        S: Service<E>,
        RunSvcError<E>: From<E>,
    {
        Self {
            id,
            name,
            svc,
            p: PhantomData,
        }
    }

    fn uuid(&self) -> Uuid {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn run(
        mut self,
        mut rx: watch::Receiver<ServiceState>,
        tx: mpsc::Sender<(Uuid, ServiceState)>,
    ) -> Result<(), RunSvcError<E>>
    where
        RunSvcError<E>: From<E>,
    {
        let id = self.id;
        let name = self.name;
        let mut svc = self.svc;

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
}
