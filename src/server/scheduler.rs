use std::sync::Arc;

use chrono::Local;
use tokio::{spawn, sync::Mutex, task::JoinHandle};
use tokio_schedule::{every, EveryWeekDay, Job};

use hue::api::{
    BehaviorInstance, BehaviorInstanceConfiguration, GroupedLightUpdate, LightUpdate, On, RType,
    Resource, Room, WakeupConfiguration,
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
            .get_resources_by_type(RType::BehaviorInstance)
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
    // style
    // turn lights off

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
    #[allow(clippy::option_if_let_else)]
    let resource_links = config.where_field.iter().flat_map(|room| {
        if let Some(items) = &room.items {
            items.clone()
        } else {
            vec![room.group]
        }
    });

    for resource_link in resource_links {
        let resource = res.lock().await.get_resource_by_id(&resource_link.rid);
        let resource = match resource {
            Ok(resource) => resource,
            Err(err) => {
                log::warn!("Failed to get resource: {}", err);
                continue;
            }
        };
        log::debug!("Turning on {:#?}", resource.obj);
        match resource.obj {
            Resource::Room(room) => {
                wakeup_room(room, res.clone(), config.clone()).await;
            }
            Resource::Light(_light) => {
                let payload = LightUpdate::default()
                    .with_on(Some(On::new(true)))
                    .with_brightness(Some(config.end_brightness))
                    .with_transition(Some(config.fade_in_duration.seconds));

                let upd = res
                    .lock()
                    .await
                    .backend_request(BackendRequest::LightUpdate(resource_link, payload));
                if let Err(err) = upd {
                    log::error!("Failed to execute group update: {:#?}", err);
                }
            }
            Resource::BridgeHome(_bridge_home) => {
                let rooms = res.lock().await.get_resources_by_type(RType::Room);
                for room_resource in rooms {
                    if let Resource::Room(room) = room_resource.obj {
                        wakeup_room(room, res.clone(), config.clone()).await;
                    }
                }
            }
            _ => (),
        }
    }
}

async fn wakeup_room(room: Room, res: Arc<Mutex<Resources>>, config: WakeupConfiguration) {
    let Some(grouped_light) = room.grouped_light_service() else {
        log::error!("Failed to get grouped light service for room");
        return;
    };
    let payload = GroupedLightUpdate::default()
        .with_on(Some(On::new(true)))
        .with_brightness(Some(config.end_brightness))
        .with_transition(Some(config.fade_in_duration.seconds));

    let upd = res
        .lock()
        .await
        .backend_request(BackendRequest::GroupedLightUpdate(*grouped_light, payload));
    if let Err(err) = upd {
        log::error!("Failed to execute group update: {:#?}", err);
    }
}
