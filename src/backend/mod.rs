pub mod z2m;

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast::Receiver;

use bifrost_api::backend::BackendRequest;

use crate::error::ApiResult;

#[async_trait]
pub trait Backend {
    async fn run_forever(self, chan: Receiver<Arc<BackendRequest>>) -> ApiResult<()>;
}
