use bifrost::backend::z2m::Z2mServiceTemplate;
use bifrost::config::{AppConfig, Memory2MqttConfig};
use bifrost::server::{self, Protocol, appstate::AppState, http::HttpServer};
use memory2mqtt::service::Memory2MqttService;
use serde_json::Value;
use serde_json::json;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::{collections::BTreeMap, net::TcpListener, path::PathBuf};
use svc::error::SvcResult;
use svc::manager::ServiceManager;
use svc::serviceid::ServiceId;
use tokio::task::JoinHandle;
use url::Url;
use uuid::Uuid;

use crate::common::TestResult;

pub struct TestBridge {
    pub hue_url: Url,
    manager_future: JoinHandle<SvcResult<()>>,
    workdir: PathBuf,
}

impl Drop for TestBridge {
    fn drop(&mut self) {
        self.manager_future.abort();
        let _ = std::fs::remove_dir_all(&self.workdir);
    }
}

impl TestBridge {
    pub async fn start(z2m_state: BTreeMap<String, Value>) -> TestResult<Self> {
        let workdir = Self::create_workdir()?;

        let http_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let http_address = http_listener.local_addr()?;

        let memory2mqtt_service = Self::create_m2m_service(z2m_state).await?;

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

        // zigbee2mqtt backend
        let template = Z2mServiceTemplate::new(appstate.clone());
        mgr.register_template("z2m", template).await?;

        Self::start_services(appstate, mgr).await?;

        Ok(Self {
            hue_url: Url::parse(&format!("http://{http_address}/"))?,
            manager_future,
            workdir,
        })
    }

    fn create_workdir() -> TestResult<PathBuf> {
        let path = std::env::temp_dir().join(format!("bifrost-integration-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    async fn create_m2m_service(state: BTreeMap<String, Value>) -> TestResult<Memory2MqttService> {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let config = Memory2MqttConfig {
            listen: address,
            state,
        };
        Ok(Memory2MqttService::new(config).with_listener(listener))
    }

    async fn start_services(
        appstate: AppState,
        mut mgr: svc::manager::SvmClient,
    ) -> TestResult<()> {
        for name in appstate.config().z2m.servers.keys() {
            mgr.start(ServiceId::instance("z2m", name)).await?;
        }
        for (id, _name) in mgr.list().await? {
            appstate.manager().start(id).await?;
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
}
