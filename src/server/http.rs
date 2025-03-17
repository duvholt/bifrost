use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use async_trait::async_trait;
use axum::extract::Request;
use axum_server::accept::{Accept, DefaultAcceptor};
use axum_server::service::{MakeService, SendService};
use axum_server::{Handle, Server};
use camino::Utf8Path;
use futures::future::BoxFuture;
use futures::FutureExt;
use hyper::body::Incoming;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use svc::traits::Service;

use crate::error::{ApiError, ApiResult};

pub struct HttpServer<S, A, F, E = ()> {
    addr: SocketAddr,
    bind: fn(&Self) -> ApiResult<Server<A>>,
    server: Option<F>,
    svc: S,
    extra: E,
    handle: Handle,
}

#[async_trait]
impl<S, A, E> Service for HttpServer<S, A, BoxFuture<'static, Result<(), std::io::Error>>, E>
where
    E: Send + Unpin,
    S: Send + Clone + MakeService<SocketAddr, Request<Incoming>> + 'static,
    S::MakeFuture: Send,
    A: Accept<TcpStream, S::Service> + Clone + Send + Sync + 'static,
    A::Stream: AsyncRead + AsyncWrite + Unpin + Send,
    A::Service: SendService<Request<Incoming>> + Send,
    A::Future: Send,
{
    type Error = ApiError;
    const SIGNAL_STOP: bool = true;

    async fn start(&mut self) -> Result<(), ApiError> {
        log::info!("Opening listen port on {}", self.addr);
        self.server = Some(
            (self.bind)(self)?
                .handle(self.handle.clone())
                .serve(self.svc.clone())
                .boxed(),
        );
        Ok(())
    }

    async fn run(&mut self) -> Result<(), ApiError> {
        if let Some(server) = self.server.take() {
            server.await?;
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ApiError> {
        log::info!("Stopping server {}", self.addr);
        self.server.take();
        Ok(())
    }

    async fn signal_stop(&mut self) -> Result<(), ApiError> {
        self.handle.graceful_shutdown(Some(Duration::from_secs(1)));
        Ok(())
    }
}

impl<S, F> HttpServer<S, DefaultAcceptor, F>
where
    Self: Service,
{
    pub fn http(listen_addr: Ipv4Addr, listen_port: u16, svc: S) -> Self
    where
        S: Send + Clone + MakeService<SocketAddr, Request<Incoming>>,
        S::MakeFuture: Send,
    {
        let addr = SocketAddr::from((listen_addr, listen_port));

        Self {
            addr,
            bind: |slf| Ok(axum_server::bind(slf.addr)),
            server: None,
            svc,
            extra: (),
            handle: Handle::new(),
        }
    }
}

#[cfg(feature = "tls-openssl")]
use axum_server::tls_openssl::{OpenSSLAcceptor, OpenSSLConfig};

#[cfg(feature = "tls-openssl")]
impl<S, F> HttpServer<S, OpenSSLAcceptor, F, OpenSSLConfig>
where
    Server<DefaultAcceptor>: Send,
    Self: Service,
    S: Send + Unpin,
{
    pub fn https_openssl(
        listen_addr: Ipv4Addr,
        listen_port: u16,
        svc: S,
        certfile: &Utf8Path,
    ) -> ApiResult<Self> {
        use std::sync::Arc;

        use axum_server::tls_openssl::OpenSSLConfig;
        use openssl::ssl::{AlpnError, SslAcceptor, SslFiletype, SslMethod, SslRef};

        fn alpn_select<'a>(_tls: &mut SslRef, client: &'a [u8]) -> Result<&'a [u8], AlpnError> {
            openssl::ssl::select_next_proto(b"\x02h2\x08http/1.1", client).ok_or(AlpnError::NOACK)
        }

        // the default axum-server function for configuring openssl uses
        // [`SslAcceptor::mozilla_modern_v5`], which requires TLSv1.3.
        //
        // That protocol version is too new for some important clients, like
        // Hue Sync for PC, so manually construct an OpenSSLConfig here, with
        // slightly more relaxed settings.

        log::debug!("Loading certificate from [{certfile}]");

        let mut tls_builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls())?;
        tls_builder.set_certificate_file(certfile, SslFiletype::PEM)?;
        tls_builder.set_private_key_file(certfile, SslFiletype::PEM)?;
        tls_builder.check_private_key()?;
        tls_builder.set_alpn_select_callback(alpn_select);
        let acceptor = tls_builder.build();

        let config = OpenSSLConfig::from_acceptor(Arc::new(acceptor));

        let addr = SocketAddr::from((listen_addr, listen_port));

        let srv = Self {
            addr,
            bind: |slf: &Self| Ok(axum_server::bind_openssl(slf.addr, slf.extra.clone())),
            server: None,
            svc,
            extra: config,
            handle: Handle::new(),
        };

        Ok(srv)
    }
}

#[cfg(feature = "tls-rustls")]
use axum_server::tls_rustls::{RustlsAcceptor, RustlsConfig};

#[cfg(feature = "tls-rustls")]
impl<S, F> HttpServer<S, RustlsAcceptor, F, RustlsConfig>
where
    Server<DefaultAcceptor>: Send,
    Self: Service,
    S: Send + Unpin,
{
    pub async fn https_rustls(
        listen_addr: Ipv4Addr,
        listen_port: u16,
        svc: S,
        certfile: &Utf8Path,
    ) -> ApiResult<Self> {
        use crate::error::ApiError;
        use axum_server::tls_rustls::RustlsConfig;

        log::debug!("Loading certificate from [{certfile}]");

        let config = RustlsConfig::from_pem_file(certfile, certfile)
            .await
            .map_err(|e| ApiError::Certificate(certfile.to_owned(), e))?;

        let addr = SocketAddr::from((listen_addr, listen_port));

        log::info!("https listening on {}", addr);

        let addr = SocketAddr::from((listen_addr, listen_port));

        let srv = Self {
            addr,
            bind: |slf: &Self| Ok(axum_server::bind_rustls(slf.addr, slf.extra.clone())),
            server: None,
            svc,
            extra: config,
            handle: Handle::new(),
        };

        Ok(srv)
    }
}
