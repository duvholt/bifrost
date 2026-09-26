use std::collections::BTreeMap;
use std::net::Ipv4Addr;
use std::time::Duration;

use bifrost::config::Memory2MqttConfig;
use memory2mqtt::service::{M2mMode, Memory2MqttService};
use serde_json::Value;
use tokio::{sync::broadcast, time::timeout};
use z2m::api::RawMessage;

use crate::common::{TestError, TestResult, bridge::TestZ2mDeviceOrGroup};

const TIMEOUT: Duration = Duration::from_secs(2);

pub struct Z2mRequests {
    receiver: broadcast::Receiver<RawMessage>,
}

impl Z2mRequests {
    const fn new(receiver: broadcast::Receiver<RawMessage>) -> Self {
        Self { receiver }
    }
}

impl Z2mRequests {
    pub async fn next_request(&mut self) -> TestResult<RawMessage> {
        Ok(timeout(TIMEOUT, self.receiver.recv())
            .await
            .map_err(|_| TestError::Z2mRequestTimeout)??)
    }

    pub async fn expect_quiet(&mut self) -> TestResult<()> {
        match timeout(Duration::from_millis(100), async {
            self.next_request().await
        })
        .await
        {
            Ok(event) => Err(TestError::UnexpectedZ2mEvent(event?)),
            Err(_) => Ok(()),
        }
    }

    pub async fn expect_set(
        &mut self,
        device: &(impl TestZ2mDeviceOrGroup + Sync),
        payload: Value,
    ) -> TestResult<()> {
        self.expect_request(&format!("{}/set", device.topic()), payload)
            .await
    }

    pub async fn expect_request(&mut self, topic: &str, payload: Value) -> TestResult<()> {
        self.expect_requests_unordered([(topic, payload)]).await
    }

    pub async fn expect_requests_unordered<I, S>(&mut self, expected: I) -> TestResult<()>
    where
        I: IntoIterator<Item = (S, Value)>,
        S: Into<String>,
    {
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(topic, payload)| RawMessage {
                topic: topic.into(),
                payload,
            })
            .collect();
        let actual = timeout(TIMEOUT, async {
            let mut requests = Vec::with_capacity(expected.len());
            for _ in 0..expected.len() {
                requests.push(self.next_request().await?);
            }
            Ok::<_, TestError>(requests)
        })
        .await
        .map_err(|_| TestError::Z2mRequestTimeout)??;

        if unordered_eq(&expected, &actual) {
            Ok(())
        } else {
            Err(TestError::UnexpectedZ2mRequests { expected, actual })
        }
    }
}

pub struct TestZ2mBackend {
    websocket_tx: broadcast::Sender<RawMessage>,
    observed_requests_rx: broadcast::Receiver<RawMessage>,
}

pub struct TestZ2m {
    backends: BTreeMap<String, TestZ2mBackend>,
}

pub async fn create_m2m_service(
    state: BTreeMap<String, Value>,
    mode: M2mMode,
) -> TestResult<Memory2MqttService> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let address = listener.local_addr()?;
    let config = Memory2MqttConfig {
        listen: address,
        state,
    };

    let service = Memory2MqttService::new(config)
        .with_listener(listener)
        .with_mode(mode);
    Ok(service)
}

impl TestZ2mBackend {
    #[must_use]
    pub fn from_service(service: &Memory2MqttService) -> Self {
        Self {
            websocket_tx: service.websocket_sender(),
            observed_requests_rx: service.subscribe_requests(),
        }
    }

    #[must_use]
    pub fn subscribe_requests(&self) -> Z2mRequests {
        Z2mRequests::new(self.observed_requests_rx.resubscribe())
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn publish(&self, device: &impl TestZ2mDeviceOrGroup, payload: Value) -> TestResult<()> {
        self.publish_topic(device.topic(), payload)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn publish_topic(&self, topic: String, payload: Value) -> TestResult<()> {
        self.websocket_tx.send(RawMessage { topic, payload })?;
        Ok(())
    }
}

impl TestZ2m {
    #[must_use]
    pub fn from_services<'a>(
        services: impl IntoIterator<Item = (&'a String, &'a Memory2MqttService)>,
    ) -> Self {
        Self {
            backends: services
                .into_iter()
                .map(|(name, service)| (name.clone(), TestZ2mBackend::from_service(service)))
                .collect(),
        }
    }

    pub fn backend(&self, name: &str) -> TestResult<&TestZ2mBackend> {
        self.backends
            .get(name)
            .ok_or_else(|| TestError::Z2mBackendNotFound(name.to_string()))
    }

    #[must_use]
    pub fn only_backend(&self) -> &TestZ2mBackend {
        assert_eq!(
            self.backends.len(),
            1,
            "only_backend requires a single backend to be configured"
        );
        self.backends.first_key_value().unwrap().1
    }

    #[must_use]
    pub fn subscribe_requests(&self) -> Z2mRequests {
        self.only_backend().subscribe_requests()
    }

    pub fn publish(&self, device: &impl TestZ2mDeviceOrGroup, payload: Value) -> TestResult<()> {
        self.only_backend().publish(device, payload)
    }

    pub fn publish_topic(&self, topic: String, payload: Value) -> TestResult<()> {
        self.only_backend().publish_topic(topic, payload)
    }
}

fn unordered_eq<T: PartialEq>(left: &[T], right: &[T]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    let mut unmatched: Vec<_> = right.iter().collect();

    for item in left {
        let Some(index) = unmatched.iter().position(|candidate| *candidate == item) else {
            return false;
        };

        unmatched.swap_remove(index);
    }

    true
}
