use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::response::IntoResponse;
use bytes::Bytes;
use serde::de::DeserializeOwned;
use serde::Serialize;

// Simple wrapper around axum::Json, which skips the header requirements.
//
// The axum version requires "Content-Type: application/json", which many
// (buggy) apps don't actually send. So we are forced to skip this check.
pub struct Json<T>(pub T);

impl<S, T> FromRequest<S> for Json<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = JsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let bytes = Bytes::from_request(req, state).await?;
        Ok(Self(axum::Json::from_bytes(&bytes)?.0))
    }
}

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}
