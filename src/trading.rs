use std::sync::Arc;
use tokio::sync::RwLock;

use crate::auth::AuthInner;
use crate::client::{side_asset_id, HIP4Client};
use crate::error::HIP4Error;
use crate::hl_types::HLExchangeResponse;
use crate::pricing::{format_price, get_min_shares, strip_zeros, MIN_NOTIONAL};
use crate::signing::{sign_l1_action, sort_cancel_action, sort_order_action};
use crate::types::trading::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_asset_id(market_id: &str, outcome: &str) -> Result<u64, HIP4Error> {
    let outcome_id: u64 = market_id
        .parse()
        .map_err(|_| HIP4Error::Validation(format!("Invalid market_id: {market_id}")))?;

    if outcome.starts_with('#') || outcome.starts_with('+') {
        let num = &outcome[1..];
        let side_index: u64 = num[num.len() - 1..]
            .parse()
            .map_err(|_| HIP4Error::Validation(format!("Invalid outcome: {outcome}")))?;
        if side_index > 1 {
            return Err(HIP4Error::Validation(format!(
                "Invalid sideIndex {side_index} in outcome \"{outcome}\". Must be 0 or 1."
            )));
        }
        return Ok(side_asset_id(outcome_id, side_index));
    }

    if let Some(last) = outcome.chars().last() {
        if last.is_ascii_digit() {
            let side_index = last.to_digit(10).unwrap() as u64;
            if side_index <= 1 {
                return Ok(side_asset_id(outcome_id, side_index));
            }
        }
    }

    Ok(side_asset_id(outcome_id, 0))
}

fn map_tif(order_type: OrderType, tif: Option<TimeInForce>) -> serde_json::Value {
    if order_type == OrderType::Market {
        return serde_json::json!({"limit": {"tif": "FrontendMarket"}});
    }
    match tif {
        Some(TimeInForce::FOK) | Some(TimeInForce::FAK) => {
            serde_json::json!({"limit": {"tif": "Ioc"}})
        }
        _ => serde_json::json!({"limit": {"tif": "Gtc"}}),
    }
}

fn interpret_status(status: &serde_json::Value) -> PredictionOrderResult {
    if let Some(filled) = status.get("filled") {
        return PredictionOrderResult::success(
            filled["oid"].as_u64().unwrap_or(0).to_string(),
            "filled",
            filled["totalSz"].as_str().map(|s| s.to_string()),
        );
    }
    if let Some(resting) = status.get("resting") {
        return PredictionOrderResult::success(
            resting["oid"].as_u64().unwrap_or(0).to_string(),
            "resting",
            None,
        );
    }
    if let Some(error) = status.as_str() {
        return PredictionOrderResult::error(error);
    }
    if let Some(error) = status.get("error") {
        return PredictionOrderResult::error(
            error.as_str().unwrap_or("Unknown error"),
        );
    }
    PredictionOrderResult::error("Unknown order status")
}

// ---------------------------------------------------------------------------
// HIP4Trading
// ---------------------------------------------------------------------------

/// HIP4 trading adapter. Places/cancels orders using from-scratch signing.
pub struct HIP4Trading {
    client: Arc<HIP4Client>,
    auth: Arc<RwLock<AuthInner>>,
}

impl HIP4Trading {
    pub fn new(client: Arc<HIP4Client>, auth: Arc<RwLock<AuthInner>>) -> Self {
        Self { client, auth }
    }

    pub async fn place_order(&self, params: PredictionOrderParams) -> PredictionOrderResult {
        let inner = self.auth.read().await;
        let signer = match &inner.signer {
            Some(s) => s,
            None => {
                return PredictionOrderResult::error(
                    "Not authenticated. Call auth.init_auth() first.",
                )
            }
        };

        let asset_id = match resolve_asset_id(&params.market_id, &params.outcome) {
            Ok(id) => id,
            Err(e) => return PredictionOrderResult::error(e.to_string()),
        };

        let is_buy = params.side == Side::Buy;
        let amount = strip_zeros(&params.amount);

        let price: String = if params.order_type == OrderType::Market {
            if is_buy { "0.99999" } else { "0.00001" }.to_string()
        } else {
            let raw_price: f64 = params
                .price
                .as_deref()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0.0);
            let formatted = format_price(raw_price);

            let numeric_price: f64 = formatted.parse().unwrap_or(0.0);
            let numeric_size: f64 = amount.parse().unwrap_or(0.0);

            if let Some(mark_px) = params.mark_px {
                let min_shares = get_min_shares(mark_px);
                if numeric_size < min_shares {
                    return PredictionOrderResult::error(format!(
                        "Size {numeric_size} below minimum {min_shares} shares (markPx={mark_px})"
                    ));
                }
            }

            let notional = if let Some(mark_px) = params.mark_px {
                numeric_size * mark_px.min(1.0 - mark_px)
            } else {
                numeric_price * numeric_size
            };
            if notional < MIN_NOTIONAL {
                return PredictionOrderResult::error(format!(
                    "Notional ${notional:.2} below minimum ${MIN_NOTIONAL}"
                ));
            }

            formatted
        };

        // Build wire format (canonical key order)
        let mut order_wire = serde_json::Map::new();
        order_wire.insert("a".into(), serde_json::json!(asset_id));
        order_wire.insert("b".into(), serde_json::json!(is_buy));
        order_wire.insert("p".into(), serde_json::json!(price));
        order_wire.insert("s".into(), serde_json::json!(amount));
        order_wire.insert("r".into(), serde_json::json!(false));
        order_wire.insert(
            "t".into(),
            map_tif(params.order_type, params.time_in_force),
        );

        let mut action = serde_json::Map::new();
        action.insert("type".into(), serde_json::json!("order"));
        action.insert(
            "orders".into(),
            serde_json::json!([serde_json::Value::Object(order_wire)]),
        );
        action.insert("grouping".into(), serde_json::json!("na"));

        // Builder fee support
        if let (Some(addr), Some(fee)) = (&params.builder_address, params.builder_fee) {
            if fee > 0 {
                let mut builder = serde_json::Map::new();
                builder.insert("b".into(), serde_json::json!(addr.to_lowercase()));
                builder.insert("f".into(), serde_json::json!(fee));
                action.insert("builder".into(), serde_json::Value::Object(builder));
            }
        }

        let sorted_action = sort_order_action(&serde_json::Value::Object(action.clone()));

        let nonce = inner.nonce.next();
        let is_testnet = inner.testnet;

        let signature = match sign_l1_action(signer, &sorted_action, nonce, is_testnet, None) {
            Ok(sig) => sig,
            Err(e) => return PredictionOrderResult::error(format!("Signing failed: {e}")),
        };

        let body = serde_json::json!({
            "action": sorted_action,
            "nonce": nonce,
            "signature": signature.to_hl_json(),
            "vaultAddress": null,
        });

        match self
            .client
            .exchange_post::<HLExchangeResponse>(&body)
            .await
        {
            Ok(res) => {
                if res.status != "ok" {
                    return PredictionOrderResult::error("Exchange returned non-ok status");
                }
                if let Some(resp) = &res.response {
                    if let Some(first) = resp.data.statuses.first() {
                        return interpret_status(first);
                    }
                }
                PredictionOrderResult::error("No order status returned")
            }
            Err(e) => PredictionOrderResult::error(format!("{e}")),
        }
    }

    pub async fn cancel_order(&self, params: PredictionCancelParams) -> crate::error::Result<()> {
        let inner = self.auth.read().await;
        let signer = match &inner.signer {
            Some(s) => s,
            None => {
                return Err(HIP4Error::Auth(
                    "Not authenticated. Call auth.init_auth() first.".into(),
                ))
            }
        };

        let outcome_id: u64 = params
            .market_id
            .parse()
            .map_err(|_| {
                HIP4Error::Validation(format!("Invalid market_id: {}", params.market_id))
            })?;

        let asset_id = if let Some(outcome) = &params.outcome {
            resolve_asset_id(&params.market_id, outcome)?
        } else {
            side_asset_id(outcome_id, 0)
        };

        let oid: u64 = params
            .order_id
            .parse()
            .map_err(|_| {
                HIP4Error::Validation(format!("Invalid order_id: {}", params.order_id))
            })?;

        let cancel_action = sort_cancel_action(&serde_json::json!({
            "type": "cancel",
            "cancels": [{"a": asset_id, "o": oid}]
        }));

        let nonce = inner.nonce.next();
        let is_testnet = inner.testnet;

        let signature = sign_l1_action(signer, &cancel_action, nonce, is_testnet, None)
            .map_err(|e| HIP4Error::Signing(e))?;

        let body = serde_json::json!({
            "action": cancel_action,
            "nonce": nonce,
            "signature": signature.to_hl_json(),
            "vaultAddress": null,
        });

        self.client
            .exchange_post::<serde_json::Value>(&body)
            .await?;

        Ok(())
    }
}
