use std::time::Duration;

use async_trait::async_trait;
use svc::policy::{Policy, Retry};
use svc::runservice::StandardService;
use thiserror::Error;

use svc::error::SvcResult;
use svc::manager::ServiceManager;
use svc::traits::Service;

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
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let (mut client, future) = ServiceManager::new().daemonize();

    let svc = PolicyService { counter: 0 };

    const NAME: &str = "policy-service";

    // Manually construct a ServiceRunner, and set a specific policy for
    // handling errors during .run()
    let svcr = StandardService::new(NAME, svc).with_run_policy(
        // Try up to 5 times, waiting 300ms between each attempt
        Policy::new()
            .with_retry(Retry::Limit(5))
            .with_delay(Duration::from_millis(300)),
    );

    client.register(NAME, svcr).await?;
    client.start(NAME).await?;

    println!("main: service configured to try starting 5 times");

    client.wait_for_start(NAME).await?;

    println!("main: service started");

    future.await??;

    Ok(())
}
