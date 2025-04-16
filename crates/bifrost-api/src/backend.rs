use crate::config::Z2mServer;
use crate::error::BifrostResult;
use crate::Client;

impl Client {
    pub async fn post_backend(&self, name: &str, backend: Z2mServer) -> BifrostResult<()> {
        self.post(&format!("backend/z2m/{name}"), backend).await
    }
}
