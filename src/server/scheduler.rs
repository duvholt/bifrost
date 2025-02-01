use std::sync::Arc;

use chrono::Utc;
use tokio::{spawn, sync::Mutex, task::JoinHandle};
use tokio_schedule::{every, Job};

use hue::api::{
    BehaviorInstance, BehaviorInstanceConfiguration, GroupedLightUpdate, On, RType, Resource,
    WakeupConfiguration,
};

use crate::{backend::BackendRequest, resource::Resources};

#[derive(Debug)]
pub struct Scheduler {
    jobs: Vec<JoinHandle<()>>,
    res: Arc<Mutex<Resources>>,
    behavior_instances: Vec<BehaviorInstance>,
}

impl Scheduler {
    pub const fn new(res: Arc<Mutex<Resources>>) -> Self {
        Self {
            jobs: vec![],
            behavior_instances: vec![],
            res,
        }
    }

    pub async fn update(&mut self) {
        let new_behavior_instances = self.get_behavior_instances().await;
        if new_behavior_instances != self.behavior_instances {
            self.behavior_instances = new_behavior_instances;
            self.update_jobs();
        }
    }

    fn update_jobs(&mut self) {
        for job in &self.jobs {
            job.abort();
        }
        self.jobs = self
            .behavior_instances
            .iter()
            .filter(|bi| bi.enabled)
            .map(|bi| match &bi.configuration {
                BehaviorInstanceConfiguration::Wakeup(wakeup_configuration) => {
                    // todo: weekday
                    // todo: timezone
                    // todo: everything
                    let time = &wakeup_configuration.when.time_point.time;
                    let config = wakeup_configuration.clone();
                    let res = self.res.clone();
                    let schedule = every(1)
                        .day()
                        .at(time.hour, time.minute, 00)
                        .in_timezone(&Utc);
                    log::debug!("Created new behavior instance schedule: {:#?}", schedule);
                    spawn(schedule.perform(move || run_wake_up(config.clone(), res.clone())))
                }
            })
            .collect();
    }

    async fn get_behavior_instances(&self) -> Vec<BehaviorInstance> {
        self.res
            .lock()
            .await
            .get_resources_by_type(RType::BehaviorInstance)
            .into_iter()
            .filter_map(|r| match r.obj {
                Resource::BehaviorInstance(behavior_instance) => Some(behavior_instance),
                _ => None,
            })
            .collect()
    }
}

async fn run_wake_up(config: WakeupConfiguration, res: Arc<Mutex<Resources>>) {
    log::debug!("Running scheduled behavior instance:, {:#?}", config);
    let lock = res.lock().await;
    let group = &config.where_field[0].group;
    if let Ok(resource) = lock.get_resource_by_id(&group.rid) {
        log::debug!("Turning on {:#?}", resource.obj);
        if let Resource::Room(room) = resource.obj {
            if let Some(grouped_light) = room.grouped_light_service() {
                let payload = GroupedLightUpdate::default().with_on(Some(On::new(true)));

                if let Err(err) = lock
                    .backend_request(BackendRequest::GroupedLightUpdate(*grouped_light, payload))
                {
                    log::error!("Failed to execute group update: {:#?}", err);
                }
            }
        }
    }
}
