pub mod z2m;

use async_trait::async_trait;

use crate::error::ApiResult;

#[async_trait]
pub trait Backend {
    async fn run_forever(self) -> ApiResult<()>;
}
