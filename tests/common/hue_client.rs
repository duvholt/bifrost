use std::time::Duration;

use eventsource_stream::Eventsource;
use futures::StreamExt;
use hue::{
    api::{Light, ResourceRecord},
    event::{Event, EventBlock, ObjectDelete, ObjectUpdate},
};
use reqwest::Client;
use serde_json::Value;
use tokio::sync::{broadcast, oneshot};
use tokio::time::timeout;

use crate::common::bridge::{TestLight, TestResource};
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
    pub async fn expect_update(
        &mut self,
        resource: &(impl TestResource + Sync),
    ) -> TestResult<ObjectUpdate> {
        let resource_link = resource.link();
        timeout(Duration::from_secs(2), async {
            loop {
                let position = self.pending.iter().position(|event| match event {
                    TestHueEvent::Update(update) => {
                        update.id == resource_link.rid && update.rtype == resource_link.rtype
                    }
                    _ => false,
                });

                if let Some(position) = position
                    && let TestHueEvent::Update(update) = self.pending.remove(position)
                {
                    return Ok(update);
                }
                self.receive_events().await?;
            }
        })
        .await
        .map_err(|_| TestError::HueEventTimeout(format!("update {resource_link:?}")))?
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
        Ok(self.http_client.get(url).send().await?.json::<T>().await?)
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

    pub async fn get_lights(&self) -> TestResult<HueClipResponse<ResourceRecord>> {
        self.get::<HueClipResponse<ResourceRecord>>("/clip/v2/resource/light")
            .await
    }

    pub async fn get_light(&self, light: &TestLight) -> TestResult<Light> {
        let data = self
            .get::<HueClipResponse<ResourceRecord>>(&format!(
                "/clip/v2/resource/light/{}",
                light.link.rid,
            ))
            .await?
            .data;

        let lights: Vec<_> = data
            .into_iter()
            .map(|r| match r.obj {
                hue::api::Resource::Light(light) => light,
                _ => panic!("expected light resource"),
            })
            .collect();
        assert_eq!(lights.len(), 1);
        Ok(*lights[0].clone())
    }

    pub async fn put_light(&self, light: &TestLight, value: &Value) -> TestResult<()> {
        self.put(
            &format!("/clip/v2/resource/light/{}", light.link.rid),
            value,
        )
        .await
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
