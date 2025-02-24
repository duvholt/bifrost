use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::Display;

use tokio::sync::{mpsc, watch};
use tokio::task::{AbortHandle, JoinSet};
use uuid::Uuid;

use crate::error::{RunSvcError, SvcError, SvcResult};
use crate::runservice::StandardService;
use crate::traits::{Service, ServiceRunner, ServiceState};

pub trait IntoServiceId<E: Error + Send>: Display {
    fn service_id(&self, svcm: &ServiceManager<E>) -> Option<Uuid>;
}

impl<E: Error + Send> IntoServiceId<E> for Uuid {
    fn service_id(&self, _svcm: &ServiceManager<E>) -> Option<Uuid> {
        Some(*self)
    }
}

impl<E: Error + Send> IntoServiceId<E> for &str {
    fn service_id(&self, svcm: &ServiceManager<E>) -> Option<Uuid> {
        svcm.lookup(self)
    }
}

pub struct ServiceInstance {
    tx: watch::Sender<ServiceState>,
    name: String,
    state: ServiceState,
    abort_handle: AbortHandle,
}

pub struct ServiceManager<E> {
    rx: mpsc::Receiver<(Uuid, ServiceState)>,
    tx: mpsc::Sender<(Uuid, ServiceState)>,
    svcs: BTreeMap<Uuid, ServiceInstance>,
    names: BTreeMap<String, Uuid>,
    tasks: JoinSet<Result<(), RunSvcError<E>>>,
}

impl<E: Error + Send> Default for ServiceManager<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Error + Send> ServiceManager<E> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(32);
        Self {
            tx,
            rx,
            svcs: BTreeMap::new(),
            names: BTreeMap::new(),
            tasks: JoinSet::new(),
        }
    }

    pub fn register_standard(
        &mut self,
        name: impl AsRef<str>,
        svc: impl Service<E> + 'static,
    ) -> SvcResult<Uuid>
    where
        RunSvcError<E>: From<E> + 'static,
    {
        self.register(StandardService::new(Uuid::new_v4(), name, svc))
    }

    pub fn register<S>(&mut self, svc: impl ServiceRunner<S, E> + 'static) -> SvcResult<Uuid>
    where
        S: Service<E>,
        RunSvcError<E>: From<E> + 'static,
    {
        let name = svc.name().to_string();
        if self.names.contains_key(&name) {
            return Err(SvcError::ServiceAlreadyExists(name));
        }

        let (tx, rx) = watch::channel(ServiceState::Registered);
        let id = svc.uuid();

        let abort_handle = self.tasks.spawn(svc.run(rx, self.tx.clone()));

        let rec = ServiceInstance {
            tx,
            name: name.to_string(),
            state: ServiceState::Registered,
            abort_handle,
        };

        self.svcs.insert(id, rec);
        self.names.insert(name.to_string(), id);

        Ok(id)
    }

    pub fn lookup(&self, name: &str) -> Option<Uuid> {
        self.names.get(name).copied()
    }

    pub fn abort(&self, id: impl IntoServiceId<E>) -> SvcResult<()> {
        let svc = self.get(id)?;

        svc.abort_handle.abort();

        Ok(())
    }

    pub fn get(&self, svc: impl IntoServiceId<E>) -> SvcResult<&ServiceInstance> {
        svc.service_id(self)
            .and_then(|id| self.svcs.get(&id))
            .ok_or_else(|| SvcError::ServiceNameNotFound(svc.to_string()))
    }

    pub fn start(&mut self, id: impl IntoServiceId<E>) -> SvcResult<()> {
        self.get(id)
            .and_then(|svc| Ok(svc.tx.send(ServiceState::Running)?))
    }

    pub fn stop(&mut self, id: impl IntoServiceId<E>) -> SvcResult<()> {
        self.get(id)
            .and_then(|svc| Ok(svc.tx.send(ServiceState::Stopped)?))
    }

    pub async fn next_event(&mut self) -> SvcResult<(Uuid, ServiceState)> {
        let (id, state) = self.rx.recv().await.ok_or(SvcError::Shutdown)?;
        let name = &self.svcs[&id].name;
        log::trace!("[{name}] [{id}] Service is now {state:?}");
        self.svcs.get_mut(&id).unwrap().state = state;
        Ok((id, state))
    }

    pub async fn wait_for_state(
        &mut self,
        handle: impl IntoServiceId<E>,
        expected: ServiceState,
    ) -> SvcResult<()> {
        let id = handle
            .service_id(self)
            .ok_or(SvcError::ServiceNameNotFound(handle.to_string()))?;

        loop {
            let state = self.get(id)?.state;

            if state == expected {
                break;
            }

            if state == ServiceState::Failed {
                return Err(SvcError::ServiceFailed);
            }

            self.next_event().await?;
        }

        Ok(())
    }

    pub async fn wait_for_start(&mut self, handle: impl IntoServiceId<E>) -> SvcResult<()> {
        self.wait_for_state(handle, ServiceState::Running).await
    }

    pub async fn wait_for_stop(&mut self, handle: impl IntoServiceId<E>) -> SvcResult<()> {
        self.wait_for_state(handle, ServiceState::Stopped).await
    }

    pub async fn run(mut self) -> SvcResult<()> {
        while let Some(update) = self.rx.recv().await {
            log::debug!("Update: {update:?}");
        }

        Ok(())
    }
}
