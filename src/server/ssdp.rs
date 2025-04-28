use std::net::Ipv4Addr;
use std::sync::Arc;

use async_trait::async_trait;
use mac_address::MacAddress;
use tokio::sync::watch::{self, Receiver, Sender};
use tokio::sync::Mutex;
use tokio_ssdp::{Device, Server};

use svc::traits::Service;
use uuid::Uuid;

use crate::error::ApiError;
use crate::server::updater::VersionUpdater;

pub struct SsdpService {
    service: Option<Server>,
    updater: Arc<Mutex<VersionUpdater>>,
    usn: Uuid,
    mac: MacAddress,
    ip: Ipv4Addr,
    signal: Option<Sender<bool>>,
    shutdown: Option<Receiver<bool>>,
}

impl SsdpService {
    #[must_use]
    pub fn new(mac: MacAddress, ip: Ipv4Addr, updater: Arc<Mutex<VersionUpdater>>) -> Self {
        Self {
            service: None,
            updater,
            mac,
            ip,
            usn: Uuid::new_v5(&Uuid::NAMESPACE_OID, &mac.bytes()),
            shutdown: None,
            signal: None,
        }
    }
}

#[async_trait]
impl Service for SsdpService {
    type Error = ApiError;
    const SIGNAL_STOP: bool = true;

    async fn configure(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        let location = format!("http://{}:80/description.xml", self.ip);

        let usn = self.usn.to_string();

        let legacy_api_version = self
            .updater
            .lock()
            .await
            .get()
            .await
            .get_legacy_apiversion();

        let server_fut = Server::new([
            Device::new(&usn, "urn:schemas-upnp-org:device:basic:1", &location),
            Device::new(&usn, "upnp:rootdevice", &location),
            Device::new(&usn, "", &location),
        ])
        .extra_header("hue-bridgeid", hue::bridge_id(self.mac).to_uppercase())
        .partial_request_workaround(true)
        .server_name(format!("Hue/1.0 UPnP/1.0 IpBridge/{legacy_api_version}"));

        let (tx, rx) = watch::channel(false);
        self.shutdown = Some(rx);
        self.signal = Some(tx);

        self.service = Some(server_fut);

        Ok(())
    }

    async fn run(&mut self) -> Result<(), Self::Error> {
        if let (Some(svc), Some(shutdown)) = (&self.service, &mut self.shutdown) {
            tokio::select! {
                // wait for shutdown signal
                res = shutdown.changed() => {
                    res.map_err(ApiError::service_error)?;
                },

                // wait for server to run (indefinitely)
                res = svc.clone().serve_addr(self.ip)? => {
                    res?;
                }
            }
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn signal_stop(&mut self) -> Result<(), Self::Error> {
        if let Some(signal) = self.signal.take() {
            log::debug!("Shutting down ssdp..");
            signal
                .send(true)
                .map_err(|_| ApiError::service_error("Failed to send stop signal"))?;
        }
        Ok(())
    }
}
