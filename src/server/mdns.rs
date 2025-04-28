use std::net::{IpAddr, Ipv4Addr};

use async_trait::async_trait;
use mac_address::MacAddress;
use mdns_sd::{ServiceDaemon, ServiceInfo};

use svc::traits::Service;
use tokio::sync::watch::{self, Receiver, Sender};

use crate::error::ApiError;

pub struct MdnsService {
    mac: MacAddress,
    ip: Ipv4Addr,
    daemon: Option<ServiceDaemon>,
    shutdown: Option<Receiver<bool>>,
    signal: Option<Sender<bool>>,
}

impl MdnsService {
    #[must_use]
    pub const fn new(mac: MacAddress, ip: Ipv4Addr) -> Self {
        Self {
            mac,
            ip,
            daemon: None,
            shutdown: None,
            signal: None,
        }
    }
}

#[async_trait]
impl Service for MdnsService {
    type Error = ApiError;
    const SIGNAL_STOP: bool = true;

    async fn configure(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        let mdns = ServiceDaemon::new()?;
        mdns.enable_interface(IpAddr::from(self.ip))?;
        let service_type = "_hue._tcp.local.";
        let instance_name = format!("bifrost-{}", hex::encode(self.mac.bytes()));
        let service_hostname = format!("{instance_name}.{service_type}");
        let service_addr = self.ip.to_string();
        let service_port = 443;

        let bridge_id = hue::bridge_id(self.mac);

        let properties = [
            ("bridgeid", bridge_id.as_str()),
            ("modelid", hue::HUE_BRIDGE_V2_MODEL_ID),
        ];

        let service_info = ServiceInfo::new(
            service_type,
            &instance_name,
            &service_hostname,
            service_addr,
            service_port,
            &properties[..],
        )?;

        mdns.register(service_info)?;

        let (tx, rx) = watch::channel(false);
        self.shutdown = Some(rx);
        self.signal = Some(tx);
        self.daemon = Some(mdns);

        log::info!("Registered service {}.{}", &instance_name, &service_type);

        Ok(())
    }

    async fn run(&mut self) -> Result<(), Self::Error> {
        if let Some(shutdown) = &mut self.shutdown {
            // wait for shutdown signal
            while !*shutdown.borrow() {
                shutdown.changed().await.map_err(ApiError::service_error)?;
            }

            // remove daemon handle, and request shutdown
            if let Some(daemon) = self.daemon.take() {
                daemon
                    .shutdown()?
                    .recv_async()
                    .await
                    .map_err(ApiError::service_error)?;
            }
        }

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn signal_stop(&mut self) -> Result<(), Self::Error> {
        if let Some(signal) = self.signal.take() {
            log::debug!("Shutting down mdns..");
            signal
                .send(true)
                .map_err(|_| ApiError::service_error("Failed to send stop signal"))?;
        }

        Ok(())
    }
}
