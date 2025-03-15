use std::io::Write;

use clap::Parser;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use tokio::io::{stdin, AsyncBufReadExt, BufReader};
use tokio::select;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use bifrost::error::ApiResult;
use bifrost::z2m::api::RawMessage;

#[macro_use]
extern crate log;

#[derive(Parser, Debug)]
struct Args {
    /// Url to websocket (<ws://example.org:1234/>)
    url: String,
}

#[derive(Deserialize)]
pub struct Log {
    pub level: String,
    message: String,
}

#[allow(clippy::redundant_pub_crate)]
#[tokio::main]
async fn main() -> ApiResult<()> {
    pretty_env_logger::formatted_builder()
        .write_style(pretty_env_logger::env_logger::fmt::WriteStyle::Always)
        .filter_level(log::LevelFilter::Debug)
        .parse_default_env()
        .init();

    let args = Args::parse();

    let (mut socket, _) = connect_async(args.url).await?;

    let mut lines = BufReader::new(stdin()).lines();

    loop {
        print!("> ");
        std::io::stdout().flush()?;

        select! {
            Ok(Some(line)) = lines.next_line() => {
                if line.is_empty() {
                    continue;
                }

                /* println!("{line}"); */

                let req: RawMessage = match serde_json::from_str(&line) {
                    Ok(res) => res,
                    Err(err) => {
                        error!("Failed to parse: {err}");
                        continue
                    },
                };

                /* info!("req: {req:?}"); */
                match serde_json::to_string(&req) {
                    Ok(pkt) => {
                        /* println!("Parsed: {}", &pkt); */
                        socket.send(Message::text(pkt)).await?;
                    }
                    Err(err) => {
                        error!("Nope: {err}");
                    }
                }
            }
            Some(Ok(pkt)) = socket.next() => {
                if let Message::Text(txt) = pkt {
                    let msg: RawMessage = serde_json::from_str(&txt)?;
                    if msg.topic != "bridge/info" && msg.topic != "bridge/definitions" && msg.topic != "bridge/devices" {
                        if msg.topic == "bridge/logging" {
                            let log: Log = serde_json::from_value(msg.payload)?;
                            if log.message.starts_with("zhc:tz: Read result") {
                                let body = &log.message.split('\'').nth(2).unwrap()[2..];
                                info!("{body}");
                                let rsp: Value = serde_json::from_str(body)?;
                                info!("{rsp:?}");
                            } else if log.message.contains("UNSUPPORTED_ATTRIBUTE") {
                                error!("Unsupported attribute");
                            } else {
                                info!("{:?}", log.message);
                            }
                        } else {
                            println!("{msg:?}");
                        }
                    }
                } else {
                    println!("{pkt:?}");
                }
            }
        }
    }
}
