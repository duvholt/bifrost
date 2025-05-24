use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr};
use std::os::fd::AsFd;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use nix::sys::socket;
use nix::sys::socket::sockopt::RcvBuf;
use openssl::ssl::{Ssl, SslContext, SslMethod};
use tokio::io::AsyncReadExt;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_openssl::SslStream;
use udp_stream::{UdpListenBuilder, UdpListener, UdpStream};
use uuid::Uuid;

use bifrost_api::backend::BackendRequest;
use hue::api::{Device, EntertainmentConfiguration, Light, RType};
use hue::error::HueError;
use hue::stream::{
    HueStreamLightsV1, HueStreamLightsV2, HueStreamPacket, HueStreamPacketV1, HueStreamPacketV2,
    Rgb16V2, Xy16V2,
};
use svc::traits::Service;

use crate::error::{ApiError, ApiResult};
use crate::resource::Resources;
use crate::routes::auth::STANDARD_CLIENT_KEY;

pub struct EntertainmentService {
    addr: SocketAddr,
    udp: Option<Arc<UdpListener>>,
    ctx: Option<SslContext>,
    res: Arc<Mutex<Resources>>,
}

impl EntertainmentService {
    pub fn new(addr: Ipv4Addr, port: u16, res: Arc<Mutex<Resources>>) -> ApiResult<Self> {
        let res = Self {
            addr: SocketAddr::new(addr.into(), port),
            udp: None,
            ctx: None,
            res,
        };

        Ok(res)
    }

    async fn read_frame(sess: &mut SslStream<UdpStream>, buf: &mut [u8]) -> ApiResult<usize> {
        const TIMEOUT: Duration = Duration::from_secs(10);

        match timeout(TIMEOUT, sess.read(buf)).await {
            Ok(Err(err)) if err.kind() == ErrorKind::UnexpectedEof => {
                log::debug!("Sync stream stopped by sender");
                Ok(0)
            }
            Ok(Err(err)) => {
                log::error!("Error while trying to read sync data: {err:?}");
                Err(ApiError::EntStreamDesync)
            }
            Err(_) => {
                log::warn!("Timeout while waiting for sync data");
                Err(ApiError::EntStreamTimeout)
            }
            Ok(Ok(n)) => {
                log::trace!("Read {n} bytes of sync data");
                Ok(n)
            }
        }
    }

    #[allow(clippy::unused_async, unreachable_code, unused_variables)]
    pub fn translate_v1_frame(
        res: &Resources,
        v1: HueStreamPacketV1,
    ) -> ApiResult<HueStreamPacketV2> {
        let mut lights = BTreeMap::new();
        for id_v1 in v1.light_ids() {
            let uuid = res.from_id_v1(id_v1)?;
            lights.insert(id_v1, uuid);
        }
        let light_ids: BTreeSet<_> = lights.values().copied().collect();

        let mut best_included = 0;
        let mut best_excluded = 0;
        let mut best_area = None;

        for uuid in res.get_resource_ids_by_type(RType::EntertainmentConfiguration) {
            let entconf = res.get_id::<EntertainmentConfiguration>(uuid)?;

            let ent_light_ids: BTreeSet<Uuid> =
                entconf.light_services.iter().map(|ls| ls.rid).collect();

            let included = light_ids.intersection(&ent_light_ids).count();
            let excluded = light_ids.difference(&ent_light_ids).count();

            if (included > best_included)
                || ((included == best_included) && (excluded < best_excluded))
            {
                log::trace!("Found candidate entertainment_configuration: {uuid}");
                log::trace!("  lights included: {included}");
                log::trace!("  lights excluded: {excluded}");
                best_included = included;
                best_excluded = excluded;
                best_area = Some(uuid);
            }
        }

        if let Some(area) = best_area {
            let mut mapping = BTreeMap::new();
            for (id_v1, id) in &lights {
                let light: &Light = res.get_id(*id)?;
                let dev: &Device = res.get(&light.owner)?;
                let ent_svc = dev.entertainment_service().ok_or(HueError::NotFound(*id))?;

                mapping.insert(*id_v1, ent_svc.rid);
            }

            let entconf = res.get_id::<EntertainmentConfiguration>(area)?;

            let lights = match v1.lights {
                HueStreamLightsV1::Rgb(rgb16_v1s) => {
                    let mut v2 = vec![];

                    for rgb in rgb16_v1s {
                        let ent = mapping[&rgb.light_id];
                        for chan in &entconf.channels {
                            for member in &chan.members {
                                if member.service.rid != ent {
                                    continue;
                                }

                                v2.push(Rgb16V2 {
                                    channel: u8::try_from(chan.channel_id)?,
                                    rgb: rgb.rgb,
                                });

                                // there's almost always just a single
                                // member for each channel, but we don't
                                // want to add the light multiple times
                                break;
                            }
                        }
                    }

                    HueStreamLightsV2::Rgb(v2)
                }
                HueStreamLightsV1::Xy(xy16_v1s) => {
                    let mut v2 = vec![];

                    for xy in xy16_v1s {
                        let ent = mapping[&xy.light_id];
                        for chan in &entconf.channels {
                            for member in &chan.members {
                                if member.service.rid != ent {
                                    continue;
                                }

                                v2.push(Xy16V2 {
                                    channel: u8::try_from(chan.channel_id)?,
                                    xy: xy.xy,
                                });

                                // there's almost always just a single
                                // member for each channel, but we don't
                                // want to add the light multiple times
                                break;
                            }
                        }
                    }

                    HueStreamLightsV2::Xy(v2)
                }
            };

            Ok(HueStreamPacketV2 { area, lights })
        } else {
            Err(ApiError::EntStreamInitError)
        }
    }

    pub fn translate_frame(res: &Resources, pkt: HueStreamPacket) -> ApiResult<HueStreamPacketV2> {
        match pkt {
            HueStreamPacket::V1(v1) => Self::translate_v1_frame(res, v1),
            HueStreamPacket::V2(v2) => Ok(v2),
        }
    }

    pub async fn run_loop(&self, mut sess: SslStream<UdpStream>) -> ApiResult<()> {
        let mut buf = [0u8; 1024];

        timeout(Duration::from_secs(5), Pin::new(&mut sess).accept())
            .await
            .map_err(|_| ApiError::EntStreamTimeout)??;

        // read the first frame, and use it to look up area, color mode, etc.
        // this means we discard the first frame, but since we expect at least
        // 10 frames *per second*, this is acceptable.
        let mut sz = Self::read_frame(&mut sess, &mut buf).await?;
        log::trace!("First entertainment frame: {}", hex::encode(&buf[..sz]));
        let raw = HueStreamPacket::parse(&buf[..sz])?;

        let lock = self.res.lock().await;

        let header = Self::translate_frame(&lock, raw)?;

        // look up entertainment area, to make sure it exists
        let _ent: &EntertainmentConfiguration = lock.get_id(header.area)?;

        // request entertainment mode start
        lock.backend_request(BackendRequest::EntertainmentStart(header.area))?;

        drop(lock);

        let mut fps = 0;
        let mut period = Utc::now().timestamp();

        loop {
            let view = &buf[..sz];
            log::trace!("Packet buffer: {}", view.escape_ascii());

            let raw = HueStreamPacket::parse(view)?;
            let pkt = Self::translate_frame(&*self.res.lock().await, raw)?;

            if pkt.color_mode() != header.color_mode() {
                log::error!("Entertainment Mode color_mode changed mid-stream.");
                return Err(ApiError::EntStreamDesync);
            }

            if pkt.area != header.area {
                log::error!("Entertainment Mode area changed mid-stream.");
                return Err(ApiError::EntStreamDesync);
            }

            let ts = Utc::now().timestamp();
            if period != ts {
                log::info!("Incoming entertainment fps: {fps}");
                period = ts;
                fps = 0;
            }

            fps += 1;
            let req = BackendRequest::EntertainmentFrame(pkt.lights);
            self.res.lock().await.backend_request(req)?;

            sz = Self::read_frame(&mut sess, &mut buf).await?;
            if sz == 0 {
                break;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Service for EntertainmentService {
    type Error = ApiError;

    async fn configure(&mut self) -> Result<(), Self::Error> {
        let mut bldr = SslContext::builder(SslMethod::dtls_server())?;

        bldr.set_psk_server_callback(|_sslref, cid, psk| {
            let client_id = String::from_utf8_lossy(cid.unwrap_or_default());
            log::debug!("Setting PSK for {client_id}",);
            STANDARD_CLIENT_KEY.write_to_slice(psk).unwrap();

            log::trace!("psk: {}", hex::encode(&psk[..16]));
            Ok(16)
        });

        self.ctx = Some(bldr.build());
        Ok(())
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        let socket = UdpSocket::bind(self.addr).await?;
        // We need a very small receive buffer, since we deliberately want to
        // drop packets if we can't keep up with the sync mode packets
        socket::setsockopt(&socket.as_fd(), RcvBuf, &512)?;

        let listener = UdpListenBuilder::new(socket)
            .with_buffer_size(512)
            .listen()
            .await?;
        self.udp = Some(Arc::new(listener));
        Ok(())
    }

    async fn run(&mut self) -> Result<(), Self::Error> {
        let Some(udp) = self.udp.clone() else {
            return Err(ApiError::service_error("Udp not initialized"));
        };

        let Some(ctx) = self.ctx.as_ref() else {
            return Err(ApiError::service_error("Ctx not initialized"));
        };

        loop {
            let (socket, _addr) = udp.accept().await?;
            let ssl = Ssl::new(ctx)?;
            let stream = SslStream::new(ssl, socket)?;

            match self.run_loop(stream).await {
                Ok(()) => log::info!("Entertainment stream finished"),
                Err(err) => log::error!("Entertainment stream error: {err}"),
            }

            let req = BackendRequest::EntertainmentStop();
            self.res.lock().await.backend_request(req)?;
        }
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.udp.take();
        Ok(())
    }
}
