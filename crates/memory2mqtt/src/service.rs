use std::sync::Arc;

use async_trait::async_trait;
use bifrost_api::config::Memory2MqttConfig;
use futures::{SinkExt, StreamExt};
use svc::traits::Service;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, broadcast};
use tokio_tungstenite::accept_async;
use tungstenite::Message;
use z2m::api::RawMessage;

use crate::error::{M2MError, M2MResult};
use crate::handler::Memory2Mqtt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum M2mMode {
    #[default]
    Automatic,
    Manual,
}

pub struct Memory2MqttService {
    pub config: Memory2MqttConfig,
    listener: Option<TcpListener>,
    pub state: Arc<Mutex<Memory2Mqtt>>,
    websocket_tx: broadcast::Sender<RawMessage>,
    requests_tx: broadcast::Sender<RawMessage>,
    mode: M2mMode,
}

impl Memory2MqttService {
    #[must_use]
    pub fn new(config: Memory2MqttConfig) -> Self {
        let state = Memory2Mqtt::new(config.state.clone());
        let (websocket_tx, _) = broadcast::channel(128);
        let (requests_tx, _) = broadcast::channel(128);

        Self {
            config,
            listener: None,
            state: Arc::new(Mutex::new(state)),
            websocket_tx,
            requests_tx,
            mode: M2mMode::Automatic,
        }
    }

    #[must_use]
    pub fn with_listener(mut self, listener: TcpListener) -> Self {
        self.listener = Some(listener);
        self
    }

    #[must_use]
    pub const fn with_mode(mut self, mode: M2mMode) -> Self {
        self.mode = mode;
        self
    }

    #[must_use]
    pub fn subscribe_requests(&self) -> broadcast::Receiver<RawMessage> {
        self.requests_tx.subscribe()
    }

    #[must_use]
    pub fn websocket_sender(&self) -> broadcast::Sender<RawMessage> {
        self.websocket_tx.clone()
    }

    async fn client(
        socket: tokio::net::TcpStream,
        state: Arc<Mutex<Memory2Mqtt>>,
        websocket_tx: broadcast::Sender<RawMessage>,
        requests_tx: broadcast::Sender<RawMessage>,
        mode: M2mMode,
    ) -> M2MResult<()> {
        let socket = accept_async(socket).await?;
        let (mut write, mut read) = socket.split();
        let mut websocket_rx = websocket_tx.subscribe();

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
                    let _ = requests_tx.send(request.clone());
                    match mode {
                        M2mMode::Automatic => {
                            let replies = state.lock().await.handle(request);
                            for reply in replies {
                                let _ = websocket_tx.send(reply);
                            }
                        },
                        M2mMode::Manual => {},
                    }
                }
                event = websocket_rx.recv() => match event {
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
        log::info!("Memory2MQTT server configured on {}", self.config.listen);
        Ok(())
    }

    async fn run(&mut self) -> M2MResult<()> {
        let listener = match self.listener.take() {
            Some(listener) => listener,
            None => TcpListener::bind(self.config.listen).await?,
        };
        log::info!("Memory2MQTT server listening on {}", self.config.listen);
        loop {
            let (socket, address) = listener.accept().await?;
            let state = self.state.clone();
            let websocket_tx = self.websocket_tx.clone();
            let requests_tx = self.requests_tx.clone();
            let mode = self.mode;
            tokio::spawn(async move {
                if let Err(error) =
                    Self::client(socket, state, websocket_tx, requests_tx, mode).await
                {
                    log::debug!("Memory2MQTT client {address} disconnected: {error}");
                }
            });
        }
    }
}
