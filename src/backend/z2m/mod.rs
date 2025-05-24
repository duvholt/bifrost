mod backend_event;
mod bridge_event;
mod bridge_import;
pub mod entertainment;
pub mod learn;
pub mod websocket;
pub mod zclcommand;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use native_tls::TlsConnector;
use tokio::select;
use tokio::sync::broadcast::Receiver;
use tokio::sync::{Mutex, mpsc};
use tokio::time::sleep;
use tokio_tungstenite::{Connector, connect_async_tls_with_config};

use bifrost_api::backend::BackendRequest;
use hue::api::ResourceLink;
use z2m::update::DeviceUpdate;

use crate::backend::Backend;
use crate::backend::z2m::entertainment::EntStream;
use crate::backend::z2m::learn::SceneLearn;
use crate::backend::z2m::websocket::Z2mWebSocket;
use crate::config::{AppConfig, Z2mServer};
use crate::error::{ApiError, ApiResult};
use crate::model::throttle::Throttle;
use crate::resource::Resources;

pub struct Z2mBackend {
    name: String,
    server: Z2mServer,
    config: Arc<AppConfig>,
    state: Arc<Mutex<Resources>>,
    map: HashMap<String, ResourceLink>,
    rmap: HashMap<ResourceLink, String>,
    learner: SceneLearn,
    ignore: HashSet<String>,
    network: HashMap<String, z2m::api::Device>,
    entstream: Option<EntStream>,
    counter: u32,
    fps: u32,
    throttle: Throttle,

    // for sending delayed messages over the websocket
    message_rx: mpsc::UnboundedReceiver<(String, DeviceUpdate)>,
    message_tx: mpsc::UnboundedSender<(String, DeviceUpdate)>,
}

impl Z2mBackend {
    const DEFAULT_FPS: u32 = 20;
    const LIGHT_BREATHE_DURATION: Duration = Duration::from_secs(2);

    pub fn new(
        name: String,
        server: Z2mServer,
        config: Arc<AppConfig>,
        state: Arc<Mutex<Resources>>,
    ) -> ApiResult<Self> {
        let fps = server.streaming_fps.map_or(Self::DEFAULT_FPS, u32::from);
        let map = HashMap::new();
        let rmap = HashMap::new();
        let ignore = HashSet::new();
        let learner = SceneLearn::new(name.clone());
        let network = HashMap::new();
        let entstream = None;
        let throttle = Throttle::from_fps(fps);
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        Ok(Self {
            name,
            server,
            config,
            state,
            map,
            rmap,
            learner,
            ignore,
            network,
            entstream,
            throttle,
            fps,
            message_rx,
            message_tx,
            counter: 0,
        })
    }

    pub async fn event_loop(
        &mut self,
        chan: &mut Receiver<Arc<BackendRequest>>,
        mut socket: Z2mWebSocket,
    ) -> ApiResult<()> {
        loop {
            select! {
                // all backend event handling implemented in backend::z2m::backend_event
                pkt = chan.recv() => {
                    let api_req = pkt?;
                    self.handle_backend_event(&mut socket, api_req).await?;
                    // FIXME: this used to be our "throttle" feature, but it breaks entertainment mode
                    /* tokio::time::sleep(std::time::Duration::from_millis(100)).await; */
                },

                // all bridge event handling implemented in backend::z2m::bridge_event
                pkt = socket.next() => {
                    self.handle_bridge_event(pkt.ok_or(ApiError::UnexpectedZ2mEof)??).await?;
                },

                Some((topic, upd)) = self.message_rx.recv() => {
                    socket.send_update(&topic, &upd).await?;
                }
            };
        }
    }
}

#[async_trait]
impl Backend for Z2mBackend {
    async fn run_forever(mut self, mut chan: Receiver<Arc<BackendRequest>>) -> ApiResult<()> {
        // let's not include auth tokens in log output
        let sanitized_url = self.server.get_sanitized_url();
        let url = self.server.get_url();

        if url != self.server.url {
            log::info!(
                "[{}] Rewrote url for compatibility with z2m 2.x.",
                self.name
            );
            log::info!(
                "[{}] Consider updating websocket url to {}",
                self.name,
                sanitized_url
            );
        }

        // if tls verification is disabled, build a TlsConnector that explicitly
        // does not check certificate validity. This is obviously neither safe
        // nor recommended.
        let connector = if self.server.disable_tls_verify.unwrap_or_default() {
            log::warn!(
                "[{}] TLS verification disabled; will accept any certificate!",
                self.name
            );
            Some(Connector::NativeTls(
                TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .build()?,
            ))
        } else {
            None
        };

        loop {
            log::info!("[{}] Connecting to {}", self.name, &sanitized_url);
            match connect_async_tls_with_config(url.as_str(), None, false, connector.clone()).await
            {
                Ok((socket, _)) => {
                    let z2m_socket = Z2mWebSocket::new(self.name.clone(), socket);
                    let res = self.event_loop(&mut chan, z2m_socket).await;
                    if let Err(err) = res {
                        log::error!("[{}] Event loop broke: {err}", self.name);
                    }
                }
                Err(err) => {
                    log::error!("[{}] Connect failed: {err:?}", self.name);
                }
            }
            sleep(Duration::from_millis(2000)).await;
        }
    }
}
