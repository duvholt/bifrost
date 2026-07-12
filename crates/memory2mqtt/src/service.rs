use std::sync::Arc;

use async_trait::async_trait;
use bifrost_api::config::Memory2MqttConfig;
use futures::{SinkExt, StreamExt};
use svc::traits::Service;
use tokio::{
    net::TcpListener,
    sync::{Mutex, broadcast},
};
use tokio_tungstenite::accept_async;
use tungstenite::Message;
use z2m::api::RawMessage;

use crate::{
    error::{M2MError, M2MResult},
    handler::Memory2Mqtt,
};

pub struct Memory2MqttService {
    config: Memory2MqttConfig,
    listener: Option<TcpListener>,
    state: Arc<Mutex<Memory2Mqtt>>,
    events: broadcast::Sender<RawMessage>,
}

impl Memory2MqttService {
    #[must_use]
    pub fn new(config: Memory2MqttConfig) -> Self {
        let state = Memory2Mqtt::new(config.state.clone());
        let (events, _) = broadcast::channel(128);
        Self {
            config,
            listener: None,
            state: Arc::new(Mutex::new(state)),
            events,
        }
    }

    #[must_use]
    pub fn with_listener(mut self, listener: TcpListener) -> Self {
        self.listener = Some(listener);
        self
    }

    async fn client(
        socket: tokio::net::TcpStream,
        state: Arc<Mutex<Memory2Mqtt>>,
        events: broadcast::Sender<RawMessage>,
    ) -> M2MResult<()> {
        let socket = accept_async(socket).await?;
        let (mut write, mut read) = socket.split();
        let mut updates = events.subscribe();

        let startup_messages = state.lock().await.startup_messages();
        for message in startup_messages {
            write
                .send(Message::text(serde_json::to_string(&message)?))
                .await?;
        }

        loop {
            tokio::select! {
                incoming = read.next() => {
                    let Some(incoming) = incoming else { return Ok(()); };
                    let Message::Text(text) = incoming? else { continue; };
                    let request = serde_json::from_str::<RawMessage>(&text)?;
                    let replies = state.lock().await.handle(request);
                    for reply in replies {
                        let _ = events.send(reply);
                    }
                }
                event = updates.recv() => match event {
                    Ok(message) => write.send(Message::text(serde_json::to_string(&message)?)).await?,
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        log::warn!("Test Z2M client lagged by {count} events");
                    }
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                }
            }
        }
    }
}

#[async_trait]
impl Service for Memory2MqttService {
    type Error = M2MError;

    async fn start(&mut self) -> M2MResult<()> {
        log::info!(
            "Test Zigbee2MQTT server configured on {}",
            self.config.listen
        );
        Ok(())
    }

    async fn run(&mut self) -> M2MResult<()> {
        let listener = match self.listener.take() {
            Some(listener) => listener,
            None => TcpListener::bind(self.config.listen).await?,
        };
        log::info!(
            "Test Zigbee2MQTT server listening on {}",
            self.config.listen
        );
        loop {
            let (socket, address) = listener.accept().await?;
            let state = self.state.clone();
            let events = self.events.clone();
            tokio::spawn(async move {
                if let Err(error) = Self::client(socket, state, events).await {
                    log::debug!("Test Zigbee2MQTT client {address} disconnected: {error}");
                }
            });
        }
    }
}
