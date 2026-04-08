use std::time::Duration;

use crate::error::{HIP4Error, Result};
use crate::hl_types::*;

const MAINNET_INFO_URL: &str = "https://api.hyperliquid.xyz/info";
const MAINNET_EXCHANGE_URL: &str = "https://api.hyperliquid.xyz/exchange";
const TESTNET_INFO_URL: &str = "https://api-ui.hyperliquid-testnet.xyz/info";
const TESTNET_EXCHANGE_URL: &str = "https://api-ui.hyperliquid-testnet.xyz/exchange";

pub const TESTNET_WS_URL: &str = "wss://api-ui.hyperliquid-testnet.xyz/ws";
pub const MAINNET_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";

// ---------------------------------------------------------------------------
// Coin-name helpers
// ---------------------------------------------------------------------------

/// Outcome-level coin name (AMM instrument): `@<outcomeId>`
pub fn outcome_coin(outcome_id: u64) -> String {
    format!("@{}", outcome_id)
}

/// Per-side probability coin name: `#<outcomeId><sideIndex>`
pub fn side_coin(outcome_id: u64, side_index: u64) -> String {
    format!("#{}{}", outcome_id, side_index)
}

/// Asset ID for order placement: `100_000_000 + outcomeId * 10 + sideIndex`
pub fn side_asset_id(outcome_id: u64, side_index: u64) -> u64 {
    100_000_000 + outcome_id * 10 + side_index
}

/// Parse a side coin like "#5160" or "+5160" into (outcomeId, sideIndex).
pub fn parse_side_coin(coin: &str) -> Option<(u64, u64)> {
    if !coin.starts_with('#') && !coin.starts_with('+') {
        return None;
    }
    let num = &coin[1..];
    if num.len() < 2 {
        return None;
    }
    let side_index: u64 = num[num.len() - 1..].parse().ok()?;
    let outcome_id: u64 = num[..num.len() - 1].parse().ok()?;
    if side_index > 1 {
        return None;
    }
    Some((outcome_id, side_index))
}

/// Parse an outcome-level coin like "@1338" into outcomeId.
pub fn parse_outcome_coin(coin: &str) -> Option<u64> {
    if !coin.starts_with('@') {
        return None;
    }
    coin[1..].parse().ok()
}

/// Extract the outcome ID from either @, #, or + coin format.
pub fn coin_outcome_id(coin: &str) -> Option<u64> {
    if coin.starts_with('#') || coin.starts_with('+') {
        parse_side_coin(coin).map(|(id, _)| id)
    } else if coin.starts_with('@') {
        parse_outcome_coin(coin)
    } else {
        None
    }
}

/// Check if a coin is an outcome-level or side-level instrument.
pub fn is_outcome_coin(coin: &str) -> bool {
    coin.starts_with('@') || coin.starts_with('#') || coin.starts_with('+')
}

// ---------------------------------------------------------------------------
// HIP4Client
// ---------------------------------------------------------------------------

pub struct HIP4Client {
    http: reqwest::Client,
    pub info_url: String,
    pub exchange_url: String,
    pub ws_url: String,
    pub testnet: bool,
}

impl HIP4Client {
    pub fn new(testnet: bool, info_url: Option<&str>, exchange_url: Option<&str>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            info_url: info_url
                .unwrap_or(if testnet { TESTNET_INFO_URL } else { MAINNET_INFO_URL })
                .to_string(),
            exchange_url: exchange_url
                .unwrap_or(if testnet { TESTNET_EXCHANGE_URL } else { MAINNET_EXCHANGE_URL })
                .to_string(),
            ws_url: if testnet { TESTNET_WS_URL } else { MAINNET_WS_URL }.to_string(),
            testnet,
        }
    }

    // -- Info endpoints --

    pub async fn fetch_outcome_meta(&self) -> Result<HLOutcomeMeta> {
        self.info_post(&serde_json::json!({"type": "outcomeMeta"})).await
    }

    pub async fn fetch_l2_book(&self, coin: &str) -> Result<HLL2Book> {
        self.info_post(&serde_json::json!({"type": "l2Book", "coin": coin}))
            .await
    }

    pub async fn fetch_recent_trades(&self, coin: &str) -> Result<Vec<HLTrade>> {
        self.info_post(&serde_json::json!({"type": "recentTrades", "coin": coin}))
            .await
    }

    pub async fn fetch_all_mids(&self) -> Result<HLAllMids> {
        self.info_post(&serde_json::json!({"type": "allMids"})).await
    }

    pub async fn fetch_candle_snapshot(
        &self,
        coin: &str,
        interval: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<HLCandle>> {
        self.info_post(&serde_json::json!({
            "type": "candleSnapshot",
            "req": {"coin": coin, "interval": interval, "startTime": start_time, "endTime": end_time}
        }))
        .await
    }

    pub async fn fetch_spot_clearinghouse_state(
        &self,
        user: &str,
    ) -> Result<HLSpotClearinghouseState> {
        self.info_post(&serde_json::json!({"type": "spotClearinghouseState", "user": user}))
            .await
    }

    pub async fn fetch_user_fills_by_time(
        &self,
        user: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<HLFill>> {
        self.info_post(&serde_json::json!({
            "type": "userFillsByTime",
            "user": user,
            "startTime": start_time,
            "endTime": end_time,
            "aggregateByTime": true,
            "reversed": true,
        }))
        .await
    }

    pub async fn fetch_frontend_open_orders(&self, user: &str) -> Result<Vec<HLFrontendOrder>> {
        self.info_post(&serde_json::json!({"type": "frontendOpenOrders", "user": user}))
            .await
    }

    pub async fn fetch_spot_asset_ctx(&self, spot_index: u64) -> Result<Option<HLSpotAssetCtx>> {
        let data: (serde_json::Value, Vec<HLSpotAssetCtx>) = self
            .info_post(&serde_json::json!({"type": "spotMetaAndAssetCtxs"}))
            .await?;

        Ok(data.1.get(spot_index as usize).cloned().filter(|ctx| ctx.mark_px.is_some()))
    }

    // -- Exchange endpoints --

    pub async fn exchange_post<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .http
            .post(&self.exchange_url)
            .json(body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(HIP4Error::Api {
                status: status.as_u16(),
                message: text,
            });
        }

        let text = resp.text().await?;
        serde_json::from_str(&text).map_err(|e| HIP4Error::Api {
            status: status.as_u16(),
            message: format!("JSON parse error: {e} - body: {text}"),
        })
    }

    // -- Internal --

    async fn info_post<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T> {
        match self.do_info_post(body).await {
            Ok(v) => Ok(v),
            Err(HIP4Error::Api { status, .. }) if (400..500).contains(&status) => {
                Err(HIP4Error::Api {
                    status,
                    message: "Client error, not retrying".into(),
                })
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
                self.do_info_post(body).await
            }
        }
    }

    async fn do_info_post<T: serde::de::DeserializeOwned>(
        &self,
        body: &serde_json::Value,
    ) -> Result<T> {
        let resp = self.http.post(&self.info_url).json(body).send().await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(HIP4Error::Api {
                status: status.as_u16(),
                message: format!("HL info API responded with {}: {}", status.as_u16(), text),
            });
        }

        let text = resp.text().await?;
        serde_json::from_str(&text).map_err(|_| HIP4Error::Api {
            status: status.as_u16(),
            message: "Exchange returned non-JSON response".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_side_coin() {
        assert_eq!(side_coin(516, 0), "#5160");
        assert_eq!(side_coin(516, 1), "#5161");
    }

    #[test]
    fn test_side_asset_id() {
        assert_eq!(side_asset_id(516, 0), 100005160);
        assert_eq!(side_asset_id(516, 1), 100005161);
    }

    #[test]
    fn test_parse_side_coin() {
        assert_eq!(parse_side_coin("#5160"), Some((516, 0)));
        assert_eq!(parse_side_coin("#5161"), Some((516, 1)));
        assert_eq!(parse_side_coin("+5160"), Some((516, 0)));
        assert_eq!(parse_side_coin("@516"), None);
        assert_eq!(parse_side_coin("#5162"), None);
    }

    #[test]
    fn test_parse_outcome_coin() {
        assert_eq!(parse_outcome_coin("@1338"), Some(1338));
        assert_eq!(parse_outcome_coin("#1338"), None);
    }

    #[test]
    fn test_is_outcome_coin() {
        assert!(is_outcome_coin("@1338"));
        assert!(is_outcome_coin("#5160"));
        assert!(is_outcome_coin("+5160"));
        assert!(!is_outcome_coin("BTC"));
    }
}
