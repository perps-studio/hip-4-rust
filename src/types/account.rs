use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionPosition {
    pub market_id: String,
    pub event_title: String,
    pub market_question: String,
    pub outcome: String,
    pub outcome_name: String,
    pub shares: String,
    pub avg_cost: String,
    pub current_price: String,
    pub unrealized_pnl: String,
    pub potential_payout: String,
    pub event_status: EventStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Active,
    PendingResolution,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionActivity {
    pub id: String,
    #[serde(rename = "type")]
    pub activity_type: ActivityType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    Trade,
    Redeem,
    Deposit,
    Withdrawal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredictionAuthState {
    pub status: AuthStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    Disconnected,
    PendingApproval,
    Ready,
}
