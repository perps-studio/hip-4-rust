use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionEvent {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub markets: Vec<PredictionMarket>,
    pub total_volume: String,
    pub end_date: String,
    pub status: PredictionEventStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionEventStatus {
    Active,
    PendingResolution,
    Resolved,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionMarket {
    pub id: String,
    pub event_id: String,
    pub question: String,
    pub outcomes: Vec<PredictionOutcome>,
    pub volume: String,
    pub liquidity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_neg_risk: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionOutcome {
    pub name: String,
    pub token_id: String,
    pub price: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionCategory {
    pub id: String,
    pub name: String,
    pub slug: String,
}
