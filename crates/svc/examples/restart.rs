use std::time::Duration;

use tokio::time::sleep;

use svc::error::{RunSvcError, SvcResult};
use svc::manager::ServiceManager;

async fn run() -> Result<(), RunSvcError> {
    let dur = Duration::from_millis(200);

    println!("Hello");

    let mut counter = 0;

    loop {
        println!("{counter}");
        sleep(dur).await;
        counter += 1;
    }
}

#[tokio::main]
async fn main() -> SvcResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let (mut client, future) = ServiceManager::spawn();

    client.register_function("foo", run()).await?;

    client.start("foo").await?;
    println!("main: service configured");

    client.wait_for_start("foo").await?;
    println!("main: service started");

    sleep(Duration::from_millis(1000)).await;

    client.stop("foo").await?;
    client.wait_for_stop("foo").await?;

    println!("main: service stopped");

    client.start("foo").await?;
    client.wait_for_start("foo").await?;
    println!("main: service started");

    sleep(Duration::from_millis(1000)).await;
    client.shutdown().await?;

    future.await??;

    Ok(())
}
