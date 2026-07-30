use eventsource_stream::Eventsource;
use futures::StreamExt;
use hue::{api::ResourceRecord, event::EventBlock};
use reqwest::Client;
use serde_json::Value;
use tokio::sync::{broadcast::Sender, oneshot};

use crate::common::{HueClipResponse, TestResult};

#[derive(Clone)]
pub struct HueClient {
    base_url: String,
    events_sender: Sender<Vec<EventBlock>>,
    http_client: Client,
}

impl HueClient {
    #[must_use]
    pub fn new(base_url: String, events_sender: Sender<Vec<EventBlock>>) -> Self {
        Self {
            base_url,
            events_sender,
            http_client: Client::new(),
        }
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

    pub async fn put_light(
        &self,
        light_id: impl std::fmt::Display,
        value: &Value,
    ) -> TestResult<()> {
        self.put(&format!("/clip/v2/resource/light/{light_id}"), value)
            .await
    }

    pub async fn run_evenstream(&self, ready_tx: oneshot::Sender<()>) -> TestResult<()> {
        let base_url = &self.base_url;
        let response = self
            .http_client
            .get(format!("{base_url}/eventstream/clip/v2"))
            .send()
            .await?;

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
