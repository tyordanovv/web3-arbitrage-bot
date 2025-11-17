use async_trait::async_trait;
use futures_util::{StreamExt, SinkExt};
use serde_json::Value;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, interval};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::event::context::WsContext;
use crate::types::{BotError, DexId, Network, RawEvent, Result};

#[async_trait]
pub trait WebSocketManager: Send + Sync {
    async fn connect(&mut self, ctx: Arc<WsContext>) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn is_connected(&self) -> bool;
}

pub struct DexWebSocket {
    pub url: String,
    pub package_id: String,
    pub event_type: String,
}

impl DexWebSocket {
    pub fn new(url: &str, package_id: &str, event_type: &str) -> Self {
        Self {
            url: url.to_string(),
            package_id: package_id.to_string(),
            event_type: event_type.to_string(),
        }
    }
}

#[async_trait]
impl WebSocketManager for DexWebSocket {
    async fn connect(&mut self, ctx: Arc<WsContext>) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.url)
            .await
            .map_err(|e| BotError::WebSocket(e.to_string()))?;

        info!("Connected to {} WS for {:?}", ctx.network, ctx.dex_id);

        let (mut write, mut read) = ws_stream.split();

        // Network-specific subscription logic
        let sub_msg = match ctx.network {
            Network::SuiMainnet => json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "suix_subscribeEvent",
                "params": [{
                    "MoveEventType": format!("{}::{}::{}", 
                        self.package_id, 
                        "swap",
                        self.event_type
                    ),
                }]
            }),
            _ => {
                return Err(BotError::WebSocket(format!(
                    "Unsupported network for DefaultDexWs: {:?}",
                    ctx.network
                )));
            }
        };

        write
            .send(Message::Text(sub_msg.to_string().into()))
            .await
            .map_err(|e| BotError::WebSocket(e.to_string()))?;

        info!("Subscribed to {} events for {:?}", ctx.network, ctx.dex_id);

        // Event processing loop (same as before)
        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(json) = serde_json::from_str::<Value>(&text) {
                        if json.get("params").is_some() {
                            let raw = RawEvent::new(
                                json,
                                self.package_id.clone(),
                                self.event_type.clone(),
                            );

                            if ctx.tx.send((ctx.dex_id, raw)).await.is_err() {
                                error!("Failed to send event, channel closed");
                                break;
                            }
                        }
                    }
                }
                Ok(Message::Ping(data)) => {
                    if let Err(e) = write.send(Message::Pong(data)).await {
                        error!("Failed to send pong: {}", e);
                        break;
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("WebSocket connection closed for {:?}", ctx.dex_id);
                    break;
                }
                Err(e) => {
                    error!("WS error for {:?}: {}", ctx.dex_id, e);
                    break;
                }
                _ => {}
            }
        }

        info!("WebSocket disconnected for {:?}", ctx.dex_id);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }

    async fn is_connected(&self) -> bool {
        false
    }
}