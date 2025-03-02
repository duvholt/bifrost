use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;
use uuid::Uuid;

use crate::error::RunSvcError;
use crate::manager::SvmRequest;
use crate::policy::{Policy, Retry};
use crate::traits::{Service, ServiceRunner, ServiceState};

struct State {
    id: Uuid,
    retry: u32,
    state: ServiceState,
    tx: mpsc::Sender<SvmRequest>,
}

impl State {
    pub fn new(id: Uuid, state: ServiceState, tx: mpsc::Sender<SvmRequest>) -> Self {
        Self {
            id,
            retry: 0,
            state,
            tx,
        }
    }

    pub async fn set(&mut self, next: ServiceState) -> Result<(), RunSvcError> {
        self.state = next;
        self.retry = 0;
        Ok(self
            .tx
            .send(SvmRequest::ServiceEvent {
                id: self.id,
                state: self.state,
            })
            .await?)
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

pub struct StandardService<S: Service> {
    name: String,
    svc: S,
    configure_policy: Policy,
    start_policy: Policy,
    run_policy: Policy,
    stop_policy: Policy,
}

impl<S: Service> StandardService<S> {
    pub fn new(name: impl AsRef<str>, svc: S) -> Self {
        Self {
            name: name.as_ref().to_string(),
            svc,
            configure_policy: Policy::new(),
            start_policy: Policy::new()
                .with_delay(Duration::from_secs(1))
                .with_retry(Retry::Limit(3)),
            run_policy: Policy::new().with_delay(Duration::from_secs(1)),
            stop_policy: Policy::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn with_configure_policy(mut self, policy: Policy) -> Self {
        self.configure_policy = policy;
        self
    }

    pub fn with_start_policy(mut self, policy: Policy) -> Self {
        self.start_policy = policy;
        self
    }

    pub fn with_run_policy(mut self, policy: Policy) -> Self {
        self.run_policy = policy;
        self
    }

    pub fn with_stop_policy(mut self, policy: Policy) -> Self {
        self.stop_policy = policy;
        self
    }
}

#[async_trait]
impl<S: Service> ServiceRunner for StandardService<S> {
    async fn run(
        mut self,
        id: Uuid,
        mut rx: watch::Receiver<ServiceState>,
        tx: mpsc::Sender<SvmRequest>,
    ) -> Result<(), RunSvcError> {
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
                        self.run_policy.sleep().await;
                        if self.run_policy.should_retry(state.retry()) {
                            log::warn!("[{name}] Service failed to start, retrying..");
                        } else {
                            log::error!("[{name}] Failed to run service: {err}");
                            match svc.stop().await {
                                Ok(()) => {
                                    log::debug!("[{name}] Stopped failing service");
                                }
                                Err(err) => {
                                    log::error!("[{name}] Failed to stop failing service: {err}");
                                }
                            }
                            state.set(ServiceState::Failed).await?;
                        }
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
                    rx.changed().await?;
                    if rx.has_changed()? {
                        log::debug!("[{name}] Service stopped.");
                    }
                }

                ServiceState::Failed => {
                    rx.changed().await?;
                    if rx.has_changed()? {
                        log::debug!("[{name}] Service failed.");
                    }
                }
            }
        }
    }
}
