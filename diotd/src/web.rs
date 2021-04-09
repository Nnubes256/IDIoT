use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::{sync::broadcast::Sender as BroadcastSender, task::JoinHandle};
use warp::Filter;

use crate::{hardware::FullSensorData, store::Storage, swarm::PeerData};

static FRONTEND_SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/www-dist/index.html"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebserverConfig {
    port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum WebserverMessage {
    SensorData {
        node: String,
        #[serde(flatten)]
        data: FullSensorData,
    },
    PeerIdentity {
        node: String,
        #[serde(flatten)]
        data: PeerData,
    },
}

mod ws_events {
    use std::sync::Arc;

    use futures::{FutureExt, SinkExt, StreamExt};
    use tokio::sync::broadcast::Receiver as BroadcastReceiver;
    use tokio_stream::wrappers::{errors::BroadcastStreamRecvError, BroadcastStream};
    use warp::ws::{Message, WebSocket};

    use crate::store::Storage;

    use super::WebserverMessage;

    pub async fn user_connected(
        ws: WebSocket,
        channel: BroadcastReceiver<WebserverMessage>,
        storage: Arc<Storage>,
    ) {
        let (mut ws_tx, mut ws_rx) = ws.split();

        let json_state =
            serde_json::to_string(storage.full_system_state()).expect("serializable state");

        if let Err(err) = ws_tx.send(Message::text(json_state)).await {
            error!("Error while sending state to web client: {}", err);
            return;
        };

        let channel_stream = BroadcastStream::new(channel).filter_map(|msg| async move {
            let msg = match msg {
                Ok(msg) => msg,
                Err(BroadcastStreamRecvError::Lagged(num)) => {
                    error!(
                        "Missed {} system messages on a webserver WebSocket task",
                        num
                    );
                    return None;
                }
            };

            match serde_json::to_string(&msg) {
                Ok(json) => Some(Ok(Message::text(json))),
                Err(err) => {
                    error!("Error while serializing message: {}", err);
                    None
                }
            }
        });

        tokio::spawn(channel_stream.forward(ws_tx).map(|r| {
            if let Err(err) = r {
                error!("websocket send error: {}", err)
            }
        }));

        while ws_rx.next().await.is_some() {}
    }
}

pub async fn webserver_spawn(
    storage: Arc<Storage>,
    main_sender: BroadcastSender<WebserverMessage>,
    config: WebserverConfig,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        webserver(config, storage, main_sender).await;
    })
}

#[instrument(skip(storage, main_sender))]
async fn webserver(
    config: WebserverConfig,
    storage: Arc<Storage>,
    main_sender: BroadcastSender<WebserverMessage>,
) {
    let channel = warp::any().map(move || main_sender.subscribe());
    let storage = warp::any().map(move || storage.clone());

    let ws = warp::path("updates")
        .and(warp::ws())
        .and(channel)
        .and(storage)
        .map(|ws: warp::ws::Ws, channel, storage| {
            ws.on_upgrade(move |socket| ws_events::user_connected(socket, channel, storage))
        });

    let frontend = warp::path::end().map(|| warp::reply::html(FRONTEND_SOURCE));
    let paths = frontend.or(ws);

    info!("Webserver listening on wherever");

    warp::serve(paths).run(([0, 0, 0, 0], config.port)).await;
}
