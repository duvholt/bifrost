use std::time::Duration;

use svc::runservice::StandardService;
use thiserror::Error;
use tokio::time::sleep;

use svc::error::{RunSvcError, SvcResult};
use svc::manager::ServiceManager;
use svc::traits::ServiceRunner;

#[derive(Error, Debug)]
pub enum SimpleError {
    #[error("That didn't work")]
    Nope,
}

impl From<SimpleError> for RunSvcError<SimpleError> {
    fn from(value: SimpleError) -> Self {
        Self::ServiceError(value)
    }
}

async fn run() -> Result<(), SimpleError> {
    let dur = Duration::from_millis(300);

    println!("Hello");

    println!("1");
    sleep(dur).await;
    println!("2");
    sleep(dur).await;
    println!("3");

    println!("Done running. Now going to stop (this will fail the first time)");
    Ok(())
}

#[tokio::main]
async fn main() -> SvcResult<()> {
    pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let mut svm = ServiceManager::new();

    svm.register(StandardService::new("foo", Box::pin(run())))?;

    svm.start("foo")?;

    println!("main: service configured");

    svm.wait_for_start("foo").await?;

    println!("main: service started");

    svm.wait_for_stop("foo").await?;

    println!("main: service stopped");

    Ok(())
}
