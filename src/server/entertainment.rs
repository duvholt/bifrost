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

use bifrost_api::backend::BackendRequest;
use hue::api::EntertainmentConfiguration;
use hue::stream::{HueStreamPacket, HueStreamPacketHeader};
use svc::traits::Service;

use crate::error::{ApiError, ApiResult};
use crate::model::throttle::{Throttle, ThrottleQueue};
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
        const TIMEOUT: Duration = Duration::from_millis(1000);

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

    pub async fn run_loop(&self, mut sess: SslStream<UdpStream>) -> ApiResult<()> {
        let mut buf = [0u8; 1024];

        // read the first frame, and use it to look up area, color mode, etc.
        // this means we discard the first frame, but since we expect at least
        // 10 frames *per second*, this is acceptable.
        let sz = Self::read_frame(&mut sess, &mut buf).await?;
        let header = HueStreamPacketHeader::parse(&buf[..sz])?;

        let lock = self.res.lock().await;

        // look up entertainment area, to make sure it exists
        let _ent: &EntertainmentConfiguration = lock.get_id(header.area)?;

        // request entertainment mode start
        lock.backend_request(BackendRequest::EntertainmentStart(header.area))?;

        drop(lock);

        let mut fps = 0;
        let mut period = Utc::now().timestamp();
        let throttle = Throttle::from_fps(30);
        let mut queue = ThrottleQueue::new(throttle, 2);

        loop {
            let n = Self::read_frame(&mut sess, &mut buf).await?;
            if n == 0 {
                break;
            }

            let view = &buf[..n];
            log::trace!("Packet buffer: {}", view.escape_ascii());

            let pkt = HueStreamPacket::parse(view)?;

            if pkt.color_mode != header.color_mode {
                log::error!("Entertainment Mode color_mode changed mid-stream.");
                return Err(ApiError::EntStreamDesync);
            }

            if pkt.area != header.area {
                log::error!("Entertainment Mode area changed mid-stream.");
                return Err(ApiError::EntStreamDesync);
            }

            queue.push(BackendRequest::EntertainmentFrame(pkt.lights));

            let ts = Utc::now().timestamp();
            if period != ts {
                log::info!("Entertainment fps: {fps}");
                period = ts;
                fps = 0;
            }

            if let Some(req) = queue.pop() {
                fps += 1;
                self.res.lock().await.backend_request(req)?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Service for EntertainmentService {
    type Error = ApiError;
    const SIGNAL_STOP: bool = false;

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
        self.udp = Some(Arc::new(UdpListener::bind(self.addr).await?));
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

            let mut stream = SslStream::new(ssl, socket)?;
            Pin::new(&mut stream).accept().await?;
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

    async fn signal_stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
