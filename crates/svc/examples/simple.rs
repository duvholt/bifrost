use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio::time::sleep;

use svc::error::{RunSvcError, SvcResult};
use svc::manager::ServiceManager;
use svc::traits::Service;

#[derive(Clone)]
struct Simple {
    name: String,
    counter: u32,
}

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

#[async_trait]
impl Service<SimpleError> for Simple {
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

    let mut svm = ServiceManager::new();

    let svc = Simple {
        name: "Simple Service".to_string(),
        counter: 0,
    };

    svm.register("foo", svc)?;
    svm.start("foo")?;

    svm.wait_for_start("foo").await?;

    svm.wait_for_stop("foo").await?;
    Ok(())
}
