use bifrost::backend::z2m::Z2mServiceTemplate;
use bifrost::config::AppConfig;
use bifrost::server::{self, Protocol, appstate::AppState, http::HttpServer};
use hue::api::{RType, ResourceRecord};
use hue::event::{Event, EventBlock, ObjectUpdate};
use memory2mqtt::service::M2mMode;
use serde_json::Value;
use serde_json::json;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::time::Duration;
use std::{collections::BTreeMap, net::TcpListener, path::PathBuf};
use svc::error::SvcResult;
use svc::manager::{ServiceManager, SvmClient};
use svc::serviceid::ServiceId;
use tokio::sync::broadcast::{self, Receiver};
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};
use tokio::time::timeout;
use url::Url;
use uuid::Uuid;

use crate::common::{HueClient, TestResult, TestZ2m, create_m2m_service};

pub struct TestBridge {
    pub hue_url: Url,
    manager_future: JoinHandle<SvcResult<()>>,
    tasks: JoinSet<TestResult<()>>,
    workdir: PathBuf,
    pub hue_client: HueClient,
    pub z2m: TestZ2m,
    events: Receiver<Vec<EventBlock>>,
}

impl Drop for TestBridge {
    fn drop(&mut self) {
        self.tasks.abort_all();
        self.manager_future.abort();
        let _ = std::fs::remove_dir_all(&self.workdir);
    }
}

impl TestBridge {
    pub async fn start(z2m_state: BTreeMap<String, Value>) -> TestResult<Self> {
        Self::start_with_z2m_mode(z2m_state, M2mMode::Automatic).await
    }

    pub async fn start_with_z2m_mode(
        z2m_state: BTreeMap<String, Value>,
        mode: M2mMode,
    ) -> TestResult<Self> {
        let workdir = Self::create_workdir()?;
        let mut tasks = JoinSet::new();

        let http_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let http_address = http_listener.local_addr()?;

        let memory2mqtt_service = create_m2m_service(z2m_state, mode).await?;
        let test_z2m = TestZ2m::from_service(&memory2mqtt_service);

        let config =
            Self::create_appconfig(http_address, memory2mqtt_service.config.listen, &workdir)?;
        let (svc_manager, manager_future) = ServiceManager::spawn();
        let appstate = AppState::from_config(config, svc_manager).await?;
        let mut mgr = appstate.manager();

        // memory2mqtt
        mgr.register_service("m2m", memory2mqtt_service).await?;

        // bifrost http
        let http_service = HttpServer::http_listener(
            http_address,
            http_listener,
            server::build_service(Protocol::Http, appstate.clone()),
        );
        mgr.register_service("http", http_service).await?;

        Self::start_services(&mut mgr).await?;

        let api_base_url = format!("http://{http_address}/");

        let (events_sender, events_receiver) = broadcast::channel(128);
        let hue_client = HueClient::new(api_base_url.clone(), events_sender);

        // zigbee2mqtt backend
        let template = Z2mServiceTemplate::new(appstate.clone());
        mgr.register_template("z2m", template).await?;
        Self::setup_z2m(&appstate, &mut mgr, hue_client.clone(), &mut tasks).await?;

        Ok(Self {
            hue_url: Url::parse(&api_base_url)?,
            manager_future,
            tasks,
            workdir,
            hue_client,
            z2m: test_z2m,
            events: events_receiver,
        })
    }

    fn create_workdir() -> TestResult<PathBuf> {
        let path = std::env::temp_dir().join(format!("bifrost-integration-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    async fn start_services(mgr: &mut SvmClient) -> TestResult<()> {
        for (id, _name) in mgr.list().await? {
            mgr.start(id).await?;
        }
        Ok(())
    }

    async fn setup_z2m(
        appstate: &AppState,
        mgr: &mut SvmClient,
        hue_client: HueClient,
        tasks: &mut JoinSet<TestResult<()>>,
    ) -> TestResult<()> {
        let (ready_tx, ready_rx) = oneshot::channel();

        let z2m_servers = appstate.config().z2m.servers.clone();
        tasks.spawn(async move { hue_client.run_evenstream(ready_tx).await });

        ready_rx.await?;
        for name in z2m_servers.keys() {
            // mgr.start(ServiceId::instance("z2m", name)).await?;
            let id = mgr.start(ServiceId::instance("z2m", name)).await?;
            mgr.start(id).await?;
        }
        Ok(())
    }

    fn create_appconfig(
        http_address: SocketAddr,
        z2m_address: SocketAddr,
        workdir: &std::path::Path,
    ) -> TestResult<AppConfig> {
        Ok(serde_json::from_value(json!({
            "bridge": {
                "name": "Bifrost integration test",
                "mac": "11:22:33:44:55:66",
                "ipaddress": http_address.ip(),
                "http_port": http_address.port(),
                "https_port": 0,
                "entm_port": 0,
                "netmask": "255.0.0.0",
                "gateway": http_address.ip(),
                "timezone": "Etc/UTC"
            },
            "z2m": {
                "test": {"url": format!("ws://{z2m_address}/api")}
            },
            "bifrost": {
                "state_file": workdir.join("state.yaml"),
                "cert_file": workdir.join("cert.pem")
            }
        }))?)
    }

    pub fn clear_events(&mut self) {
        self.events = self.events.resubscribe();
    }

    pub async fn wait_for_event_add(&mut self, rtype: RType) -> TestResult<ResourceRecord> {
        timeout(Duration::from_secs(2), async {
            loop {
                let blocks = self.events.recv().await?;
                for block in blocks {
                    let Event::Add(add) = block.event else {
                        continue;
                    };
                    if let Some(object) = add
                        .data
                        .into_iter()
                        .find(|record| record.obj.rtype() == rtype)
                    {
                        return Ok(object);
                    }
                }
            }
        })
        .await?
    }

    pub async fn wait_for_event_update(&mut self, id: Uuid) -> TestResult<ObjectUpdate> {
        timeout(Duration::from_secs(2), async {
            loop {
                let blocks = self.events.recv().await?;
                for block in blocks {
                    let Event::Update(update) = block.event else {
                        continue;
                    };

                    if let Some(object) = update.data.into_iter().find(|object| object.id == id) {
                        return Ok(object);
                    }
                }
            }
        })
        .await?
    }
}
