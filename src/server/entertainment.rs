use std::net::{Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use openssl::ssl::{Ssl, SslContext, SslMethod};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
use tokio::sync::Mutex;
use tokio::time::timeout;
use tokio_openssl::SslStream;
use udp_stream::{UdpListener, UdpStream};

use hue::api::EntertainmentConfiguration;
use hue::stream::{HueStreamPacket, HueStreamPacketHeader};
use svc::traits::Service;

use crate::backend::BackendRequest;
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

async fn fill_buffer_to<T>(rdr: &mut BufReader<T>, size: usize) -> ApiResult<()>
where
    T: AsyncRead + Unpin,
{
    while rdr.buffer().len() < size {
        rdr.fill_buf().await?;
    }

    Ok(())
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

    pub async fn run_loop(&self, sess: SslStream<UdpStream>) -> ApiResult<()> {
        const TIMEOUT: Duration = Duration::from_millis(1000);

        let mut rdr = BufReader::new(sess);

        let len = HueStreamPacket::HEADER_SIZE;

        match timeout(TIMEOUT, fill_buffer_to(&mut rdr, len)).await {
            Ok(Err(_)) | Err(_) => return Err(ApiError::EntStreamInitError),
            Ok(Ok(())) => {}
        };

        let header = HueStreamPacketHeader::parse(rdr.buffer())?;

        // look up entertainment area
        let lock = self.res.lock().await;
        let ent: &EntertainmentConfiguration = lock.get_id(header.area)?;
        let nlights = ent.channels.len();
        lock.backend_request(BackendRequest::EntertainmentStart(header.area))?;
        drop(lock);

        let mut buf = vec![0u8; HueStreamPacket::size_with_lights(nlights)];

        let mut fps = 0;
        let mut period = Utc::now().timestamp();
        let throttle = Throttle::from_fps(30);
        let mut queue = ThrottleQueue::new(throttle, 2);

        loop {
            match timeout(Duration::from_millis(1000), rdr.read_exact(&mut buf)).await {
                Ok(Err(_)) | Err(_) => break,
                Ok(Ok(_)) => {}
            };

            let pkt = HueStreamPacket::parse(&buf)?;

            if pkt.color_mode != header.color_mode {
                log::error!("Entertainment Mode color_mode changes mid-stream.");
                return Err(ApiError::EntStreamDesync);
            }

            if pkt.area != header.area {
                log::error!("Entertainment Mode area changed mid-stream.");
                return Err(ApiError::EntStreamDesync);
            }

            if queue.push(BackendRequest::EntertainmentFrame(pkt.lights)) {
                let ts = Utc::now().timestamp();
                if period != ts {
                    log::info!("Entertainment fps: {fps}");
                    period = ts;
                    fps = 0;
                }
            }

            if let Some(req) = queue.pop() {
                fps += 1;
                self.res.lock().await.backend_request(req)?;
            }
        }

        let req = BackendRequest::EntertainmentStop();
        self.res.lock().await.backend_request(req)?;

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
            return Err(ApiError::SvcError("Udp not initialized".to_string()));
        };

        let Some(ctx) = self.ctx.as_ref() else {
            return Err(ApiError::SvcError("Ctx not initialized".to_string()));
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
        }
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn signal_stop(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
