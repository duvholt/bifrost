use std::{sync::Arc, time::Duration};

use bifrost_api::backend::BackendRequest;
use chrono::{Days, Local, NaiveTime, Timelike, Weekday};
use futures::{StreamExt, stream};
use tokio::{spawn, sync::Mutex, task::JoinHandle, time::sleep};
use tokio_schedule::{Job, every};

use hue::api::{
    BehaviorInstance, BehaviorInstanceConfiguration, BehaviorInstanceUpdate, BehaviorScript,
    GroupedLightDynamicsUpdate, GroupedLightUpdate, LightDynamicsUpdate, LightUpdate, On, RType,
    Resource, Room, WakeupConfiguration,
};
use uuid::Uuid;

use crate::resource::Resources;

#[derive(Debug)]
pub struct Scheduler {
    jobs: Vec<JoinHandle<()>>,
    res: Arc<Mutex<Resources>>,
    behavior_instances: Vec<ScheduleBehaviorInstance>,
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
            .filter(|ScheduleBehaviorInstance(_, bi)| bi.enabled)
            .filter_map(|ScheduleBehaviorInstance(id, bi)| match bi.script_id {
                BehaviorScript::WAKE_UP_ID => {
                    match serde_json::from_value::<WakeupConfiguration>(bi.configuration.clone()) {
                        Ok(config) => Some((id, BehaviorInstanceConfiguration::Wakeup(config))),
                        Err(err) => {
                            log::error!(
                                "Failed to parse behavior instance {}: {}",
                                bi.configuration,
                                err
                            );
                            None
                        }
                    }
                }
                _ => None,
            })
            .flat_map(|(id, configuration)| match &configuration {
                BehaviorInstanceConfiguration::Wakeup(wakeup_configuration) => {
                    wakeup(self.res.clone(), *id, wakeup_configuration.clone())
                }
            })
            .collect();
    }

    async fn get_behavior_instances(&self) -> Vec<ScheduleBehaviorInstance> {
        self.res
            .lock()
            .await
            .get_resources_by_type(RType::BehaviorInstance)
            .into_iter()
            .filter_map(|r| match r.obj {
                Resource::BehaviorInstance(behavior_instance) => {
                    Some(ScheduleBehaviorInstance(r.id, behavior_instance))
                }
                _ => None,
            })
            .collect()
    }
}

fn wakeup(
    res: Arc<Mutex<Resources>>,
    id: Uuid,
    wakeup_configuration: WakeupConfiguration,
) -> Vec<JoinHandle<()>> {
    let jobs = create_wake_up_jobs(&wakeup_configuration);
    jobs.into_iter()
        .map(move |job| spawn(job.run(id, wakeup_configuration.clone(), res.clone())))
        .collect()
}

async fn disable_behavior_instance(id: Uuid, res: Arc<Mutex<Resources>>) {
    let upd = BehaviorInstanceUpdate::default().with_enabled(false);
    let upd_result = res
        .lock()
        .await
        .update::<BehaviorInstance>(&id, |bi| *bi += upd);
    if let Err(err) = upd_result {
        log::error!("Failed to disable behavior instance {:?}", err);
    }
}

#[derive(Debug, PartialEq)]
struct ScheduleBehaviorInstance(Uuid, BehaviorInstance);

#[derive(Clone, Debug)]
struct Time {
    pub hour: u32,
    pub minute: u32,
}

impl Time {
    fn get_sleep_duration(&self, now: chrono::DateTime<Local>) -> Result<Duration, &'static str> {
        let wakeup_datetime = self.next_datetime(now)?;
        let sleep_duration = (wakeup_datetime - now).to_std().ok().ok_or("duration")?;
        Ok(sleep_duration)
    }

    fn next_datetime(
        &self,
        now: chrono::DateTime<Local>,
    ) -> Result<chrono::DateTime<Local>, &'static str> {
        let naive_time = NaiveTime::from_hms_opt(self.hour, self.minute, 0).ok_or("naive time")?;
        let next = match now.with_time(naive_time) {
            chrono::offset::LocalResult::Single(time) => time,
            chrono::offset::LocalResult::Ambiguous(_, latest) => latest,
            chrono::offset::LocalResult::None => {
                return Err("with time");
            }
        };
        let wakeup_datetime = if next < now {
            next.checked_add_days(Days::new(1)).ok_or("add day")?
        } else {
            next
        };
        Ok(wakeup_datetime)
    }
}

#[derive(Debug)]
enum ScheduleJob {
    Recurring(Weekday, Time),
    Once(Time),
}

impl ScheduleJob {
    async fn run(
        self,
        id: Uuid,
        wakeup_configuration: WakeupConfiguration,
        res: Arc<Mutex<Resources>>,
    ) {
        log::debug!("Created new behavior instance job: {:?}", self);
        let now = Local::now();
        match self {
            Self::Recurring(weekday, time) => match time.next_datetime(now) {
                Ok(datetime) => {
                    let fade_in_start = datetime - wakeup_configuration.fade_in_duration.to_std();
                    every(1)
                        .week()
                        .on(weekday)
                        .at(
                            fade_in_start.hour(),
                            fade_in_start.minute(),
                            fade_in_start.second(),
                        )
                        .perform(move || run_wake_up(wakeup_configuration.clone(), res.clone()))
                        .await;
                }
                Err(err) => {
                    log::error!("Failed to get next datetime {:?}: {}", time, err);
                }
            },
            Self::Once(time) => match time.get_sleep_duration(now) {
                Ok(time_until_wakeup) => {
                    let fade_in_start =
                        time_until_wakeup - wakeup_configuration.fade_in_duration.to_std();
                    sleep(fade_in_start).await;
                    run_wake_up(wakeup_configuration.clone(), res.clone()).await;
                    disable_behavior_instance(id, res).await;
                }
                Err(err) => {
                    log::error!("Failed to get sleep duration for time {:?}: {}", time, err);
                }
            },
        }
    }
}

fn create_wake_up_jobs(configuration: &WakeupConfiguration) -> Vec<ScheduleJob> {
    // todo:
    // timezone

    let chrono_time = configuration.when.time_point.time();
    let time = Time {
        hour: chrono_time.hour,
        minute: chrono_time.minute,
    };
    let weekdays = configuration.when.recurrence_days.as_ref();

    if let Some(weekdays) = weekdays {
        weekdays
            .into_iter()
            .map(|weekday| ScheduleJob::Recurring(weekday.clone(), time.clone()))
            .collect()
    } else {
        vec![ScheduleJob::Once(time)]
    }
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

    let resources = stream::iter(resource_links.into_iter())
        .filter_map(|resource_link| {
            let res = res.clone();
            async move {
                let resource = res.lock().await.get_resource_by_id(&resource_link.rid);
                match resource {
                    Ok(resource) => Some((resource_link, resource)),
                    Err(err) => {
                        log::warn!("Failed to get resource: {}", err);
                        None
                    }
                }
            }
        })
        .collect::<Vec<_>>()
        .await;

    for (resource_link, resource) in &resources {
        log::debug!("Turning on {:#?}", resource.obj);
        match &resource.obj {
            Resource::Room(room) => {
                wakeup_room(room, res.clone(), config.clone()).await;
            }
            Resource::Light(_light) => {
                let payload = LightUpdate::default()
                    .with_on(Some(On::new(true)))
                    .with_brightness(Some(config.end_brightness))
                    .with_dynamics(Some(
                        LightDynamicsUpdate::new()
                            .with_duration(Some(config.fade_in_duration.seconds * 1000)),
                    ));

                let upd = res
                    .lock()
                    .await
                    .backend_request(BackendRequest::LightUpdate(*resource_link, payload));
                if let Err(err) = upd {
                    log::error!("Failed to execute group update: {:#?}", err);
                }
            }
            Resource::BridgeHome(_bridge_home) => {
                let rooms = res.lock().await.get_resources_by_type(RType::Room);
                for room_resource in rooms {
                    if let Resource::Room(room) = room_resource.obj {
                        wakeup_room(&room, res.clone(), config.clone()).await;
                    }
                }
            }
            _ => (),
        }
    }

    if let Some(duration) = config.turn_lights_off_after {
        sleep(config.fade_in_duration.to_std() + duration.to_std()).await;
        for (resource_link, resource) in resources {
            match resource.obj {
                Resource::Room(room) => {
                    turn_off_room(&room, res.clone()).await;
                }
                Resource::Light(_light) => {
                    let payload = LightUpdate::default().with_on(Some(On::new(false)));

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
                            turn_off_room(&room, res.clone()).await;
                        }
                    }
                }
                _ => (),
            }
        }
    }
}

async fn wakeup_room(room: &Room, res: Arc<Mutex<Resources>>, config: WakeupConfiguration) {
    let Some(grouped_light) = room.grouped_light_service() else {
        log::error!("Failed to get grouped light service for room");
        return;
    };
    let payload = GroupedLightUpdate::default()
        .with_on(Some(On::new(true)))
        .with_brightness(Some(config.end_brightness))
        .with_dynamics(Some(
            GroupedLightDynamicsUpdate::new()
                .with_duration(Some(config.fade_in_duration.seconds * 1000)),
        ));

    let upd = res
        .lock()
        .await
        .backend_request(BackendRequest::GroupedLightUpdate(*grouped_light, payload));
    if let Err(err) = upd {
        log::error!("Failed to execute group update: {:#?}", err);
    }
}

async fn turn_off_room(room: &Room, res: Arc<Mutex<Resources>>) {
    let Some(grouped_light) = room.grouped_light_service() else {
        log::error!("Failed to get grouped light service for room");
        return;
    };
    let payload = GroupedLightUpdate::default().with_on(Some(On::new(false)));

    let upd = res
        .lock()
        .await
        .backend_request(BackendRequest::GroupedLightUpdate(*grouped_light, payload));
    if let Err(err) = upd {
        log::error!("Failed to execute group update: {:#?}", err);
    }
}
