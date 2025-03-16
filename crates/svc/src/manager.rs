#![allow(clippy::future_not_send)]
//! A [`ServiceManager`] manages a collection of [`Service`] instances.
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::Debug;
use std::future::Future;
use std::time::Duration;

use futures::future::BoxFuture;
use tokio::select;
use tokio::sync::{mpsc, watch};
use tokio::task::{AbortHandle, JoinHandle, JoinSet};
use uuid::Uuid;

use crate::error::{RunSvcError, SvcError, SvcResult};
use crate::rpc::RpcRequest;
use crate::runservice::StandardService;
use crate::serviceid::{IntoServiceId, ServiceId};
use crate::traits::{Service, ServiceRunner, ServiceState};

pub struct ServiceInstance {
    tx: watch::Sender<ServiceState>,
    name: String,
    state: ServiceState,
    abort_handle: AbortHandle,
}

pub type ServiceFunc = Box<
    dyn FnOnce(
            Uuid,
            watch::Receiver<ServiceState>,
            mpsc::Sender<ServiceEvent>,
        ) -> BoxFuture<'static, Result<(), RunSvcError>>
        + Send,
>;

#[derive(Debug, Clone, Copy)]
pub struct ServiceEvent {
    id: Uuid,
    state: ServiceState,
}

impl ServiceEvent {
    #[must_use]
    pub const fn new(id: Uuid, state: ServiceState) -> Self {
        Self { id, state }
    }
}

/// A request to a [`ServiceManager`]
pub enum SvmRequest {
    Stop(RpcRequest<ServiceId, SvcResult<()>>),
    Start(RpcRequest<ServiceId, SvcResult<()>>),
    Status(RpcRequest<ServiceId, SvcResult<ServiceState>>),
    List(RpcRequest<(), Vec<(Uuid, String)>>),
    Resolve(RpcRequest<ServiceId, SvcResult<Uuid>>),
    Register(RpcRequest<(String, ServiceFunc), SvcResult<Uuid>>),
    Subscribe(RpcRequest<mpsc::Sender<ServiceEvent>, SvcResult<Uuid>>),
    Shutdown(RpcRequest<(), ()>),
}

#[derive(Clone)]
pub struct SvmClient {
    tx: mpsc::Sender<SvmRequest>,
}

impl SvmClient {
    #[must_use]
    pub const fn new(tx: mpsc::Sender<SvmRequest>) -> Self {
        Self { tx }
    }

    pub async fn rpc<Q, A>(
        &mut self,
        func: impl FnOnce(RpcRequest<Q, A>) -> SvmRequest,
        args: Q,
    ) -> SvcResult<A> {
        let (rpc, rx) = RpcRequest::new(args);
        self.send(func(rpc)).await?;
        Ok(rx.await?)
    }

    async fn send(&self, value: SvmRequest) -> SvcResult<()> {
        Ok(self.tx.send(value).await?)
    }

    pub async fn register_service<S>(&mut self, name: impl AsRef<str>, svc: S) -> SvcResult<Uuid>
    where
        S: Service + 'static,
    {
        self.register(&name, StandardService::new(&name, svc)).await
    }

    pub async fn register_function<F, E>(
        &mut self,
        name: impl AsRef<str>,
        func: F,
    ) -> SvcResult<Uuid>
    where
        F: Future<Output = Result<(), E>> + Send + 'static,
        E: Error + Send + 'static,
    {
        self.register(&name, StandardService::new(&name, Box::pin(func)))
            .await
    }

    pub async fn register<S>(&mut self, name: impl AsRef<str>, svc: S) -> SvcResult<Uuid>
    where
        S: ServiceRunner + Send + 'static,
    {
        let name = name.as_ref().to_string();
        self.rpc(
            SvmRequest::Register,
            (name, Box::new(|a, b, c| svc.run(a, b, c))),
        )
        .await?
    }

    pub async fn start(&mut self, id: impl IntoServiceId) -> SvcResult<()> {
        self.rpc(SvmRequest::Start, id.service_id()).await?
    }

    pub async fn stop(&mut self, id: impl IntoServiceId) -> SvcResult<()> {
        self.rpc(SvmRequest::Stop, id.service_id()).await?
    }

    pub async fn resolve(&mut self, id: impl IntoServiceId) -> SvcResult<Uuid> {
        self.rpc(SvmRequest::Resolve, id.service_id()).await?
    }

    pub async fn subscribe(&mut self) -> SvcResult<(Uuid, mpsc::Receiver<ServiceEvent>)> {
        let (tx, rx) = mpsc::channel(10);

        let uuid = self.rpc(SvmRequest::Subscribe, tx).await??;

        Ok((uuid, rx))
    }

    pub async fn wait_for_state(
        &mut self,
        handle: impl IntoServiceId,
        expected: ServiceState,
    ) -> SvcResult<()> {
        let svc_id = self.resolve(&handle).await?;

        let (_cid, mut channel) = self.subscribe().await?;

        while let Some(msg) = channel.recv().await {
            if msg.id == svc_id {
                if msg.state == expected {
                    return Ok(());
                }

                if msg.state == ServiceState::Failed {
                    return Err(SvcError::ServiceFailed);
                }
            }
        }

        Err(SvcError::Shutdown)
    }

    pub async fn wait_for_start(
        &mut self,
        handle: impl IntoServiceId + Send + 'static,
    ) -> SvcResult<()> {
        self.wait_for_state(handle, ServiceState::Running).await
    }

    pub async fn wait_for_stop(
        &mut self,
        handle: impl IntoServiceId + Send + 'static,
    ) -> SvcResult<()> {
        self.wait_for_state(handle, ServiceState::Stopped).await
    }

    pub async fn status(
        &mut self,
        id: impl IntoServiceId + Send + 'static,
    ) -> SvcResult<ServiceState> {
        self.rpc(SvmRequest::Status, id.service_id()).await?
    }

    pub async fn list(&mut self) -> SvcResult<Vec<(Uuid, String)>> {
        self.rpc(SvmRequest::List, ()).await
    }

    pub async fn shutdown(&mut self) -> SvcResult<()> {
        self.rpc(SvmRequest::Shutdown, ()).await
    }
}

impl Debug for SvmRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stop(arg0) => f.debug_tuple("Stop").field(arg0).finish(),
            Self::Start(arg0) => f.debug_tuple("Start").field(arg0).finish(),
            Self::Status(arg0) => f.debug_tuple("Status").field(arg0).finish(),
            Self::List(arg0) => f.debug_tuple("List").field(arg0).finish(),
            Self::Register(_arg0) => f.debug_tuple("Register").field(&"<service>").finish(),
            Self::Resolve(arg0) => f.debug_tuple("Resolve").field(arg0).finish(),
            Self::Subscribe(_arg0) => f.debug_tuple("Subscribe").finish(),
            Self::Shutdown(_arg0) => f.debug_tuple("Shutdown").finish(),
        }
    }
}

pub struct ServiceManager {
    control_rx: mpsc::Receiver<SvmRequest>,
    control_tx: mpsc::Sender<SvmRequest>,
    service_rx: mpsc::Receiver<ServiceEvent>,
    service_tx: mpsc::Sender<ServiceEvent>,
    subscribers: BTreeMap<Uuid, mpsc::Sender<ServiceEvent>>,
    svcs: BTreeMap<Uuid, ServiceInstance>,
    names: BTreeMap<String, Uuid>,
    tasks: JoinSet<Result<(), RunSvcError>>,
    shutdown: bool,
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceManager {
    #[must_use]
    pub fn new() -> Self {
        let (control_tx, control_rx) = mpsc::channel(32);
        let (service_tx, service_rx) = mpsc::channel(32);
        Self {
            control_tx,
            control_rx,
            service_tx,
            service_rx,
            subscribers: BTreeMap::new(),
            svcs: BTreeMap::new(),
            names: BTreeMap::new(),
            tasks: JoinSet::new(),
            shutdown: false,
        }
    }

    /// Daemonize the [`ServiceManager`], returning a (clonable) [`SvmClient`] as
    /// well as a [`JoinHandle`] used to control the service manager task
    /// itself.
    #[must_use]
    pub fn daemonize(self) -> (SvmClient, JoinHandle<SvcResult<()>>) {
        let client = self.client();
        let fut = tokio::task::spawn(self.run());
        (client, fut)
    }

    /// Convenience function to create and daemonize a [`ServiceManager`].
    #[must_use]
    pub fn spawn() -> (SvmClient, JoinHandle<SvcResult<()>>) {
        Self::new().daemonize()
    }

    /// Create a new [`SvmClient`] connected to this service manager.
    #[must_use]
    pub fn client(&self) -> SvmClient {
        SvmClient::new(self.handle())
    }

    fn handle(&self) -> mpsc::Sender<SvmRequest> {
        self.control_tx.clone()
    }

    fn register(&mut self, name: &str, svc: ServiceFunc) -> SvcResult<Uuid> {
        let name = name.to_string();
        if self.names.contains_key(&name) {
            return Err(SvcError::ServiceAlreadyExists(name));
        }

        let (tx, rx) = watch::channel(ServiceState::Registered);
        let id = Uuid::new_v4();

        let abort_handle = self.tasks.spawn((svc)(id, rx, self.service_tx.clone()));

        let rec = ServiceInstance {
            tx,
            name: name.to_string(),
            state: ServiceState::Registered,
            abort_handle,
        };

        self.svcs.insert(id, rec);
        self.names.insert(name, id);

        Ok(id)
    }

    fn list(&self) -> impl Iterator<Item = &Uuid> {
        self.svcs.keys()
    }

    fn resolve(&self, handle: impl IntoServiceId) -> SvcResult<Uuid> {
        let id = handle.service_id();
        match &id {
            ServiceId::Name(name) => self
                .names
                .get(name.as_str())
                .ok_or_else(|| SvcError::ServiceNotFound(id))
                .copied(),
            ServiceId::Id(uuid) => {
                if self.svcs.contains_key(uuid) {
                    Ok(*uuid)
                } else {
                    Err(SvcError::ServiceNotFound(id))
                }
            }
        }
    }

    fn remove(&mut self, handle: &ServiceId) -> SvcResult<()> {
        let id = self.resolve(handle)?;
        self.svcs.remove(&id);
        self.names.retain(|_, v| *v != id);

        Ok(())
    }

    fn abort(&mut self, id: &ServiceId) -> SvcResult<()> {
        let svc = self.get(id)?;

        svc.abort_handle.abort();

        self.remove(id)
    }

    fn get(&self, svc: impl IntoServiceId) -> SvcResult<&ServiceInstance> {
        let id = self.resolve(svc)?;
        Ok(&self.svcs[&id])
    }

    fn start(&self, id: impl IntoServiceId) -> SvcResult<()> {
        self.get(&id).and_then(|svc| {
            log::debug!("Starting {id} {}", &svc.name);
            Ok(svc.tx.send(ServiceState::Running)?)
        })
    }

    fn stop(&self, id: impl IntoServiceId) -> SvcResult<()> {
        let id = self.resolve(id)?;

        if self.svcs[&id].state == ServiceState::Stopped {
            return Ok(());
        }

        log::debug!("Stopping {id} {}", self.svcs[&id].name);
        self.get(id)
            .and_then(|svc| Ok(svc.tx.send(ServiceState::Stopped)?))
    }

    async fn notify_subscribers(&mut self, event: ServiceEvent) {
        let mut failed = vec![];
        for (key, sub) in &self.subscribers {
            log::trace!("UPDATE: [sub-{key}] {} -> {:?}", &event.id, &event.state);
            if sub.send(event).await.is_err() {
                failed.push(*key);
            }
        }
        if !failed.is_empty() {
            self.subscribers.retain(|k, _| !failed.contains(k));
        }
    }

    async fn next_event(&mut self) -> SvcResult<()> {
        tokio::select! {
            event = self.control_rx.recv() => self.handle_svm_request(event.ok_or(SvcError::Shutdown)?).await,
            event = self.service_rx.recv() => self.handle_service_event(event.ok_or(SvcError::Shutdown)?).await,
        }
    }

    async fn handle_service_event(&mut self, event: ServiceEvent) -> SvcResult<()> {
        self.notify_subscribers(event).await;
        let name = &self.svcs[&event.id].name;
        log::trace!("[{name}] [{}] Service is now {:?}", event.id, event.state);
        self.svcs.get_mut(&event.id).unwrap().state = event.state;

        Ok(())
    }

    async fn handle_svm_request(&mut self, upd: SvmRequest) -> SvcResult<()> {
        match upd {
            SvmRequest::Start(rpc) => rpc.respond(|id| self.start(&id)),

            SvmRequest::Stop(rpc) => rpc.respond(|id| self.stop(&id)),

            SvmRequest::Status(rpc) => rpc.respond(|id| Ok(self.get(&id)?.state)),

            SvmRequest::List(rpc) => rpc.respond(|()| {
                let mut res = vec![];

                for (name, id) in &self.names {
                    res.push((*id, name.to_string()));
                }
                res
            }),

            SvmRequest::Register(rpc) => rpc.respond(|(name, svc)| self.register(&name, svc)),

            SvmRequest::Resolve(rpc) => rpc.respond(|id| self.resolve(&id)),

            SvmRequest::Subscribe(rpc) => {
                for (id, svc) in &self.svcs {
                    rpc.data().send(ServiceEvent::new(*id, svc.state)).await?;
                }

                rpc.respond(|tx| {
                    let uuid = Uuid::new_v4();
                    self.subscribers.insert(uuid, tx);

                    Ok(uuid)
                });
            }

            SvmRequest::Shutdown(rpc) => {
                log::info!("Service managed shutting down..");
                let ids: Vec<Uuid> = self.list().copied().collect();

                self.stop_multiple(&ids)?;

                select! {
                    Ok(()) = Box::pin(self.wait_for_multiple(&ids, ServiceState::Stopped)) => {}
                    () = tokio::time::sleep(Duration::from_secs(3)) => {
                        log::error!("Service shutdown timed out, aborting tasks..");
                        for id in &ids {
                            self.abort(&ServiceId::from(*id))?;
                        }
                    }
                }
                log::debug!("All services stopped.");
                self.shutdown = true;
                rpc.respond(|_rsp| ());
            }
        }

        Ok(())
    }

    fn stop_multiple(&self, handles: &[impl IntoServiceId]) -> SvcResult<()> {
        let ids = self.resolve_multiple(handles)?;
        for id in ids {
            self.stop(id)?;
        }

        Ok(())
    }

    fn resolve_multiple(&self, handles: &[impl IntoServiceId]) -> SvcResult<BTreeSet<Uuid>> {
        let res = BTreeSet::from_iter(
            handles
                .iter()
                .map(|id| self.resolve(id))
                .collect::<Result<Vec<Uuid>, SvcError>>()?,
        );

        Ok(res)
    }

    async fn wait_for_multiple(
        &mut self,
        handles: &[impl IntoServiceId],
        target: ServiceState,
    ) -> SvcResult<()> {
        let mut missing = self.resolve_multiple(handles)?;
        let mut done = BTreeSet::new();

        loop {
            for m in &missing {
                let state = self.get(*m)?.state;

                if state == ServiceState::Failed && target != ServiceState::Stopped {
                    return Err(SvcError::ServiceFailed);
                }

                if state == target {
                    done.insert(*m);
                }
            }

            missing.retain(|f| !done.contains(f));

            if missing.is_empty() {
                return Ok(());
            }

            self.next_event().await?;
        }
    }

    pub async fn run(mut self) -> SvcResult<()> {
        while !self.shutdown {
            self.next_event().await?;
        }

        Ok(())
    }
}
