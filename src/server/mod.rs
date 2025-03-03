pub mod appstate;
pub mod banner;
pub mod certificate;
pub mod http;
pub mod hueevents;
pub mod updater;

use std::fs::File;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use axum::response::Response;
use axum::routing::IntoMakeService;
use axum::{Router, ServiceExt};
use axum_server::service::MakeService;

use camino::Utf8PathBuf;
use hyper::body::Incoming;
use tokio::select;
use tokio::sync::Mutex;
use tokio::time::{sleep_until, MissedTickBehavior};
use tower::Layer;
use tower_http::normalize_path::{NormalizePath, NormalizePathLayer};
use tower_http::trace::TraceLayer;
use tracing::{info_span, Span};

use crate::error::ApiResult;
use crate::resource::Resources;
use crate::routes;
use crate::server::appstate::AppState;
use crate::server::updater::VersionUpdater;

fn trace_layer_on_response(response: &Response<Body>, latency: Duration, span: &Span) {
    span.record(
        "latency",
        tracing::field::display(format!("{}μs", latency.as_micros())),
    );
    span.record("status", tracing::field::display(response.status()));
}

fn router(appstate: AppState) -> Router<()> {
    routes::router(appstate).layer(
        TraceLayer::new_for_http()
            .make_span_with(|request: &Request| {
                info_span!(
                    "http",
                    method = ?request.method(),
                    uri = ?request.uri(),
                    status = tracing::field::Empty,
                    /* latency = tracing::field::Empty, */
                )
            })
            .on_response(trace_layer_on_response),
    )
}

#[must_use]
pub fn build_service(appstate: AppState) -> IntoMakeService<NormalizePath<Router>> {
    let normalized = NormalizePathLayer::trim_trailing_slash().layer(router(appstate));

    ServiceExt::<Request>::into_make_service(normalized)
}

pub async fn http_server<S>(listen_addr: Ipv4Addr, listen_port: u16, svc: S) -> ApiResult<()>
where
    S: Send + MakeService<SocketAddr, Request<Incoming>>,
    S::MakeFuture: Send,
{
    let addr = SocketAddr::from((listen_addr, listen_port));
    log::info!("http listening on {}", addr);

    axum_server::bind(addr).serve(svc).await?;

    Ok(())
}

#[cfg(feature = "tls-openssl")]
pub async fn https_server_openssl<S>(
    listen_addr: Ipv4Addr,
    listen_port: u16,
    svc: S,
    certfile: Utf8PathBuf,
) -> ApiResult<()>
where
    S: Send + MakeService<SocketAddr, Request<Incoming>>,
    S::MakeFuture: Send,
{
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
    tls_builder.set_certificate_file(&certfile, SslFiletype::PEM)?;
    tls_builder.set_private_key_file(&certfile, SslFiletype::PEM)?;
    tls_builder.check_private_key()?;
    tls_builder.set_alpn_select_callback(alpn_select);
    let acceptor = tls_builder.build();

    let config = OpenSSLConfig::from_acceptor(Arc::new(acceptor));

    let addr = SocketAddr::from((listen_addr, listen_port));

    log::info!("https listening on {}", addr);
    axum_server::bind_openssl(addr, config).serve(svc).await?;

    Ok(())
}

#[cfg(feature = "tls-rustls")]
pub async fn https_server_rustls<S>(
    listen_addr: Ipv4Addr,
    listen_port: u16,
    svc: S,
    certfile: Utf8PathBuf,
) -> ApiResult<()>
where
    S: Send + MakeService<SocketAddr, Request<Incoming>>,
    S::MakeFuture: Send,
{
    use crate::error::ApiError;
    use axum_server::tls_rustls::RustlsConfig;

    log::debug!("Loading certificate from [{certfile}]");

    let config = RustlsConfig::from_pem_file(&certfile, &certfile)
        .await
        .map_err(|e| ApiError::Certificate(certfile.clone(), e))?;

    let addr = SocketAddr::from((listen_addr, listen_port));

    log::info!("https listening on {}", addr);
    axum_server::bind_rustls(addr, config).serve(svc).await?;

    Ok(())
}

pub async fn config_writer(res: Arc<Mutex<Resources>>, filename: Utf8PathBuf) -> ApiResult<()> {
    const STABILIZE_TIME: Duration = Duration::from_secs(1);

    let rx = res.lock().await.state_channel();
    let tmp = filename.with_extension("tmp");

    let mut old_state = res.lock().await.serialize()?;

    loop {
        /* Wait for change notification */
        rx.notified().await;

        /* Updates often happen in burst, and we don't want to write the state
         * file over and over, so ignore repeated update notifications within
         * STABILIZE_TIME */
        let deadline = tokio::time::Instant::now() + STABILIZE_TIME;
        loop {
            select! {
                () = rx.notified() => {},
                () = sleep_until(deadline) => break,
            }
        }

        /* Now that the state is likely stabilized, serialize the new state */
        let new_state = res.lock().await.serialize()?;

        /* If state is not actually changed, try again */
        if old_state == new_state {
            continue;
        }

        log::debug!("Config changed, saving..");

        let mut fd = File::create(&tmp)?;
        fd.write_all(new_state.as_bytes())?;
        std::fs::rename(&tmp, &filename)?;

        old_state = new_state;
    }
}

#[allow(clippy::significant_drop_tightening)]
pub async fn version_updater(
    res: Arc<Mutex<Resources>>,
    upd: Arc<Mutex<VersionUpdater>>,
) -> ApiResult<()> {
    const INTERVAL: Duration = Duration::from_secs(60);
    let mut interval = tokio::time::interval(INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut version = upd.lock().await.get().await.clone();

    res.lock().await.update_bridge_version(version.clone());

    loop {
        interval.tick().await;

        let mut lock = upd.lock().await;
        let new_version = lock.get().await;
        if new_version != &version {
            log::info!("New version detected! Patching state database with new version numbers..");
            version.clone_from(new_version);
            res.lock().await.update_bridge_version(version.clone());
        }
    }
}
