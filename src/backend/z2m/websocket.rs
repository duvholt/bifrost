use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{SinkExt, Stream};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use z2m::{api::RawMessage, request::Z2mRequest};

use crate::error::ApiResult;

pub struct Z2mWebSocket {
    pub name: String,
    pub socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl Z2mWebSocket {
    pub const fn new(name: String, socket: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Self {
        Self { name, socket }
    }

    pub async fn send(&mut self, topic: &str, payload: &Z2mRequest<'_>) -> ApiResult<()> {
        /* let Some(link) = self.map.get(topic) else { */
        /*     log::trace!( */
        /*         "[{}] Topic [{topic}] unknown on this z2m connection", */
        /*         self.name */
        /*     ); */
        /*     return Ok(()); */
        /* }; */

        /* log::trace!( */
        /*     "[{}] Topic [{topic}] known as {link:?} on this z2m connection, sending event..", */
        /*     self.name */
        /* ); */

        let api_req = match &payload {
            Z2mRequest::Untyped { endpoint, value } => RawMessage {
                topic: format!("{topic}/{endpoint}/set"),
                payload: serde_json::to_value(value)?,
            },
            Z2mRequest::RawWrite(value) => RawMessage {
                topic: format!("{topic}/set/write"),
                payload: serde_json::to_value(value)?,
            },
            Z2mRequest::GroupMemberAdd(value) => RawMessage {
                topic: "bridge/request/group/members/add".into(),
                payload: serde_json::to_value(value)?,
            },
            Z2mRequest::GroupMemberRemove(value) => RawMessage {
                topic: "bridge/request/group/members/remove".into(),
                payload: serde_json::to_value(value)?,
            },
            _ => RawMessage {
                topic: format!("{topic}/set"),
                payload: serde_json::to_value(payload)?,
            },
        };

        let json = serde_json::to_string(&api_req)?;
        log::debug!("[{}] Sending {json}", self.name);
        let msg = Message::text(json);
        Ok(self.socket.send(msg).await?)
    }
}

impl Stream for Z2mWebSocket
where
    Self: Unpin,
{
    type Item = Result<Message, tungstenite::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        WebSocketStream::poll_next(Pin::new(&mut self.socket), cx)
    }
}
