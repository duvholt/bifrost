use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use axum::Router;
use futures::stream::{self, Stream};
use futures::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::error::ApiResult;
use crate::server::appstate::AppState;

pub async fn get_clip_v2(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = ApiResult<Event>>> {
    let hello = tokio_stream::iter([Ok(Event::default().comment("hi"))]);
    let last_event_id = headers.get("last-event-id");

    let channel = state.res.lock().await.hue_channel();
    let stream = BroadcastStream::new(channel);
    let events = match last_event_id {
        Some(id) => {
            let previous_events = state
                .res
                .lock()
                .await
                .hue_events_since(id.to_str().unwrap());
            stream::iter(previous_events.into_iter().map(Ok))
                .chain(stream)
                .boxed()
        }
        None => stream.boxed(),
    };

    let stream = events.map(move |e| {
        let (evt_id, evt) = e?;
        let json = [evt];
        log::trace!(
            "## EVENT ##: {}",
            serde_json::to_string(&json).unwrap_or_else(|_| "ERROR".to_string())
        );
        Ok(Event::default().id(evt_id).json_data(json)?)
    });

    Sse::new(hello.chain(stream))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/clip/v2", get(get_clip_v2))
}
