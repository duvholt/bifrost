use std::time::Duration;

use async_trait::async_trait;
use svc::policy::{Policy, Retry};
use svc::runservice::StandardService;
use thiserror::Error;

use svc::error::SvcResult;
use svc::manager::ServiceManager;
use svc::traits::{Service, ServiceState};

#[derive(Clone)]
struct PolicyService {
    counter: u32,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Not done yet")]
    MoreToDo,
}

#[async_trait]
impl Service for PolicyService {
    type Error = Error;

    async fn run(&mut self) -> Result<(), Error> {
        println!("Hello {}", self.counter);
        self.counter += 1;

        // returning an Err will invoke the policy for the Running state
        Err(Error::MoreToDo)
    }
}

#[tokio::main]
async fn main() -> SvcResult<()> {
    const NAME: &str = "policy-service";

    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let (mut client, future) = ServiceManager::spawn();

    let svc = PolicyService { counter: 0 };

    // Manually construct a ServiceRunner, and set a specific policy for
    // handling errors during .run()
    let svcr = StandardService::new(NAME, svc).with_run_policy(
        // Try up to 5 times, waiting 300ms between each attempt
        Policy::new()
            .with_retry(Retry::Limit(5))
            .with_delay(Duration::from_millis(300)),
    );

    let uuid = client.register(NAME, svcr).await?;
    client.start(uuid).await?;
    println!("main: service will attempt to run 5 times");

    client.wait_for_start(uuid).await?;
    println!("main: service started");

    client.wait_for_state(uuid, ServiceState::Failed).await?;
    client.shutdown().await?;
    future.await??;

    Ok(())
}
