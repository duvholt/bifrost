use std::io::Write;

use svc::manager::ServiceManager;

use bifrost::backend::Backend;
use bifrost::backend::z2m::Z2mBackend;
use bifrost::config;
use bifrost::error::ApiResult;
use bifrost::server::appstate::AppState;
use bifrost::server::http::HttpServer;
use bifrost::server::mdns::MdnsService;
use bifrost::server::{self, Protocol};

/*
 * Formatter function to output in syslog format. This makes sense when running
 * as a service (where output might go to a log file, or the system journal)
 */
#[allow(clippy::match_same_arms)]
fn syslog_format(
    buf: &mut pretty_env_logger::env_logger::fmt::Formatter,
    record: &log::Record,
) -> std::io::Result<()> {
    writeln!(
        buf,
        "<{}>{}: {}",
        match record.level() {
            log::Level::Error => 3,
            log::Level::Warn => 4,
            log::Level::Info => 6,
            log::Level::Debug => 7,
            log::Level::Trace => 7,
        },
        record.target(),
        record.args()
    )
}

fn init_logging() -> ApiResult<()> {
    /* Try to provide reasonable default filters, when RUST_LOG is not specified */
    const DEFAULT_LOG_FILTERS: &[&str] = &[
        "debug",
        "mdns_sd=off",
        "tokio_ssdp=info",
        "tower_http::trace::on_request=info",
        "h2=info",
        "axum::rejection=trace",
    ];

    let log_filters = std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_FILTERS.join(","));

    /* Detect if we need syslog or human-readable formatting */
    if std::env::var("SYSTEMD_EXEC_PID").is_ok_and(|pid| pid == std::process::id().to_string()) {
        Ok(pretty_env_logger::env_logger::builder()
            .format(syslog_format)
            .parse_filters(&log_filters)
            .try_init()?)
    } else {
        Ok(pretty_env_logger::formatted_timed_builder()
            .parse_filters(&log_filters)
            .try_init()?)
    }
}

#[allow(clippy::similar_names)]
async fn build_tasks(appstate: &AppState) -> ApiResult<()> {
    let bconf = &appstate.config().bridge;

    let mut mgr = appstate.manager();

    mgr.register_service("mdns", MdnsService::new(bconf.mac, bconf.ipaddress))
        .await?;

    log::info!("Serving mac [{}]", bconf.mac);

    // register plain http service
    let http_service = HttpServer::http(
        bconf.ipaddress,
        bconf.http_port,
        server::build_service(Protocol::Http, appstate.clone()),
    );
    mgr.register_service("http", http_service).await?;

    let https_service = HttpServer::https_openssl(
        bconf.ipaddress,
        bconf.https_port,
        server::build_service(Protocol::Https, appstate.clone()),
        &appstate.config().bifrost.cert_file,
    )?;

    // .. if either tls backend is enabled, register https service
    mgr.register_service("https", https_service).await?;

    // register config writer
    let svc = server::config_writer(
        appstate.res.clone(),
        appstate.config().bifrost.state_file.clone(),
    );
    mgr.register_function("config-writer", svc).await?;

    // register version updater
    let svc = server::version_updater(appstate.res.clone(), appstate.updater());
    mgr.register_function("version-updater", svc).await?;

    // register ssdp listener
    let svc = server::ssdp::SsdpService::new(bconf.mac, bconf.ipaddress, appstate.updater());
    mgr.register_service("ssdp", svc).await?;

    // register entertainment streaming listener
    let svc = server::entertainment::EntertainmentService::new(
        bconf.ipaddress,
        bconf.entm_port,
        appstate.res.clone(),
    )?;
    mgr.register_service("entertainment", svc).await?;

    // register all z2m backends as services
    for (name, server) in &appstate.config().z2m.servers {
        let client = Z2mBackend::new(
            name.clone(),
            server.clone(),
            appstate.config(),
            appstate.res.clone(),
        )?;
        let stream = appstate.res.lock().await.backend_event_stream();
        let name = format!("z2m-{name}");
        let svc = client.run_forever(stream);

        mgr.register_function(name, svc).await?;
    }

    // finally, iterate over all services and start them
    for (id, _name) in mgr.list().await? {
        mgr.start(id).await?;
    }

    Ok(())
}

async fn run() -> ApiResult<()> {
    init_logging()?;

    #[cfg(feature = "server-banner")]
    server::banner::print()?;

    let config = config::parse("config.yaml".into())?;
    log::debug!("Configuration loaded successfully");

    let (client, future) = ServiceManager::spawn();

    let appstate = AppState::from_config(config, client).await?;

    build_tasks(&appstate).await?;

    tokio::spawn(async move {
        if matches!(tokio::signal::ctrl_c().await, Ok(())) {
            log::warn!("Ctrl-C pressed, exiting..");
            let _ = appstate.manager().shutdown().await;
        }
    });

    future.await??;

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        log::error!("Bifrost error: {err}");
        log::error!("Fatal error encountered, cannot continue.");
    }
}
