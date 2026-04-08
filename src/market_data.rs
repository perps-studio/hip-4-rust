use std::sync::Arc;

use crate::client::{side_coin, HIP4Client};
use crate::error::Result;
use crate::events::SideNameResolver;
use crate::hl_types::*;
use crate::types::market::*;
use crate::websocket::{SubscriptionHandle, WsPool};

use tokio::sync::RwLock;

const MIDS_CACHE_TTL_MS: i64 = 5_000;

fn map_book(raw: &HLL2Book, market_id: &str) -> PredictionOrderBook {
    PredictionOrderBook {
        market_id: market_id.to_string(),
        bids: raw
            .levels
            .0
            .iter()
            .map(|l| PredictionOrderBookLevel {
                price: l.px.clone(),
                size: l.sz.clone(),
            })
            .collect(),
        asks: raw
            .levels
            .1
            .iter()
            .map(|l| PredictionOrderBookLevel {
                price: l.px.clone(),
                size: l.sz.clone(),
            })
            .collect(),
        timestamp: raw.time,
    }
}

fn map_trade(raw: &HLTrade, market_id: &str) -> PredictionTrade {
    PredictionTrade {
        id: raw.tid.to_string(),
        market_id: market_id.to_string(),
        outcome: raw.coin.clone(),
        side: if raw.side == "B" {
            "buy".to_string()
        } else {
            "sell".to_string()
        },
        price: raw.px.clone(),
        size: raw.sz.clone(),
        timestamp: raw.time,
    }
}

pub struct HIP4MarketData {
    client: Arc<HIP4Client>,
    ws_pool: Arc<WsPool>,
    side_names: SideNameResolver,
    mids_cache: RwLock<Option<(HLAllMids, i64)>>,
}

impl HIP4MarketData {
    pub fn new(
        client: Arc<HIP4Client>,
        ws_pool: Arc<WsPool>,
        side_names: SideNameResolver,
    ) -> Self {
        Self {
            client,
            ws_pool,
            side_names,
            mids_cache: RwLock::new(None),
        }
    }

    pub async fn fetch_order_book(
        &self,
        market_id: &str,
        side_index: u64,
    ) -> Result<PredictionOrderBook> {
        let outcome_id: u64 = market_id.parse().unwrap_or(0);
        let coin = side_coin(outcome_id, side_index);
        let raw = self.client.fetch_l2_book(&coin).await?;
        Ok(map_book(&raw, market_id))
    }

    pub async fn fetch_price(&self, market_id: &str) -> Result<PredictionPrice> {
        let outcome_id: u64 = market_id.parse().unwrap_or(0);
        let mids = self.get_mids().await?;
        let (name0, name1) = self.side_names_for(outcome_id).await;

        let side0_coin = side_coin(outcome_id, 0);
        let side1_coin = side_coin(outcome_id, 1);

        let side0_mid = mids.get(&side0_coin).cloned().unwrap_or_else(|| "0".into());
        let side1_mid = mids.get(&side1_coin).cloned().unwrap_or_else(|| "0".into());

        Ok(PredictionPrice {
            market_id: market_id.to_string(),
            outcomes: vec![
                PredictionPriceOutcome {
                    name: name0,
                    price: side0_mid.clone(),
                    midpoint: side0_mid,
                },
                PredictionPriceOutcome {
                    name: name1,
                    price: side1_mid.clone(),
                    midpoint: side1_mid,
                },
            ],
            timestamp: chrono::Utc::now().timestamp_millis(),
        })
    }

    pub async fn fetch_trades(
        &self,
        market_id: &str,
        limit: usize,
        side_index: u64,
    ) -> Result<Vec<PredictionTrade>> {
        let outcome_id: u64 = market_id.parse().unwrap_or(0);
        let coin = side_coin(outcome_id, side_index);
        let raw = self.client.fetch_recent_trades(&coin).await?;
        Ok(raw.iter().take(limit).map(|t| map_trade(t, market_id)).collect())
    }

    pub async fn fetch_candles(
        &self,
        market_id: &str,
        interval: &str,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) -> Result<Vec<CandleData>> {
        let outcome_id: u64 = market_id.parse().unwrap_or(0);
        let coin = side_coin(outcome_id, 0);
        let now = chrono::Utc::now().timestamp_millis();
        let start = start_time.unwrap_or(now - 14 * 24 * 60 * 60 * 1000);
        let end = end_time.unwrap_or(now);
        let raw = self
            .client
            .fetch_candle_snapshot(&coin, interval, start, end)
            .await?;

        Ok(raw
            .iter()
            .map(|c| CandleData {
                time: c.t / 1000,
                open: c.o.parse().unwrap_or(0.0),
                high: c.h.parse().unwrap_or(0.0),
                low: c.l.parse().unwrap_or(0.0),
                close: c.c.parse().unwrap_or(0.0),
                volume: c.v.parse().unwrap_or(0.0),
            })
            .collect())
    }

    // -- WebSocket subscriptions --

    pub async fn subscribe_order_book(
        &self,
        market_id: &str,
        on_data: impl Fn(PredictionOrderBook) + Send + Sync + 'static,
    ) -> SubscriptionHandle {
        let outcome_id: u64 = market_id.parse().unwrap_or(0);
        let coin = side_coin(outcome_id, 0);
        let mid = market_id.to_string();

        self.ws_pool
            .subscribe("l2Book", Some(&coin), move |data| {
                if let Ok(book) = serde_json::from_value::<HLWsL2BookData>(data) {
                    let mapped = PredictionOrderBook {
                        market_id: mid.clone(),
                        bids: book
                            .levels
                            .0
                            .iter()
                            .map(|l| PredictionOrderBookLevel {
                                price: l.px.clone(),
                                size: l.sz.clone(),
                            })
                            .collect(),
                        asks: book
                            .levels
                            .1
                            .iter()
                            .map(|l| PredictionOrderBookLevel {
                                price: l.px.clone(),
                                size: l.sz.clone(),
                            })
                            .collect(),
                        timestamp: book.time,
                    };
                    on_data(mapped);
                }
            })
            .await
    }

    pub async fn subscribe_price(
        &self,
        market_id: &str,
        on_data: impl Fn(PredictionPrice) + Send + Sync + 'static,
    ) -> SubscriptionHandle {
        let outcome_id: u64 = market_id.parse().unwrap_or(0);
        let coin0 = side_coin(outcome_id, 0);
        let coin1 = side_coin(outcome_id, 1);
        let mid = market_id.to_string();

        self.ws_pool
            .subscribe("allMids", None, move |data| {
                let mids = match data.get("mids") {
                    Some(m) => m,
                    None => return,
                };

                let side0 = mids
                    .get(&coin0)
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");
                let side1 = mids
                    .get(&coin1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("0");

                on_data(PredictionPrice {
                    market_id: mid.clone(),
                    outcomes: vec![
                        PredictionPriceOutcome {
                            name: "Side 0".to_string(),
                            price: side0.to_string(),
                            midpoint: side0.to_string(),
                        },
                        PredictionPriceOutcome {
                            name: "Side 1".to_string(),
                            price: side1.to_string(),
                            midpoint: side1.to_string(),
                        },
                    ],
                    timestamp: chrono::Utc::now().timestamp_millis(),
                });
            })
            .await
    }

    pub async fn subscribe_trades(
        &self,
        market_id: &str,
        on_data: impl Fn(PredictionTrade) + Send + Sync + 'static,
    ) -> SubscriptionHandle {
        let outcome_id: u64 = market_id.parse().unwrap_or(0);
        let coin = side_coin(outcome_id, 0);
        let mid = market_id.to_string();

        self.ws_pool
            .subscribe("trades", Some(&coin), move |data| {
                if let Some(arr) = data.as_array() {
                    for item in arr {
                        if let Ok(trade) = serde_json::from_value::<HLTrade>(item.clone()) {
                            on_data(map_trade(&trade, &mid));
                        }
                    }
                }
            })
            .await
    }

    async fn get_mids(&self) -> Result<HLAllMids> {
        let now = chrono::Utc::now().timestamp_millis();
        {
            let cache = self.mids_cache.read().await;
            if let Some((ref data, ts)) = *cache {
                if now - ts < MIDS_CACHE_TTL_MS {
                    return Ok(data.clone());
                }
            }
        }
        let data = self.client.fetch_all_mids().await?;
        {
            let mut cache = self.mids_cache.write().await;
            *cache = Some((data.clone(), now));
        }
        Ok(data)
    }

    async fn side_names_for(&self, outcome_id: u64) -> (String, String) {
        let names = self.side_names.read().await;
        names
            .get(&outcome_id)
            .cloned()
            .unwrap_or_else(|| ("Side 0".to_string(), "Side 1".to_string()))
    }
}

#[derive(Debug, Clone)]
pub struct CandleData {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
