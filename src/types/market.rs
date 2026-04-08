use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionOrderBook {
    pub market_id: String,
    pub bids: Vec<PredictionOrderBookLevel>,
    pub asks: Vec<PredictionOrderBookLevel>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionOrderBookLevel {
    pub price: String,
    pub size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionTrade {
    pub id: String,
    pub market_id: String,
    pub outcome: String,
    pub side: String,
    pub price: String,
    pub size: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionPrice {
    pub market_id: String,
    pub outcomes: Vec<PredictionPriceOutcome>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionPriceOutcome {
    pub name: String,
    pub price: String,
    pub midpoint: String,
}
