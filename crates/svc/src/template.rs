use async_trait::async_trait;

#[cfg(feature = "manager")]
use crate::error::RunSvcError;
use crate::error::SvcError;
use crate::traits::{BoxDynService, Service, StopResult};

#[cfg(feature = "manager")]
pub trait ServiceTemplate: Send {
    fn generate(&self, instance: String) -> Result<BoxDynService, SvcError>;
}

pub struct ErrorAdapter<S: Service> {
    svc: S,
}

impl<S: Service> ErrorAdapter<S> {
    pub const fn new(svc: S) -> Self {
        Self { svc }
    }
}

#[async_trait]
impl<S: Service> Service for ErrorAdapter<S> {
    type Error = RunSvcError;

    async fn configure(&mut self) -> Result<(), Self::Error> {
        self.svc
            .configure()
            .await
            .map_err(|err| RunSvcError::ServiceError(Box::new(err)))
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        self.svc
            .start()
            .await
            .map_err(|err| RunSvcError::ServiceError(Box::new(err)))
    }

    async fn run(&mut self) -> Result<(), Self::Error> {
        self.svc
            .run()
            .await
            .map_err(|err| RunSvcError::ServiceError(Box::new(err)))
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.svc
            .stop()
            .await
            .map_err(|err| RunSvcError::ServiceError(Box::new(err)))
    }

    async fn signal_stop(&mut self) -> Result<StopResult, Self::Error> {
        self.svc
            .signal_stop()
            .await
            .map_err(|err| RunSvcError::ServiceError(Box::new(err)))
    }
}

impl<F> ServiceTemplate for F
where
    F: Fn(String) -> Result<BoxDynService, SvcError> + Send,
{
    fn generate(&self, instance: String) -> Result<BoxDynService, SvcError> {
        self(instance)
    }
}
