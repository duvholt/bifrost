use std::sync::Arc;

use chrono::Local;
use tokio::{spawn, sync::Mutex, task::JoinHandle};
use tokio_schedule::{every, EveryWeekDay, Job};

use crate::{
    backend::BackendRequest,
    hue::api::{
        BehaviorInstance, BehaviorInstanceConfiguration, GroupedLightUpdate, On, Resource,
        WakeupConfiguration,
    },
    resource::Resources,
};

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
            .flat_map(|bi| match &bi.configuration {
                BehaviorInstanceConfiguration::Wakeup(wakeup_configuration) => {
                    let jobs = create_wake_up_jobs(wakeup_configuration);
                    jobs.into_iter().map(|job| {
                        log::debug!("Created new behavior instance job: {:#?}", job);
                        let res = self.res.clone();
                        let config = wakeup_configuration.clone();
                        spawn(job.perform(move || run_wake_up(config.clone(), res.clone())))
                    })
                }
            })
            .collect();
    }

    async fn get_behavior_instances(&self) -> Vec<BehaviorInstance> {
        self.res
            .lock()
            .await
            .get_resources_by_type(crate::hue::api::RType::BehaviorInstance)
            .into_iter()
            .filter_map(|r| match r.obj {
                Resource::BehaviorInstance(behavior_instance) => Some(behavior_instance),
                _ => None,
            })
            .collect()
    }
}

fn create_wake_up_jobs(configuration: &WakeupConfiguration) -> Vec<EveryWeekDay<Local, Local>> {
    // todo:
    // timezone
    // non repeating
    // specific lights
    // style
    // brightness
    // turn lights off
    // fade duration

    let time = &configuration.when.time_point.time;
    configuration
        .when
        .weekdays()
        .into_iter()
        .map(|weekday| every(1).week().on(weekday).at(time.hour, time.minute, 0))
        .collect()
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
