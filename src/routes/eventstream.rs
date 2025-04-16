use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use futures::StreamExt;
use futures::stream::{self, Stream};
use tokio_stream::wrappers::BroadcastStream;

use crate::error::ApiResult;
use crate::server::appstate::AppState;

pub async fn get_clip_v2(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = ApiResult<Event>>> {
    let hello = tokio_stream::iter([Ok(Event::default().comment("hi"))]);
    let last_event_id = headers.get("last-event-id").map(HeaderValue::to_str);

    let channel = state.res.lock().await.hue_event_stream().subscribe();
    let stream = BroadcastStream::new(channel);
    let events = match last_event_id {
        Some(Ok(id)) => {
            let previous_events = state
                .res
                .lock()
                .await
                .hue_event_stream()
                .events_sent_after_id(id);
            stream::iter(previous_events.into_iter().map(Ok))
                .chain(stream)
                .boxed()
        }
        _ => stream.boxed(),
    };

    let stream = events.map(move |e| {
        let evt = e?;
        let evt_id = evt.id();
        let json = [evt.block];
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
