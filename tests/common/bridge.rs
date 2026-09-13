use bifrost::backend::z2m::Z2mServiceTemplate;
use bifrost::config::AppConfig;
use bifrost::resource::Resources;
use bifrost::server::{self, Protocol, appstate::AppState, http::HttpServer};
use hue::api::{
    Group, RType, Resource, ResourceLink, ResourceRecord, Room, ZigbeeConnectivity, Zone,
};
use hue::event::{Event, EventBlock};
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

use crate::common::fixture::{FixtureDevice, FixtureGroup};
use crate::common::{HueClient, TestError, TestResult, TestZ2m, create_m2m_service};

pub struct TestBridge {
    pub hue_url: Url,
    manager_future: JoinHandle<SvcResult<()>>,
    tasks: JoinSet<TestResult<()>>,
    workdir: PathBuf,
    pub hue_client: HueClient,
    pub z2m: TestZ2m,
    hue_events: Receiver<Vec<EventBlock>>,
    received_hue_events: Vec<Event>,
}

impl Drop for TestBridge {
    fn drop(&mut self) {
        self.tasks.abort_all();
        self.manager_future.abort();
        let _ = std::fs::remove_dir_all(&self.workdir);
    }
}

pub trait TestZ2mDeviceOrGroup {
    fn topic(&self) -> String;
}

#[derive(Clone)]
pub struct TestLight {
    pub link: ResourceLink,
    pub fixture_id: FixtureDevice,
}

#[derive(Clone)]
pub struct TestScene {
    pub link: ResourceLink,
}

impl TestZ2mDeviceOrGroup for TestLight {
    fn topic(&self) -> String {
        self.fixture_id.topic()
    }
}

#[derive(Clone)]
pub struct TestRoom<'a> {
    pub link: ResourceLink,
    pub fixture_group: FixtureGroup<'a>,
}

impl TestZ2mDeviceOrGroup for TestRoom<'_> {
    fn topic(&self) -> String {
        self.fixture_group.topic()
    }
}

#[derive(Clone)]
pub struct TestZone<'a> {
    pub link: ResourceLink,
    pub fixture_group: FixtureGroup<'a>,
}

impl TestZ2mDeviceOrGroup for TestZone<'_> {
    fn topic(&self) -> String {
        self.fixture_group.topic()
    }
}

#[derive(Clone)]
pub struct TestGroupedLight<'a> {
    pub link: ResourceLink,
    pub fixture_group: FixtureGroup<'a>,
}

impl TestZ2mDeviceOrGroup for TestGroupedLight<'_> {
    fn topic(&self) -> String {
        self.fixture_group.topic()
    }
}

impl TestBridge {
    pub async fn start(z2m_state: BTreeMap<String, Value>) -> TestResult<Self> {
        Self::start_with_seed(z2m_state, |_| Ok(())).await
    }

    pub async fn start_with_seed(
        z2m_state: BTreeMap<String, Value>,
        seed: impl FnOnce(&mut Resources) -> TestResult<()>,
    ) -> TestResult<Self> {
        Self::start_interal(BTreeMap::from([("test".to_string(), z2m_state)]), seed).await
    }

    pub async fn start_backends(
        z2m_states: BTreeMap<String, BTreeMap<String, Value>>,
    ) -> TestResult<Self> {
        Self::start_interal(z2m_states, |_| Ok(())).await
    }

    async fn start_interal(
        z2m_states: BTreeMap<String, BTreeMap<String, Value>>,
        seed: impl FnOnce(&mut Resources) -> TestResult<()>,
    ) -> TestResult<Self> {
        let workdir = Self::create_workdir()?;
        let mut tasks = JoinSet::new();

        let http_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let http_address = http_listener.local_addr()?;

        let mut memory2mqtt_services = BTreeMap::new();
        for (name, state) in z2m_states {
            memory2mqtt_services.insert(name, create_m2m_service(state, M2mMode::Manual).await?);
        }
        let test_z2m = TestZ2m::from_services(memory2mqtt_services.iter());
        let z2m_addresses = memory2mqtt_services
            .iter()
            .map(|(name, service)| (name.clone(), service.config.listen))
            .collect();
        let config = Self::create_appconfig(http_address, z2m_addresses, &workdir)?;

        let (svc_manager, manager_future) = ServiceManager::spawn();
        let appstate = AppState::from_config(config, svc_manager).await?;
        {
            let mut resources = appstate.res.lock().await;
            seed(&mut resources)?;
        }
        let mut mgr = appstate.manager();

        // memory2mqtt
        for (name, service) in memory2mqtt_services {
            mgr.register_service(format!("m2m-{name}"), service).await?;
        }

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
            hue_events: events_receiver,
            received_hue_events: Vec::new(),
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
            let id = mgr.start(ServiceId::instance("z2m", name)).await?;
            mgr.start(id).await?;
        }
        Ok(())
    }

    fn create_appconfig(
        http_address: SocketAddr,
        z2m_addresses: BTreeMap<String, SocketAddr>,
        workdir: &std::path::Path,
    ) -> TestResult<AppConfig> {
        let z2m = z2m_addresses
            .into_iter()
            .map(|(name, address)| (name, json!({"url": format!("ws://{address}/api")})))
            .collect::<BTreeMap<_, _>>();

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
            "z2m": z2m,
            "bifrost": {
                "state_file": workdir.join("state.yaml"),
                "cert_file": workdir.join("cert.pem")
            }
        }))?)
    }

    async fn wait_for_resource<F, R>(&mut self, name: &str, func: F) -> TestResult<R>
    where
        F: Fn(&[Event]) -> Option<R> + Send + Sync,
    {
        timeout(Duration::from_secs(2), async {
            loop {
                if let Some(resource) = func(&self.received_hue_events) {
                    return Ok(resource);
                }
                self.receive_events().await?;
            }
        })
        .await
        .map_err(|_| TestError::HueEventTimeout(format!("resource: {name}")))?
    }

    pub async fn wait_for_device(
        &mut self,
        fixture_id: FixtureDevice,
    ) -> TestResult<ResourceRecord> {
        let mac_address = fixture_id.mac_address();
        self.wait_for_resource(fixture_id.friendly_name, |events| {
            find_device(events, &mac_address).cloned()
        })
        .await
    }

    pub async fn wait_for_light(&mut self, fixture_id: FixtureDevice) -> TestResult<TestLight> {
        let mac_address = fixture_id.mac_address();
        self.wait_for_resource(fixture_id.friendly_name, |events| {
            find_device(events, &mac_address).and_then(|device| {
                if let Resource::Device(device) = &device.obj {
                    let light_service = device.light_service()?;
                    find_resource_by_link(events, *light_service).map(|record| TestLight {
                        link: ResourceLink::new(record.id, record.obj.rtype()),
                        fixture_id: fixture_id.clone(),
                    })
                } else {
                    None
                }
            })
        })
        .await
    }

    pub async fn wait_for_grouped_light<'a>(
        &mut self,
        fixture_group: &FixtureGroup<'a>,
    ) -> TestResult<TestGroupedLight<'a>> {
        self.wait_for_resource(fixture_group.friendly_name, |events| {
            if let Some((_, room)) = find_room(events, fixture_group.friendly_name) {
                return room.grouped_light_service().map(|&link| TestGroupedLight {
                    link,
                    fixture_group: fixture_group.clone(),
                });
            }
            if let Some((_, zone)) = find_zone(events, fixture_group.friendly_name) {
                return zone.grouped_light_service().map(|&link| TestGroupedLight {
                    link,
                    fixture_group: fixture_group.clone(),
                });
            }
            None
        })
        .await
    }

    pub async fn wait_for_room<'a>(
        &mut self,
        fixture_group: &FixtureGroup<'a>,
    ) -> TestResult<TestRoom<'a>> {
        self.wait_for_resource(fixture_group.friendly_name, |events| {
            if let Some((link, _room)) = find_room(events, fixture_group.friendly_name) {
                return Some(TestRoom {
                    link,
                    fixture_group: fixture_group.clone(),
                });
            }
            None
        })
        .await
    }

    pub async fn wait_for_scene(
        &mut self,
        group: ResourceLink,
        name: &str,
    ) -> TestResult<TestScene> {
        self.wait_for_resource(name, |events| {
            added_resources(events).find_map(|resource| match &resource.obj {
                Resource::Scene(scene) if scene.group == group && scene.metadata.name == name => {
                    Some(TestScene {
                        link: resource.link(),
                    })
                }
                _ => None,
            })
        })
        .await
    }

    pub async fn wait_for_zone<'a>(
        &mut self,
        fixture_group: &FixtureGroup<'a>,
    ) -> TestResult<TestZone<'a>> {
        self.wait_for_resource(fixture_group.friendly_name, |events| {
            if let Some((link, _zone)) = find_zone(events, fixture_group.friendly_name) {
                return Some(TestZone {
                    link,
                    fixture_group: fixture_group.clone(),
                });
            }
            None
        })
        .await
    }

    async fn receive_events(&mut self) -> TestResult<Vec<Event>> {
        let blocks = self.hue_events.recv().await?;
        let mut events = Vec::new();
        for block in blocks {
            self.received_hue_events.push(block.event.clone());
            events.push(block.event);
        }
        Ok(events)
    }
}

fn added_resources(events: &[Event]) -> impl Iterator<Item = &ResourceRecord> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Add(add) => Some(add.data.iter()),
            _ => None,
        })
        .flatten()
}

fn find_resource_by_link(events: &[Event], link: ResourceLink) -> Option<&ResourceRecord> {
    added_resources(events)
        .find(|resource| resource.id == link.rid && resource.obj.rtype() == link.rtype)
}

fn find_device<'a>(events: &'a [Event], mac_address: &str) -> Option<&'a ResourceRecord> {
    match find_zigbee_connectivity(events, mac_address) {
        Some(zc) if zc.owner.rtype == RType::Device => find_resource_by_link(events, zc.owner),
        _ => None,
    }
}

fn find_zigbee_connectivity<'a>(
    events: &'a [Event],
    mac_address: &str,
) -> Option<&'a ZigbeeConnectivity> {
    added_resources(events).find_map(|resource| match &resource.obj {
        Resource::ZigbeeConnectivity(zc) if zc.mac_address == mac_address => Some(zc),
        _ => None,
    })
}

fn find_room<'a>(events: &'a [Event], room_name: &str) -> Option<(ResourceLink, &'a Room)> {
    added_resources(events).find_map(|resource| match &resource.obj {
        Resource::Room(room) if room.metadata.name == room_name => Some((resource.link(), room)),
        _ => None,
    })
}

fn find_zone<'a>(events: &'a [Event], zone_name: &str) -> Option<(ResourceLink, &'a Zone)> {
    added_resources(events).find_map(|resource| match &resource.obj {
        Resource::Zone(zone) if zone.metadata.name == zone_name => Some((resource.link(), zone)),
        _ => None,
    })
}
