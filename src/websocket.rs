use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;

const MAX_RECONNECT_ATTEMPTS: u32 = 10;
const MAX_RECONNECT_DELAY_MS: u64 = 30_000;

type Callback = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

struct SubscriptionEntry {
    callbacks: Vec<(u64, Callback)>,
}

/// Manages a single WebSocket connection with ref-counted subscriptions.
pub struct WsPool {
    ws_url: String,
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionEntry>>>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    cmd_tx: RwLock<Option<mpsc::UnboundedSender<WsCommand>>>,
    destroyed: Arc<std::sync::atomic::AtomicBool>,
}

enum WsCommand {
    Subscribe { channel: String, coin: Option<String> },
    Shutdown,
}

impl WsPool {
    pub fn new(ws_url: &str) -> Self {
        Self {
            ws_url: ws_url.to_string(),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            cmd_tx: RwLock::new(None),
            destroyed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Subscribe to a channel+coin. Returns a handle - drop it to unsubscribe.
    pub async fn subscribe(
        &self,
        channel: &str,
        coin: Option<&str>,
        callback: impl Fn(serde_json::Value) + Send + Sync + 'static,
    ) -> SubscriptionHandle {
        let sub_key = match coin {
            Some(c) => format!("{channel}:{c}"),
            None => channel.to_string(),
        };

        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        {
            let mut subs = self.subscriptions.write().await;
            let entry = subs.entry(sub_key.clone()).or_insert_with(|| SubscriptionEntry {
                callbacks: Vec::new(),
            });
            entry.callbacks.push((id, Arc::new(callback)));
        }

        // Ensure WS connection is running
        self.ensure_connection().await;

        // Send subscribe command
        if let Some(tx) = self.cmd_tx.read().await.as_ref() {
            let _ = tx.send(WsCommand::Subscribe {
                channel: channel.to_string(),
                coin: coin.map(|s| s.to_string()),
            });
        }

        SubscriptionHandle {
            pool: self.subscriptions.clone(),
            sub_key,
            id,
        }
    }

    async fn ensure_connection(&self) {
        let mut cmd_tx = self.cmd_tx.write().await;
        if cmd_tx.is_some() {
            return;
        }

        let (tx, mut rx) = mpsc::unbounded_channel::<WsCommand>();
        *cmd_tx = Some(tx);

        let ws_url = self.ws_url.clone();
        let subscriptions = self.subscriptions.clone();
        let destroyed = self.destroyed.clone();

        tokio::spawn(async move {
            let mut reconnect_attempts: u32 = 0;

            loop {
                if destroyed.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                let connect_result =
                    tokio_tungstenite::connect_async(&ws_url).await;

                let (mut ws_stream, _) = match connect_result {
                    Ok(conn) => {
                        reconnect_attempts = 0;
                        conn
                    }
                    Err(e) => {
                        tracing::warn!("WS connect failed: {e}");
                        if reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
                            tracing::error!("WS max reconnect attempts reached");
                            break;
                        }
                        let delay = Duration::from_millis(
                            (1000 * 2u64.pow(reconnect_attempts)).min(MAX_RECONNECT_DELAY_MS),
                        );
                        reconnect_attempts += 1;
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                };

                // Re-send all existing subscriptions
                {
                    let subs = subscriptions.read().await;
                    for sub_key in subs.keys() {
                        let (channel, coin) = parse_sub_key(sub_key);
                        let msg = build_subscribe_msg(&channel, coin.as_deref());
                        let _ = ws_stream.send(Message::Text(msg)).await;
                    }
                }

                // Main event loop
                loop {
                    tokio::select! {
                        Some(msg) = ws_stream.next() => {
                            match msg {
                                Ok(Message::Text(text)) => {
                                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                                        dispatch_message(&subscriptions, &parsed).await;
                                    }
                                }
                                Ok(Message::Close(_)) | Err(_) => {
                                    break; // Will reconnect
                                }
                                _ => {}
                            }
                        }
                        Some(cmd) = rx.recv() => {
                            match cmd {
                                WsCommand::Subscribe { channel, coin } => {
                                    let msg = build_subscribe_msg(&channel, coin.as_deref());
                                    let _ = ws_stream.send(Message::Text(msg)).await;
                                }
                                WsCommand::Shutdown => {
                                    let _ = ws_stream.close(None).await;
                                    return;
                                }
                            }
                        }
                    }
                }

                // Connection lost, reconnect
                if destroyed.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                if reconnect_attempts >= MAX_RECONNECT_ATTEMPTS {
                    tracing::error!("WS max reconnect attempts reached");
                    break;
                }

                let delay = Duration::from_millis(
                    (1000 * 2u64.pow(reconnect_attempts)).min(MAX_RECONNECT_DELAY_MS),
                );
                reconnect_attempts += 1;
                tracing::warn!(
                    "WS closed, reconnecting in {}ms (attempt {}/{})",
                    delay.as_millis(),
                    reconnect_attempts,
                    MAX_RECONNECT_ATTEMPTS
                );
                tokio::time::sleep(delay).await;
            }
        });
    }

    pub async fn destroy(&self) {
        self.destroyed
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(tx) = self.cmd_tx.write().await.take() {
            let _ = tx.send(WsCommand::Shutdown);
        }
    }
}

async fn dispatch_message(
    subscriptions: &Arc<RwLock<HashMap<String, SubscriptionEntry>>>,
    msg: &serde_json::Value,
) {
    let channel = match msg.get("channel").and_then(|c| c.as_str()) {
        Some(c) => c,
        None => return,
    };

    let data = match msg.get("data") {
        Some(d) => d.clone(),
        None => return,
    };

    let subs = subscriptions.read().await;

    // Extract coin from data for coin-specific routing
    let data_coin = if data.is_array() {
        data.as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.get("coin"))
            .and_then(|c| c.as_str())
    } else {
        data.get("coin").and_then(|c| c.as_str())
    };

    if let Some(coin) = data_coin {
        let coin_key = format!("{channel}:{coin}");
        if let Some(entry) = subs.get(&coin_key) {
            for (_, cb) in &entry.callbacks {
                cb(data.clone());
            }
        }
    }

    // Channel-only subscriptions
    if let Some(entry) = subs.get(channel) {
        for (_, cb) in &entry.callbacks {
            cb(data.clone());
        }
    }
}

fn build_subscribe_msg(channel: &str, coin: Option<&str>) -> String {
    match coin {
        Some(c) => serde_json::json!({
            "method": "subscribe",
            "subscription": {"type": channel, "coin": c}
        })
        .to_string(),
        None => serde_json::json!({
            "method": "subscribe",
            "subscription": {"type": channel}
        })
        .to_string(),
    }
}

fn parse_sub_key(key: &str) -> (String, Option<String>) {
    if let Some(idx) = key.find(':') {
        (key[..idx].to_string(), Some(key[idx + 1..].to_string()))
    } else {
        (key.to_string(), None)
    }
}

/// Handle to a subscription. Dropping it unsubscribes.
pub struct SubscriptionHandle {
    pool: Arc<RwLock<HashMap<String, SubscriptionEntry>>>,
    sub_key: String,
    id: u64,
}

impl SubscriptionHandle {
    pub async fn unsubscribe(self) {
        self.do_unsub().await;
    }

    async fn do_unsub(&self) {
        let mut subs = self.pool.write().await;
        if let Some(entry) = subs.get_mut(&self.sub_key) {
            entry.callbacks.retain(|(id, _)| *id != self.id);
            if entry.callbacks.is_empty() {
                subs.remove(&self.sub_key);
            }
        }
    }
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        let pool = self.pool.clone();
        let sub_key = self.sub_key.clone();
        let id = self.id;
        tokio::spawn(async move {
            let mut subs = pool.write().await;
            if let Some(entry) = subs.get_mut(&sub_key) {
                entry.callbacks.retain(|(i, _)| *i != id);
                if entry.callbacks.is_empty() {
                    subs.remove(&sub_key);
                }
            }
        });
    }
}
