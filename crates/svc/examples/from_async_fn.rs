use std::time::Duration;

use thiserror::Error;
use tokio::time::sleep;

use svc::error::SvcResult;
use svc::manager::ServiceManager;

#[derive(Error, Debug)]
pub enum SimpleError {
    #[error("That didn't work")]
    Nope,
}

async fn run() -> Result<(), SimpleError> {
    let dur = Duration::from_millis(800);

    println!("Hello");

    println!("1");
    sleep(dur).await;
    println!("2");
    sleep(dur).await;
    println!("3");

    Ok(())
}

#[tokio::main]
async fn main() -> SvcResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let (mut client, future) = ServiceManager::spawn();

    client.register_function("foo", Box::pin(run())).await?;

    client.start("foo").await?;

    println!("main: service configured");

    client.wait_for_start("foo").await?;

    println!("main: service started");

    let list = client.list().await?;

    println!("{:?}", list);

    client.shutdown().await?;

    future.await??;

    println!("main: service stopped");

    Ok(())
}
