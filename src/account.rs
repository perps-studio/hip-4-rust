use std::collections::HashMap;
use std::sync::Arc;

use crate::client::{coin_outcome_id, is_outcome_coin, parse_side_coin, HIP4Client};
use crate::error::Result;
use crate::events::SideNameResolver;
use crate::hl_types::*;
use crate::types::account::*;
use crate::types::event::PredictionEvent;

fn map_spot_balance(
    bal: &HLSpotBalance,
    all_mids: &HLAllMids,
    name_map: &HashMap<String, (String, String)>,
    side_names: &HashMap<u64, (String, String)>,
) -> Option<PredictionPosition> {
    if !is_outcome_coin(&bal.coin) {
        return None;
    }

    let total: f64 = bal.total.parse().ok()?;
    if total == 0.0 {
        return None;
    }

    let outcome_id = coin_outcome_id(&bal.coin);
    let market_id = outcome_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| bal.coin.clone());

    let outcome = bal.coin.clone();

    // Resolve human-readable name
    let parsed = parse_side_coin(&bal.coin);
    let outcome_name = if let Some((oid, side_idx)) = parsed {
        side_names
            .get(&oid)
            .map(|names| {
                if side_idx == 0 {
                    names.0.clone()
                } else {
                    names.1.clone()
                }
            })
            .unwrap_or_else(|| format!("Side {side_idx}"))
    } else {
        bal.coin.clone()
    };

    let entry_ntl: f64 = bal.entry_ntl.parse().unwrap_or(0.0);
    let avg_cost = if total != 0.0 { entry_ntl / total } else { 0.0 };

    let current_price: f64 = all_mids
        .get(&bal.coin)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    let unrealized_pnl = (current_price - avg_cost) * total;
    let potential_payout = total;

    let names = name_map.get(&market_id);

    Some(PredictionPosition {
        market_id,
        event_title: names.map(|n| n.0.clone()).unwrap_or_default(),
        market_question: names.map(|n| n.1.clone()).unwrap_or_default(),
        outcome,
        outcome_name,
        shares: format!("{total:.6}"),
        avg_cost: format!("{avg_cost:.6}"),
        current_price: all_mids
            .get(&bal.coin)
            .cloned()
            .unwrap_or_else(|| "0".to_string()),
        unrealized_pnl: format!("{unrealized_pnl:.6}"),
        potential_payout: format!("{potential_payout:.6}"),
        event_status: EventStatus::Active,
    })
}

fn map_fill(raw: &HLFill) -> Option<PredictionActivity> {
    if !is_outcome_coin(&raw.coin) {
        return None;
    }

    let outcome_id = coin_outcome_id(&raw.coin);
    let market_id = outcome_id.map(|id| id.to_string()).unwrap_or_else(|| raw.coin.clone());

    Some(PredictionActivity {
        id: raw.tid.to_string(),
        activity_type: ActivityType::Trade,
        market_id: Some(market_id),
        outcome: Some(raw.coin.clone()),
        side: Some(if raw.side == "B" { "buy" } else { "sell" }.to_string()),
        price: Some(raw.px.clone()),
        size: Some(raw.sz.clone()),
        amount: None,
        timestamp: raw.time,
    })
}

/// Minimal event data source for building name maps.
pub trait EventDataSource: Send + Sync {
    fn fetch_events(
        &self,
        limit: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<PredictionEvent>> + Send + '_>>;
}

pub struct HIP4Account {
    client: Arc<HIP4Client>,
    side_names: SideNameResolver,
}

impl HIP4Account {
    pub fn new(client: Arc<HIP4Client>, side_names: SideNameResolver) -> Self {
        Self { client, side_names }
    }

    pub async fn fetch_positions(&self, address: &str) -> Result<Vec<PredictionPosition>> {
        let (state, all_mids) = tokio::try_join!(
            self.client.fetch_spot_clearinghouse_state(address),
            self.client.fetch_all_mids(),
        )?;

        let side_names = self.side_names.read().await;
        // Empty name map for now (would need event adapter reference for full names)
        let name_map: HashMap<String, (String, String)> = HashMap::new();

        let positions: Vec<PredictionPosition> = state
            .balances
            .iter()
            .filter(|bal| is_outcome_coin(&bal.coin))
            .filter(|bal| bal.total.parse::<f64>().unwrap_or(0.0) != 0.0)
            .filter_map(|bal| map_spot_balance(bal, &all_mids, &name_map, &side_names))
            .collect();

        Ok(positions)
    }

    pub async fn fetch_activity(&self, address: &str) -> Result<Vec<PredictionActivity>> {
        let now = chrono::Utc::now().timestamp_millis();
        let thirty_days_ms: i64 = 30 * 24 * 60 * 60 * 1000;
        let start_time = now - thirty_days_ms;

        let fills = self
            .client
            .fetch_user_fills_by_time(address, start_time, now)
            .await?;

        Ok(fills.iter().filter_map(map_fill).collect())
    }

    pub async fn fetch_balance(
        &self,
        address: &str,
    ) -> Result<Vec<SpotBalance>> {
        let state = self.client.fetch_spot_clearinghouse_state(address).await?;
        Ok(state
            .balances
            .iter()
            .map(|b| SpotBalance {
                coin: b.coin.clone(),
                total: b.total.clone(),
                hold: b.hold.clone(),
            })
            .collect())
    }

    pub async fn fetch_open_orders(
        &self,
        address: &str,
    ) -> Result<Vec<OpenOrder>> {
        let orders = self.client.fetch_frontend_open_orders(address).await?;
        Ok(orders
            .iter()
            .map(|o| OpenOrder {
                coin: o.coin.clone(),
                side: o.side.clone(),
                limit_px: o.limit_px.clone(),
                sz: o.sz.clone(),
                oid: o.oid,
                timestamp: o.timestamp,
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct SpotBalance {
    pub coin: String,
    pub total: String,
    pub hold: String,
}

#[derive(Debug, Clone)]
pub struct OpenOrder {
    pub coin: String,
    pub side: String,
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: i64,
}
