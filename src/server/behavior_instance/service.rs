use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Weekday;
use hue::event::Event;
use svc::traits::Service;
use tokio::sync::{Mutex, mpsc};

use hue::api::{
    BehaviorInstance, BehaviorInstanceConfigurationUpdate, BehaviorScript, RType, Resource,
};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::resource::Resources;
use crate::server::behavior_instance::hue_accessories::HueAccessoriesBehavior;
use crate::server::behavior_instance::schedule::ScheduledBehaviorRunner;
use crate::server::behavior_instance::sleep::GoToSleepBehavior;
use crate::server::behavior_instance::wakeup::WakeupBehavior;
use crate::server::behavior_instance::{
    BehaviorClock, BehaviorCommand, BehaviorContext, BehaviorTriggerRequest, SystemClock,
};

#[derive(Debug)]
struct BehaviorHandle {
    commands: mpsc::Sender<BehaviorCommand>,
    task: JoinHandle<()>,
}

impl Drop for BehaviorHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Debug)]
pub struct BehaviorInstanceService {
    res: Arc<Mutex<Resources>>,
    handles: HashMap<Uuid, BehaviorHandle>,
    clock: Arc<dyn BehaviorClock>,
    behavior_trigger_rx: mpsc::Receiver<BehaviorTriggerRequest>,
}

impl BehaviorInstanceService {
    pub fn new(
        res: Arc<Mutex<Resources>>,
        behavior_trigger_rx: mpsc::Receiver<BehaviorTriggerRequest>,
    ) -> Self {
        Self::with_clock(res, behavior_trigger_rx, Arc::new(SystemClock))
    }

    pub fn with_clock(
        res: Arc<Mutex<Resources>>,
        behavior_trigger_rx: mpsc::Receiver<BehaviorTriggerRequest>,
        clock: Arc<dyn BehaviorClock>,
    ) -> Self {
        Self {
            handles: HashMap::new(),
            res,
            clock,
            behavior_trigger_rx,
        }
    }

    async fn get_all_behavior_instances(&self) -> Vec<Uuid> {
        self.res
            .lock()
            .await
            .get_resources_by_type(RType::BehaviorInstance)
            .into_iter()
            .filter_map(|r| match r.obj {
                Resource::BehaviorInstance(_behavior_instance) => Some(r.id),
                _ => None,
            })
            .collect()
    }

    async fn get_behavior_instance(&self, rid: Uuid) -> ApiResult<BehaviorInstance> {
        let bi = self
            .res
            .lock()
            .await
            .get_id::<BehaviorInstance>(rid)?
            .clone();
        Ok(bi)
    }

    async fn new_job(&mut self, rid: Uuid) -> ApiResult<()> {
        let bi = self.get_behavior_instance(rid).await?;
        let (commands, receiver) = mpsc::channel(16);
        let context = Arc::new(BehaviorContext::new(self.res.clone(), self.clock.clone()));

        let handle = match bi.script_id {
            BehaviorScript::WAKE_UP_ID => {
                let behavior =
                    ScheduledBehaviorRunner::new(rid, bi, WakeupBehavior, context, receiver)?;
                let task = tokio::spawn(async move { behavior.run().await });
                BehaviorHandle { commands, task }
            }
            BehaviorScript::GO_TO_SLEEP_ID => {
                let behavior =
                    ScheduledBehaviorRunner::new(rid, bi, GoToSleepBehavior, context, receiver)?;
                let task = tokio::spawn(async move { behavior.run().await });
                BehaviorHandle { commands, task }
            }
            BehaviorScript::HUE_ACCESSORIES_ID => {
                let behavior = HueAccessoriesBehavior::new(&bi, context, receiver)?;
                let task = tokio::spawn(async move { behavior.run().await });
                BehaviorHandle { commands, task }
            }
            _ => return Ok(()),
        };

        self.handles.insert(rid, handle);
        Ok(())
    }

    async fn update_job(&mut self, rid: Uuid) -> ApiResult<()> {
        let bi = self.get_behavior_instance(rid).await?;
        match self.handles.get_mut(&rid) {
            Some(handle) => {
                handle
                    .commands
                    .send(BehaviorCommand::Update(Box::new(bi)))
                    .await?;
            }
            None => {
                self.new_job(rid).await?;
            }
        }
        Ok(())
    }

    fn delete_job(&mut self, rid: Uuid) {
        self.handles.remove(&rid);
    }

    async fn handle_hue_event(&mut self, event: Event) {
        match event {
            Event::Add(add) => {
                for obj in add.data {
                    if let Resource::BehaviorInstance(_behavior_instance) = obj.obj {
                        log::debug!("Adding behavior instance job {}", obj.id);
                        if let Err(err) = self.new_job(obj.id).await {
                            log::error!(
                                "Failed to create new behavior instance job {}: {}",
                                obj.id,
                                err
                            );
                        }
                    }
                }
            }
            Event::Update(update) => {
                for obj in update.data {
                    if RType::BehaviorInstance == obj.rtype {
                        log::debug!("Updating behavior instance job {}", obj.id);
                        if let Err(err) = self.update_job(obj.id).await {
                            log::error!(
                                "Failed to update behavior instance job {}: {}",
                                obj.id,
                                err
                            );
                        }
                    }
                }
            }
            Event::Delete(delete) => {
                for obj in delete.data {
                    if RType::BehaviorInstance == obj.rtype {
                        log::debug!("Deleting behavior instance job {}", obj.id);
                        self.delete_job(obj.id);
                    }
                }
            }
            Event::Error(_error) => {}
        }
    }

    async fn handle_trigger(&mut self, request: BehaviorTriggerRequest) -> ApiResult<()> {
        if let Some(handle) = self.handles.get_mut(&request.instance_id) {
            handle
                .commands
                .send(BehaviorCommand::Trigger(request.action))
                .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl Service for BehaviorInstanceService {
    type Error = ApiError;

    async fn configure(&mut self) -> Result<(), Self::Error> {
        for rid in self.get_all_behavior_instances().await {
            if let Err(err) = self.new_job(rid).await {
                log::error!("Failed to create behavior instance for: {rid}: {err}");
            }
        }

        Ok(())
    }

    async fn run(&mut self) -> Result<(), Self::Error> {
        let mut hue_events = self.res.lock().await.hue_event_stream().subscribe();

        loop {
            tokio::select! {
                event = hue_events.recv() => {
                    match event {
                        Ok(event) => {
                            self.handle_hue_event(event.block.event).await;
                        }
                        Err(err) => {
                            log::error!("Failed to read event {err}");
                        }
                    }
                }
                Some(request) = self.behavior_trigger_rx.recv() => {
                    log::info!("Received behavior trigger {request:?}");
                    self.handle_trigger(request).await?;
                }
                else => break,
            }
        }
        Ok(())
    }
}

pub async fn disable_behavior_instance(id: Uuid, res: Arc<Mutex<Resources>>) {
    let upd = BehaviorInstanceConfigurationUpdate::default().with_enabled(false);
    let upd_result = res
        .lock()
        .await
        .update::<BehaviorInstance>(&id, |bi| *bi += upd);
    if let Err(err) = upd_result {
        log::error!("Failed to disable behavior instance {err:?}");
    }
}

#[derive(Debug, Clone)]
pub enum ScheduleType {
    Recurring(HashSet<Weekday>),
    Once(),
}
