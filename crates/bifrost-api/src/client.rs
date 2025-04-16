use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::BifrostResult;

#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
    url: Url,
}

impl Client {
    #[must_use]
    pub const fn new(client: reqwest::Client, url: Url) -> Self {
        Self { client, url }
    }

    #[must_use]
    pub fn from_url(url: Url) -> Self {
        Self::new(reqwest::Client::new(), url)
    }

    pub async fn request<I: Serialize, O: DeserializeOwned>(
        &self,
        scope: &str,
        method: Method,
        data: Option<I>,
    ) -> BifrostResult<O> {
        let url = self.url.join(scope)?;

        let mut req = self.client.request(method, url);

        if let Some(data) = data {
            req = req.json(&data);
        }

        let response = req.send().await?.error_for_status()?.json().await?;

        Ok(response)
    }

    pub async fn get<T: DeserializeOwned>(&self, scope: &str) -> BifrostResult<T> {
        self.request(scope, Method::GET, None::<()>).await
    }

    pub async fn post<I: Serialize, O: DeserializeOwned>(
        &self,
        scope: &str,
        data: I,
    ) -> BifrostResult<O> {
        self.request(scope, Method::POST, Some(data)).await
    }

    pub async fn put<I: Serialize, O: DeserializeOwned>(
        &self,
        scope: &str,
        data: I,
    ) -> BifrostResult<O> {
        self.request(scope, Method::PUT, Some(data)).await
    }
}
