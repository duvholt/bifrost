use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio::time::sleep;

use svc::error::SvcResult;
use svc::manager::ServiceManager;
use svc::traits::Service;

#[derive(Clone)]
struct Simple {
    name: String,
    counter: u32,
}

#[derive(Error, Debug)]
pub enum SimpleError {
    #[error("That didn't work..")]
    Nope,
}

#[async_trait]
impl Service for Simple {
    type Error = SimpleError;

    async fn run(&mut self) -> Result<(), SimpleError> {
        let dur = Duration::from_millis(300);

        println!("Hello from {}", self.name);

        println!("1");
        sleep(dur).await;
        println!("2");
        sleep(dur).await;
        println!("3");

        println!("Done running. Now going to stop (this will fail the first time)");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SimpleError> {
        self.counter += 1;

        // pretend this service doesn't succeed at stopping right away
        if self.counter > 1 {
            Ok(())
        } else {
            Err(SimpleError::Nope)
        }
    }
}

#[tokio::main]
async fn main() -> SvcResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let (mut client, future) = ServiceManager::spawn();

    let svc = Simple {
        name: "Simple Service".to_string(),
        counter: 0,
    };

    client.register_service("foo", svc).await?;
    client.start("foo").await?;

    println!("main: service configured");

    client.wait_for_start("foo").await?;

    println!("main: service started");

    client.wait_for_stop("foo").await?;

    println!("main: service stopped");

    client.shutdown().await?;

    future.await??;

    Ok(())
}
