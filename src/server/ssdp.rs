use std::net::Ipv4Addr;
use std::sync::Arc;

use async_trait::async_trait;
use mac_address::MacAddress;
use tokio::sync::Mutex;
use tokio::sync::watch::{self, Receiver, Sender};
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

/// Prefix used in all USN for hue bridges
///
/// The USN is constructed like so:
///
///    {PREFIX}-{MAC}
///
/// Example: 2f402f80-da50-11e1-9b23-112233445566
const HUE_BRIDGE_USN_PREFIX: &str = "2f402f80-da50-11e1-9b23";

#[must_use]
pub fn hue_bridge_usn(mac: MacAddress) -> Uuid {
    let hexmac = hex::encode(mac.bytes());
    let uuid_str = format!("{HUE_BRIDGE_USN_PREFIX}-{hexmac}");
    Uuid::try_parse(&uuid_str).unwrap()
}

impl SsdpService {
    #[must_use]
    pub fn new(mac: MacAddress, ip: Ipv4Addr, updater: Arc<Mutex<VersionUpdater>>) -> Self {
        Self {
            service: None,
            updater,
            mac,
            ip,
            usn: hue_bridge_usn(mac),
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

        let legacy_api_version = self
            .updater
            .lock()
            .await
            .get()
            .await
            .get_legacy_apiversion();

        let usn = format!("uuid:{}", self.usn);
        let usn_rootdev = format!("{usn}::upnp:rootdevice");

        // It's uncertain if these Device settings are valid according to the UPnP
        // spec, but they exactly match the format sent out by real hue bridges
        let server_fut = Server::new([
            Device::raw(&usn_rootdev, "upnp:rootdevice", &location),
            Device::raw(&usn, &usn, &location),
            Device::raw(&usn, "urn:schemas-upnp-org:device:basic:1", &location),
        ])
        .extra_header("hue-bridgeid", hue::bridge_id(self.mac).to_uppercase())
        // enable workarounds to make Hue Essentials work
        .partial_request_workaround(true)
        // Hue Essentials strikes again: server name must look like this
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

#[cfg(test)]
mod tests {
    use mac_address::MacAddress;
    use uuid::uuid;

    use crate::server::ssdp::hue_bridge_usn;

    #[test]
    fn usn_generation() {
        let expected = uuid!("2f402f80-da50-11e1-9b23-112233445566");
        let generated = hue_bridge_usn(MacAddress::new([0x11, 0x22, 0x33, 0x44, 0x55, 0x66]));

        assert_eq!(generated, expected);
    }
}
