use std::sync::Arc;
use tokio::sync::RwLock;

use crate::auth::AuthInner;
use crate::client::HIP4Client;
use crate::hl_types::HLExchangeResponse;
use crate::signing::{sign_l1_action, sort_order_action};
use crate::crypto::eip712::sign_user_signed_action;
use crate::types::trading::WalletActionResult;

/// USDH spot market index on Hyperliquid.
const USDH_SPOT_INDEX: u64 = 1338;
/// USDH asset ID for order placement.
const USDH_ASSET_ID: u64 = 10000 + USDH_SPOT_INDEX;

pub struct HIP4Wallet {
    client: Arc<HIP4Client>,
    auth: Arc<RwLock<AuthInner>>,
}

impl HIP4Wallet {
    pub fn new(client: Arc<HIP4Client>, auth: Arc<RwLock<AuthInner>>) -> Self {
        Self { client, auth }
    }

    pub async fn buy_usdh(&self, amount: &str) -> WalletActionResult {
        self.execute_spot_order(true, amount).await
    }

    pub async fn sell_usdh(&self, amount: &str) -> WalletActionResult {
        self.execute_spot_order(false, amount).await
    }

    pub async fn transfer_to_spot(&self, amount: &str) -> WalletActionResult {
        self.usd_class_transfer(amount, false).await
    }

    pub async fn transfer_to_perps(&self, amount: &str) -> WalletActionResult {
        self.usd_class_transfer(amount, true).await
    }

    pub async fn withdraw(&self, destination: &str, amount: &str) -> WalletActionResult {
        self.execute_user_signed(
            "withdraw3",
            &serde_json::json!({"destination": destination, "amount": amount}),
            "time",
        )
        .await
    }

    pub async fn usd_send(&self, destination: &str, amount: &str) -> WalletActionResult {
        self.execute_user_signed(
            "usdSend",
            &serde_json::json!({"destination": destination, "amount": amount}),
            "time",
        )
        .await
    }

    async fn usd_class_transfer(&self, amount: &str, to_perp: bool) -> WalletActionResult {
        self.execute_user_signed(
            "usdClassTransfer",
            &serde_json::json!({"amount": amount, "toPerp": to_perp}),
            "nonce",
        )
        .await
    }

    async fn execute_spot_order(&self, is_buy: bool, amount: &str) -> WalletActionResult {
        let inner = self.auth.read().await;
        let signer = match &inner.signer {
            Some(s) => s,
            None => {
                return WalletActionResult::error(
                    "Not authenticated. Call auth.init_auth() first.",
                )
            }
        };

        // Fetch oracle price
        let ctx = match self.client.fetch_spot_asset_ctx(USDH_SPOT_INDEX).await {
            Ok(Some(ctx)) => ctx,
            _ => return WalletActionResult::error("Could not fetch USDH oracle price"),
        };

        let oracle: f64 = ctx
            .mark_px
            .as_deref()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0.0);
        if oracle <= 0.0 {
            return WalletActionResult::error("Could not fetch USDH oracle price");
        }

        let price = if is_buy {
            format!("{:.5}", oracle * 1.1)
        } else {
            format!("{:.5}", oracle * 0.9)
        };

        // Build wire format with canonical key order
        let mut order_wire = serde_json::Map::new();
        order_wire.insert("a".into(), serde_json::json!(USDH_ASSET_ID));
        order_wire.insert("b".into(), serde_json::json!(is_buy));
        order_wire.insert("p".into(), serde_json::json!(price));
        order_wire.insert("s".into(), serde_json::json!(amount));
        order_wire.insert("r".into(), serde_json::json!(false));
        order_wire.insert(
            "t".into(),
            serde_json::json!({"limit": {"tif": "Ioc"}}),
        );

        let action = serde_json::json!({
            "type": "order",
            "orders": [serde_json::Value::Object(order_wire)],
            "grouping": "na"
        });
        let sorted_action = sort_order_action(&action);

        let nonce = inner.nonce.next();
        let is_testnet = inner.testnet;

        let signature = match sign_l1_action(signer, &sorted_action, nonce, is_testnet, None) {
            Ok(sig) => sig,
            Err(e) => return WalletActionResult::error(format!("Signing failed: {e}")),
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
                    return WalletActionResult::error("Exchange returned non-ok status");
                }
                if let Some(resp) = &res.response {
                    if let Some(first) = resp.data.statuses.first() {
                        if let Some(filled) = first.get("filled") {
                            return WalletActionResult::filled(
                                filled["totalSz"]
                                    .as_str()
                                    .unwrap_or("0")
                                    .to_string(),
                                filled["avgPx"]
                                    .as_str()
                                    .unwrap_or("0")
                                    .to_string(),
                            );
                        }
                        if let Some(error) = first.get("error") {
                            return WalletActionResult::error(
                                error.as_str().unwrap_or("Unknown error"),
                            );
                        }
                    }
                }
                WalletActionResult::ok()
            }
            Err(e) => WalletActionResult::error(format!("{e}")),
        }
    }

    async fn execute_user_signed(
        &self,
        action_type: &str,
        fields: &serde_json::Value,
        nonce_field_name: &str,
    ) -> WalletActionResult {
        let inner = self.auth.read().await;
        let signer = match &inner.signer {
            Some(s) => s.clone(),
            None => {
                return WalletActionResult::error(
                    "Not authenticated. Call auth.init_auth() first.",
                )
            }
        };
        let testnet = inner.testnet;
        drop(inner);

        let nonce = chrono::Utc::now().timestamp_millis() as u64;

        let mut action_fields = fields.clone();
        if let Some(obj) = action_fields.as_object_mut() {
            obj.insert(nonce_field_name.to_string(), serde_json::json!(nonce));
        }

        let signature =
            match sign_user_signed_action(&signer, action_type, &action_fields, testnet) {
                Ok(sig) => sig,
                Err(e) => return WalletActionResult::error(format!("Signing failed: {e}")),
            };

        let chain = if testnet { "Testnet" } else { "Mainnet" };
        let mut action = serde_json::json!({
            "type": action_type,
            "signatureChainId": "0x66eee",
            "hyperliquidChain": chain,
        });
        if let (Some(act_obj), Some(field_obj)) =
            (action.as_object_mut(), action_fields.as_object())
        {
            for (k, v) in field_obj {
                act_obj.insert(k.clone(), v.clone());
            }
        }

        let body = serde_json::json!({
            "action": action,
            "nonce": nonce,
            "signature": signature.to_hl_json(),
        });

        match self
            .client
            .exchange_post::<serde_json::Value>(&body)
            .await
        {
            Ok(result) => {
                if result.get("status").and_then(|s| s.as_str()) == Some("ok") {
                    WalletActionResult::ok()
                } else {
                    let error_msg = result
                        .get("response")
                        .and_then(|r| {
                            r.as_str()
                                .map(|s| s.to_string())
                                .or_else(|| {
                                    r.get("error")
                                        .and_then(|e| e.as_str())
                                        .map(|s| s.to_string())
                                })
                        })
                        .unwrap_or_else(|| "Action failed".to_string());
                    WalletActionResult::error(error_msg)
                }
            }
            Err(e) => WalletActionResult::error(format!("{e}")),
        }
    }
}
