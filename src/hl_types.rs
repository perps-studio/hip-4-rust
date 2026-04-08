use serde::{Deserialize, Serialize};

// -- outcomeMeta --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLOutcomeMeta {
    pub outcomes: Vec<HLOutcome>,
    pub questions: Vec<HLQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLOutcome {
    pub outcome: u64,
    pub name: String,
    pub description: String,
    #[serde(rename = "sideSpecs")]
    pub side_specs: Vec<HLSideSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLSideSpec {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLQuestion {
    pub question: u64,
    pub name: String,
    pub description: String,
    #[serde(rename = "fallbackOutcome")]
    pub fallback_outcome: u64,
    #[serde(rename = "namedOutcomes")]
    pub named_outcomes: Vec<u64>,
    #[serde(rename = "settledNamedOutcomes")]
    pub settled_named_outcomes: Vec<u64>,
}

// -- L2 book --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLL2Book {
    pub coin: String,
    pub time: i64,
    pub levels: (Vec<HLL2Level>, Vec<HLL2Level>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLL2Level {
    pub px: String,
    pub sz: String,
    pub n: u32,
}

// -- Trades --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLTrade {
    pub coin: String,
    pub side: String,
    pub px: String,
    pub sz: String,
    pub time: i64,
    pub hash: String,
    pub tid: u64,
    #[serde(default)]
    pub users: Option<(String, String)>,
}

// -- Candles --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLCandle {
    pub t: i64,
    #[serde(rename = "T")]
    pub close_time: i64,
    pub s: String,
    pub i: String,
    pub o: String,
    pub c: String,
    pub h: String,
    pub l: String,
    pub v: String,
    pub n: u32,
}

// -- allMids --

pub type HLAllMids = std::collections::HashMap<String, String>;

// -- clearinghouseState --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLClearinghouseState {
    #[serde(rename = "marginSummary")]
    pub margin_summary: HLMarginSummary,
    #[serde(rename = "crossMarginSummary")]
    pub cross_margin_summary: HLMarginSummary,
    pub withdrawable: String,
    #[serde(rename = "assetPositions")]
    pub asset_positions: Vec<HLAssetPosition>,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLMarginSummary {
    #[serde(rename = "accountValue")]
    pub account_value: String,
    #[serde(rename = "totalNtlPos")]
    pub total_ntl_pos: String,
    #[serde(rename = "totalRawUsd")]
    pub total_raw_usd: String,
    #[serde(rename = "totalMarginUsed")]
    pub total_margin_used: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLAssetPosition {
    #[serde(rename = "type")]
    pub position_type: String,
    pub position: HLPositionData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLPositionData {
    pub coin: String,
    pub szi: String,
    #[serde(rename = "entryPx")]
    pub entry_px: String,
    #[serde(rename = "positionValue")]
    pub position_value: String,
    #[serde(rename = "unrealizedPnl")]
    pub unrealized_pnl: String,
}

// -- spotClearinghouseState --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLSpotClearinghouseState {
    pub balances: Vec<HLSpotBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLSpotBalance {
    pub coin: String,
    pub token: u64,
    pub hold: String,
    pub total: String,
    #[serde(rename = "entryNtl")]
    pub entry_ntl: String,
}

// -- frontendOpenOrders --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLFrontendOrder {
    pub coin: String,
    pub side: String,
    #[serde(rename = "limitPx")]
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: i64,
    #[serde(rename = "origSz")]
    pub orig_sz: String,
    #[serde(rename = "reduceOnly")]
    pub reduce_only: bool,
    #[serde(rename = "orderType")]
    pub order_type: String,
    pub tif: Option<String>,
    pub cloid: Option<String>,
}

// -- userFills --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLFill {
    pub coin: String,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub time: i64,
    #[serde(rename = "startPosition")]
    pub start_position: String,
    pub dir: String,
    #[serde(rename = "closedPnl")]
    pub closed_pnl: String,
    pub hash: String,
    pub oid: u64,
    pub crossed: bool,
    pub fee: String,
    pub tid: u64,
    #[serde(rename = "feeToken")]
    pub fee_token: String,
}

// -- Exchange response --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLExchangeResponse {
    pub status: String,
    pub response: Option<HLExchangeResponseData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLExchangeResponseData {
    #[serde(rename = "type")]
    pub response_type: String,
    pub data: HLExchangeResponseStatuses,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLExchangeResponseStatuses {
    pub statuses: Vec<serde_json::Value>,
}

// -- Signature --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLSignature {
    pub r: String,
    pub s: String,
    pub v: u8,
}

impl HLSignature {
    /// Split a hex signature (0x + 32 bytes r + 32 bytes s + 1 byte v).
    pub fn from_hex(hex: &str) -> Self {
        let raw = hex.strip_prefix("0x").unwrap_or(hex);
        Self {
            r: format!("0x{}", &raw[..64]),
            s: format!("0x{}", &raw[64..128]),
            v: u8::from_str_radix(&raw[128..130], 16).unwrap_or(27),
        }
    }
}

// -- WebSocket messages --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLWsMessage {
    pub channel: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLWsL2BookData {
    pub coin: String,
    pub time: i64,
    pub levels: (Vec<HLWsLevel>, Vec<HLWsLevel>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLWsLevel {
    pub px: String,
    pub sz: String,
    pub n: u32,
}

// -- Spot asset context --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HLSpotAssetCtx {
    #[serde(rename = "markPx")]
    pub mark_px: Option<String>,
    #[serde(rename = "midPx")]
    pub mid_px: Option<String>,
    pub coin: Option<String>,
}
