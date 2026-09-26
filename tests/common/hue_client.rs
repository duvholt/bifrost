use std::time::Duration;

use bifrost::routes::clip::V2Reply;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use hue::api::{
    BehaviorInstance, GroupedLight, Light, RType, Resource, ResourceLink, ResourceRecord, Room,
    RoomNew, Scene, Zone, ZoneNew,
};
use hue::error::{HueError, HueResult};
use hue::event::{Event, EventBlock, ObjectDelete, ObjectUpdate};
use reqwest::Client;
use serde_json::Value;
use tokio::sync::{broadcast, oneshot};
use tokio::time::timeout;

use crate::common::bridge::{TestGroupedLight, TestLight, TestRoom, TestScene, TestZone};
use crate::common::{HueClipResponse, TestError, TestResult};

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum TestHueEvent {
    Add(ResourceRecord),
    Update(ObjectUpdate),
    Delete(ObjectDelete),
    Error(),
}

pub struct HueEvents {
    receiver: broadcast::Receiver<Vec<EventBlock>>,
    pending: Vec<TestHueEvent>,
}

impl HueEvents {
    pub const fn new(receiver: broadcast::Receiver<Vec<EventBlock>>) -> Self {
        Self {
            receiver,
            pending: Vec::new(),
        }
    }

    pub async fn expect_quiet(&mut self) -> TestResult<()> {
        if !self.pending.is_empty() {
            return Err(TestError::UnexpectedHueEvents(self.pending.clone()));
        }
        match timeout(Duration::from_millis(100), async {
            self.receive_events().await
        })
        .await
        {
            Ok(events) => Err(TestError::UnexpectedHueEvents(events?)),
            Err(_) => Ok(()),
        }
    }

    async fn receive_events(&mut self) -> TestResult<Vec<TestHueEvent>> {
        let blocks = self.receiver.recv().await?;
        let mut events = Vec::new();
        for block in blocks {
            let block_events: Vec<TestHueEvent> = match block.event {
                Event::Add(add) => add.data.into_iter().map(TestHueEvent::Add).collect(),
                Event::Update(update) => {
                    update.data.into_iter().map(TestHueEvent::Update).collect()
                }
                Event::Delete(delete) => {
                    delete.data.into_iter().map(TestHueEvent::Delete).collect()
                }
                Event::Error(_error) => vec![TestHueEvent::Error()],
            };

            events.extend(block_events);
        }
        self.pending.extend(events.clone());
        Ok(events)
    }

    #[allow(clippy::suspicious_operation_groupings)]
    async fn expect_event<P>(&mut self, name: &str, predicate: P) -> TestResult<TestHueEvent>
    where
        P: Fn(&TestHueEvent) -> bool + Sync,
    {
        timeout(Duration::from_secs(2), async {
            loop {
                let position = self.pending.iter().position(&predicate);

                if let Some(position) = position {
                    return Ok(self.pending.remove(position));
                }
                self.receive_events().await?;
            }
        })
        .await
        .map_err(|_| TestError::HueEventTimeout(format!("Hue event  {name}")))?
    }

    #[allow(clippy::suspicious_operation_groupings)]
    pub async fn expect_update_resource(
        &mut self,
        resource_link: ResourceLink,
    ) -> TestResult<ObjectUpdate> {
        self.expect_event(&format!("update {resource_link:?}"), |event| match event {
            TestHueEvent::Update(update) => {
                update.id == resource_link.rid && update.rtype == resource_link.rtype
            }
            _ => false,
        })
        .await
        .map(|event| match event {
            TestHueEvent::Update(object_update) => object_update,
            _ => unreachable!("wrong event type"),
        })
    }

    pub async fn expect_update_type(&mut self, rtype: RType) -> TestResult<ObjectUpdate> {
        self.expect_event(&format!("update {rtype:?}"), |event| match event {
            TestHueEvent::Update(update) => update.rtype == rtype,
            _ => false,
        })
        .await
        .map(|event| match event {
            TestHueEvent::Update(object_update) => object_update,
            _ => unreachable!("wrong event type"),
        })
    }

    pub async fn expect_add(&mut self, rtype: RType) -> TestResult<ResourceRecord> {
        self.expect_event(&format!("add {rtype:?}"), |event| match event {
            TestHueEvent::Add(add) => add.obj.rtype() == rtype,
            _ => false,
        })
        .await
        .map(|event| match event {
            TestHueEvent::Add(add) => add,
            _ => unreachable!("wrong event type"),
        })
    }

    #[allow(clippy::suspicious_operation_groupings)]
    pub async fn expect_delete_resource(
        &mut self,
        resource_link: ResourceLink,
    ) -> TestResult<ObjectDelete> {
        self.expect_event(&format!("delete {resource_link:?}"), |event| match event {
            TestHueEvent::Delete(delete) => {
                delete.rtype == resource_link.rtype && delete.id == resource_link.rid
            }
            _ => false,
        })
        .await
        .map(|event| match event {
            TestHueEvent::Delete(delete) => delete,
            _ => unreachable!("wrong event type"),
        })
    }
}

#[derive(Clone)]
pub struct HueClient {
    base_url: String,
    events_sender: broadcast::Sender<Vec<EventBlock>>,
    http_client: Client,
}

impl HueClient {
    #[must_use]
    pub fn new(base_url: String, events_sender: broadcast::Sender<Vec<EventBlock>>) -> Self {
        Self {
            base_url,
            events_sender,
            http_client: Client::new(),
        }
    }

    #[must_use]
    pub fn subscribe_events(&self) -> HueEvents {
        HueEvents::new(self.events_sender.subscribe())
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> TestResult<T> {
        let url = format!("{}/{path}", self.base_url);
        Ok(self
            .http_client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await?)
    }

    pub async fn get_resource<T>(&self, link: ResourceLink) -> TestResult<T>
    where
        T: TryFrom<Resource, Error = HueError>,
    {
        let data = self
            .get::<HueClipResponse<ResourceRecord>>(&format!(
                "/clip/v2/resource/{}/{}",
                serde_json::to_value(link.rtype)
                    .unwrap()
                    .as_str()
                    .expect("RType serializes as a string"),
                link.rid,
            ))
            .await?
            .data;

        assert_eq!(data.len(), 1);
        Ok(data[0].obj.clone().try_into()?)
    }

    pub async fn get_resources<T>(&self, rtype: RType) -> TestResult<Vec<T>>
    where
        T: TryFrom<Resource, Error = HueError>,
    {
        let data = self
            .get::<HueClipResponse<ResourceRecord>>(&format!(
                "/clip/v2/resource/{}/",
                serde_json::to_value(rtype)
                    .unwrap()
                    .as_str()
                    .expect("RType serializes as a string"),
            ))
            .await?
            .data;

        let resources: HueResult<Vec<T>> = data
            .into_iter()
            .map(|resource| resource.obj.try_into())
            .collect();

        Ok(resources?)
    }

    pub async fn put(&self, path: &str, value: &Value) -> TestResult<()> {
        let url = format!("{}/{path}", self.base_url);
        self.http_client
            .put(url)
            .json(value)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    pub async fn post(&self, path: &str, value: &Value) -> TestResult<ResourceLink> {
        let url = format!("{}/{path}", self.base_url);
        let data = self
            .http_client
            .post(url)
            .json(value)
            .send()
            .await?
            .error_for_status()?
            .json::<V2Reply<ResourceLink>>()
            .await?
            .data;
        assert_eq!(data.len(), 1);
        Ok(data[0])
    }

    pub async fn delete(&self, path: &str) -> TestResult<ResourceLink> {
        let url = format!("{}/{path}", self.base_url);
        let data = self
            .http_client
            .delete(url)
            .send()
            .await?
            .error_for_status()?
            .json::<V2Reply<ResourceLink>>()
            .await?
            .data;
        assert_eq!(data.len(), 1);
        Ok(data[0])
    }

    pub async fn get_lights(&self) -> TestResult<Vec<Light>> {
        Ok(self
            .get_resources::<Box<Light>>(RType::Light)
            .await?
            .into_iter()
            .map(|l| *l)
            .collect())
    }

    pub async fn get_light(&self, test_light: &TestLight) -> TestResult<Light> {
        Ok(*self.get_resource::<Box<Light>>(test_light.link).await?)
    }

    pub async fn put_light(&self, light: &TestLight, value: &Value) -> TestResult<()> {
        self.put(
            &format!("/clip/v2/resource/light/{}", light.link.rid),
            value,
        )
        .await
    }

    pub async fn get_grouped_lights(&self) -> TestResult<Vec<GroupedLight>> {
        self.get_resources::<GroupedLight>(RType::GroupedLight)
            .await
    }

    pub async fn get_grouped_light(
        &self,
        light: &TestGroupedLight<'_>,
    ) -> TestResult<GroupedLight> {
        self.get_resource::<GroupedLight>(light.link).await
    }

    pub async fn put_grouped_light(
        &self,
        light: &TestGroupedLight<'_>,
        value: &Value,
    ) -> TestResult<()> {
        self.put(
            &format!("/clip/v2/resource/grouped_light/{}", light.link.rid),
            value,
        )
        .await
    }

    pub async fn get_scene(&self, scene: &TestScene) -> TestResult<Scene> {
        self.get_resource::<Scene>(scene.link).await
    }

    pub async fn get_scenes(&self) -> TestResult<Vec<Scene>> {
        self.get_resources::<Scene>(RType::Scene).await
    }

    pub async fn get_rooms(&self) -> TestResult<Vec<Room>> {
        self.get_resources::<Room>(RType::Room).await
    }

    pub async fn get_room(&self, room: &TestRoom<'_>) -> TestResult<Room> {
        self.get_resource::<Room>(room.link).await
    }

    pub async fn post_room(&self, room: RoomNew) -> TestResult<ResourceLink> {
        self.post("/clip/v2/resource/room", &serde_json::to_value(room)?)
            .await
    }

    pub async fn delete_room(&self, room: &TestRoom<'_>) -> TestResult<ResourceLink> {
        self.delete(&format!("/clip/v2/resource/room/{}", room.link.rid))
            .await
    }

    pub async fn put_room(&self, room: &TestRoom<'_>, value: &Value) -> TestResult<()> {
        self.put(&format!("/clip/v2/resource/room/{}", room.link.rid), value)
            .await
    }

    pub async fn get_zones(&self) -> TestResult<Vec<Zone>> {
        self.get_resources::<Zone>(RType::Zone).await
    }

    pub async fn get_zone(&self, zone: &TestZone<'_>) -> TestResult<Zone> {
        self.get_resource::<Zone>(zone.link).await
    }

    pub async fn post_zone(&self, zone: ZoneNew) -> TestResult<ResourceLink> {
        self.post("/clip/v2/resource/zone", &serde_json::to_value(zone)?)
            .await
    }

    pub async fn delete_zone(&self, zone: &TestZone<'_>) -> TestResult<ResourceLink> {
        self.delete(&format!("/clip/v2/resource/zone/{}", zone.link.rid))
            .await
    }

    pub async fn put_zone(&self, zone: &TestZone<'_>, value: &Value) -> TestResult<()> {
        self.put(&format!("/clip/v2/resource/zone/{}", zone.link.rid), value)
            .await
    }

    pub async fn get_behavior_instance(&self, link: ResourceLink) -> TestResult<BehaviorInstance> {
        self.get_resource::<BehaviorInstance>(link).await
    }

    pub async fn run_evenstream(&self, ready_tx: oneshot::Sender<()>) -> TestResult<()> {
        let base_url = &self.base_url;
        let response = self
            .http_client
            .get(format!("{base_url}/eventstream/clip/v2"))
            .send()
            .await?
            .error_for_status()?;

        let _ = ready_tx.send(());
        let mut event_stream = response.bytes_stream().eventsource();

        while let Some(message) = event_stream.next().await {
            let event = message?;
            if event.data.is_empty() {
                continue;
            }

            let blocks: Vec<EventBlock> = serde_json::from_str(&event.data)?;

            self.events_sender.send(blocks)?;
        }

        Ok(())
    }
}
