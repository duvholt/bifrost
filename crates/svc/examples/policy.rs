use std::time::Duration;

use async_trait::async_trait;
use svc::policy::{Policy, Retry};
use svc::runservice::StandardService;
use thiserror::Error;

use svc::error::{RunSvcError, SvcResult};
use svc::manager::ServiceManager;
use svc::traits::{Service, ServiceRunner, ServiceState};

#[derive(Clone)]
struct PolicyService {
    counter: u32,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Not done yet")]
    MoreToDo,
}

impl From<Error> for RunSvcError<Error> {
    fn from(value: Error) -> Self {
        Self::ServiceError(value)
    }
}

#[async_trait]
impl Service<Error> for PolicyService {
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

    let mut svm = ServiceManager::new();

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

    svm.register(svcr)?;
    svm.start(NAME)?;

    println!("main: service configured");

    svm.wait_for_start(NAME).await?;

    println!("main: service started");

    while let Ok((_id, state)) = svm.next_event().await {
        if state == ServiceState::Failed {
            println!("main: Service has failed");
            break;
        }
    }

    Ok(())
}
