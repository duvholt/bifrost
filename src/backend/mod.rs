pub mod z2m;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;

use crate::error::ApiResult;
use crate::z2m::request::ClientRequest;

pub type BackendRequest = ClientRequest;

#[async_trait]
pub trait Backend {
    async fn run_forever(self, chan: Receiver<Arc<BackendRequest>>) -> ApiResult<()>;
}
