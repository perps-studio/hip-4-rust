use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    GTC,
    GTD,
    FOK,
    FAK,
}

#[derive(Debug, Clone)]
pub struct PredictionOrderParams {
    pub market_id: String,
    pub outcome: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<String>,
    pub amount: String,
    pub time_in_force: Option<TimeInForce>,
    pub expiration: Option<String>,
    /// Mark price (0-1), used for minimum notional validation.
    pub mark_px: Option<f64>,
    /// Builder address for referral fees.
    pub builder_address: Option<String>,
    /// Builder fee in tenths of a basis point (0-1000).
    pub builder_fee: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionOrderResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PredictionOrderResult {
    pub fn success(order_id: String, status: &str, shares: Option<String>) -> Self {
        Self {
            success: true,
            order_id: Some(order_id),
            status: Some(status.to_string()),
            shares,
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            order_id: None,
            status: Some("error".to_string()),
            shares: None,
            error: Some(msg.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PredictionCancelParams {
    pub market_id: String,
    pub order_id: String,
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletActionResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filled_sz: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_px: Option<String>,
}

impl WalletActionResult {
    pub fn ok() -> Self {
        Self { success: true, error: None, filled_sz: None, avg_px: None }
    }

    pub fn filled(sz: String, px: String) -> Self {
        Self { success: true, error: None, filled_sz: Some(sz), avg_px: Some(px) }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self { success: false, error: Some(msg.into()), filled_sz: None, avg_px: None }
    }
}
