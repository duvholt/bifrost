use std::collections::HashMap;
use std::{iter, sync::Arc};

use async_trait::async_trait;
use chrono::Weekday;
use hue::event::Event;
use svc::traits::Service;
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use hue::api::{
    BehaviorInstance, BehaviorInstanceConfiguration, BehaviorInstanceUpdate, BehaviorScript, RType,
    Resource, WakeupConfiguration,
};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::resource::Resources;
use crate::server::behavior_instance::wakeup::WakeupJob;

#[derive(Debug)]
pub struct BehaviorInstanceService {
    res: Arc<Mutex<Resources>>,
    jobs: HashMap<Uuid, BehaviorInstanceJob>,
}

impl BehaviorInstanceService {
    pub fn new(res: Arc<Mutex<Resources>>) -> Self {
        Self {
            jobs: HashMap::new(),
            res,
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

    async fn get_behavior_configuration(
        &self,
        id: Uuid,
    ) -> ApiResult<Option<BehaviorInstanceConfiguration>> {
        let bi = self
            .res
            .lock()
            .await
            .get_id::<BehaviorInstance>(id)?
            .clone();
        if !bi.enabled {
            return Ok(None);
        }

        match bi.script_id {
            BehaviorScript::WAKE_UP_ID => {
                let config = serde_json::from_value::<WakeupConfiguration>(bi.configuration)?;
                Ok(Some(BehaviorInstanceConfiguration::Wakeup(config)))
            }
            _ => Ok(None),
        }
    }

    async fn new_job(&mut self, rid: Uuid) -> ApiResult<()> {
        if let Some(configuration) = self.get_behavior_configuration(rid).await? {
            self.jobs.insert(
                rid,
                BehaviorInstanceJob::new(rid, configuration, self.res.clone()),
            );
        }
        Ok(())
    }

    async fn update_job(&mut self, rid: Uuid) -> ApiResult<()> {
        let configuration = self.get_behavior_configuration(rid).await?;
        if let Some(job) = self.jobs.get_mut(&rid) {
            match configuration {
                Some(configuration) => {
                    job.update_configuration(configuration);
                }
                None => {
                    self.delete_job(rid).await?;
                }
            }
        }
        Ok(())
    }

    async fn delete_job(&mut self, rid: Uuid) -> ApiResult<()> {
        self.jobs.remove(&rid);
        Ok(())
    }
}

#[async_trait]
impl Service for BehaviorInstanceService {
    type Error = ApiError;

    async fn configure(&mut self) -> Result<(), Self::Error> {
        for rid in self.get_all_behavior_instances().await {
            self.new_job(rid).await?;
        }

        Ok(())
    }

    async fn run(&mut self) -> Result<(), Self::Error> {
        let mut hue_events = self.res.lock().await.hue_event_stream().subscribe();

        loop {
            let event = hue_events.recv().await;
            match event {
                Ok(event) => match event.block.event {
                    Event::Add(add) => {
                        for obj in add.data {
                            if let Resource::BehaviorInstance(_behavior_instance) = obj.obj {
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
                                if let Err(err) = self.delete_job(obj.id).await {
                                    log::error!(
                                        "Failed to delete behavior instance job {}: {}",
                                        obj.id,
                                        err
                                    );
                                }
                            }
                        }
                    }
                    Event::Error(_error) => {}
                },
                Err(err) => {
                    log::error!("Failed to read event {}", err);
                }
            }
        }
    }
}

#[derive(Debug)]
struct BehaviorInstanceJob {
    rid: Uuid,
    configuration: BehaviorInstanceConfiguration,
    res: Arc<Mutex<Resources>>,
    tasks: Vec<JoinHandle<()>>,
}

impl BehaviorInstanceJob {
    pub fn new(
        rid: Uuid,
        configuration: BehaviorInstanceConfiguration,
        res: Arc<Mutex<Resources>>,
    ) -> Self {
        let mut job = Self {
            rid,
            configuration,
            res,
            tasks: vec![],
        };
        job.update_tasks();
        log::debug!("Created new behavior instance job: {:?}", job.configuration);
        job
    }

    pub fn update_configuration(&mut self, configuration: BehaviorInstanceConfiguration) {
        self.configuration = configuration;
        for task in &self.tasks {
            task.abort();
        }
        self.update_tasks();
    }

    fn update_tasks(&mut self) {
        let futures = match &self.configuration {
            BehaviorInstanceConfiguration::Wakeup(wakeup_configuration) => {
                self.create_wake_up_tasks(&wakeup_configuration)
            }
        };
        self.tasks = futures.into_iter().map(|job| spawn(job.create())).collect();
    }

    fn create_wake_up_tasks(&self, configuration: &WakeupConfiguration) -> Vec<WakeupJob> {
        let weekdays = configuration.when.recurrence_days.as_ref();

        let schedule_types: Box<dyn Iterator<Item = ScheduleType>> = weekdays.map_or_else(
            || Box::new(iter::once(ScheduleType::Once())) as Box<dyn Iterator<Item = ScheduleType>>,
            |weekdays| Box::new(weekdays.iter().copied().map(ScheduleType::Recurring)),
        );
        schedule_types
            .map(|schedule_type| WakeupJob {
                rid: self.rid.clone(),
                schedule_type,
                configuration: configuration.clone(),
                res: self.res.clone(),
            })
            .collect()
    }
}

impl Drop for BehaviorInstanceJob {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

pub async fn disable_behavior_instance(id: Uuid, res: Arc<Mutex<Resources>>) {
    let upd = BehaviorInstanceUpdate::default().with_enabled(false);
    let upd_result = res
        .lock()
        .await
        .update::<BehaviorInstance>(&id, |bi| *bi += upd);
    if let Err(err) = upd_result {
        log::error!("Failed to disable behavior instance {:?}", err);
    }
}

#[derive(Debug)]
pub enum ScheduleType {
    Recurring(Weekday),
    Once(),
}
