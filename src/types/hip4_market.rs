use serde::{Deserialize, Serialize};

use crate::hl_types::{HLOutcome, HLQuestion};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarketType {
    DefaultBinary,
    LabelledBinary,
    MultiOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSide {
    pub name: String,
    pub coin_num: u64,
    pub coin: String,
    pub asset: u64,
}

#[derive(Debug, Clone)]
pub struct BaseMarket {
    pub market_type: MarketType,
    pub outcome_id: u64,
    pub name: String,
    pub description: String,
    pub sides: [MarketSide; 2],
    pub raw: HLOutcome,
}

#[derive(Debug, Clone)]
pub struct DefaultBinaryMarket {
    pub base: BaseMarket,
    pub underlying: String,
    pub target_price: f64,
    pub expiry: chrono::DateTime<chrono::Utc>,
    pub period: String,
}

#[derive(Debug, Clone)]
pub struct LabelledBinaryMarket {
    pub base: BaseMarket,
}

#[derive(Debug, Clone)]
pub struct MultiOutcomeMarket {
    pub base: BaseMarket,
    pub question_id: u64,
    pub question_name: String,
    pub question_description: String,
    pub is_fallback: bool,
    pub raw_question: HLQuestion,
}

#[derive(Debug, Clone)]
pub enum HIP4Market {
    DefaultBinary(DefaultBinaryMarket),
    LabelledBinary(LabelledBinaryMarket),
    MultiOutcome(MultiOutcomeMarket),
}

impl HIP4Market {
    pub fn market_type(&self) -> MarketType {
        match self {
            HIP4Market::DefaultBinary(_) => MarketType::DefaultBinary,
            HIP4Market::LabelledBinary(_) => MarketType::LabelledBinary,
            HIP4Market::MultiOutcome(_) => MarketType::MultiOutcome,
        }
    }

    pub fn base(&self) -> &BaseMarket {
        match self {
            HIP4Market::DefaultBinary(m) => &m.base,
            HIP4Market::LabelledBinary(m) => &m.base,
            HIP4Market::MultiOutcome(m) => &m.base,
        }
    }

    pub fn outcome_id(&self) -> u64 {
        self.base().outcome_id
    }

    pub fn name(&self) -> &str {
        &self.base().name
    }

    pub fn sides(&self) -> &[MarketSide; 2] {
        &self.base().sides
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBy {
    Type,
    Question,
}

#[derive(Debug, Clone, Default)]
pub struct FetchMarketsParams {
    pub market_type: Option<MarketType>,
    pub group_by: Option<GroupBy>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
