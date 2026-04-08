use std::collections::HashMap;

use crate::hl_types::{HLOutcome, HLQuestion};
use crate::market_discovery::parse_description;
use crate::types::hip4_market::*;

const PREDICTION_ASSET_OFFSET: u64 = 100_000_000;

fn build_sides(outcome: &HLOutcome) -> [MarketSide; 2] {
    let side0_name = outcome
        .side_specs
        .first()
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "Side 0".to_string());
    let side1_name = outcome
        .side_specs
        .get(1)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "Side 1".to_string());

    let coin_num_0 = outcome.outcome * 10;
    let coin_num_1 = outcome.outcome * 10 + 1;

    [
        MarketSide {
            name: side0_name,
            coin_num: coin_num_0,
            coin: format!("#{coin_num_0}"),
            asset: PREDICTION_ASSET_OFFSET + coin_num_0,
        },
        MarketSide {
            name: side1_name,
            coin_num: coin_num_1,
            coin: format!("#{coin_num_1}"),
            asset: PREDICTION_ASSET_OFFSET + coin_num_1,
        },
    ]
}

struct QuestionIndex {
    by_outcome: HashMap<u64, (HLQuestion, bool)>,
}

fn build_question_index(questions: &[HLQuestion]) -> QuestionIndex {
    let mut by_outcome = HashMap::new();
    for q in questions {
        for &id in &q.named_outcomes {
            by_outcome.insert(id, (q.clone(), false));
        }
        by_outcome.insert(q.fallback_outcome, (q.clone(), true));
    }
    QuestionIndex { by_outcome }
}

/// Classify a single HLOutcome into a typed HIP4Market.
///
/// Detection priority:
///   1. Description parses as class:priceBinary -> DefaultBinary
///   2. Outcome is in a question -> MultiOutcome
///   3. Everything else -> LabelledBinary
pub fn classify_outcome(outcome: &HLOutcome, questions: &[HLQuestion]) -> HIP4Market {
    let sides = build_sides(outcome);
    let index = build_question_index(questions);

    if let Some(parsed) = parse_description(&outcome.description) {
        return HIP4Market::DefaultBinary(DefaultBinaryMarket {
            base: BaseMarket {
                market_type: MarketType::DefaultBinary,
                outcome_id: outcome.outcome,
                name: format!(
                    "{} > ${} ({})",
                    parsed.underlying, parsed.target_price, parsed.period
                ),
                description: format!(
                    "Will {} be above ${} by expiry?",
                    parsed.underlying, parsed.target_price
                ),
                sides,
                raw: outcome.clone(),
            },
            underlying: parsed.underlying,
            target_price: parsed.target_price,
            expiry: parsed.expiry,
            period: parsed.period,
        });
    }

    if let Some((question, is_fallback)) = index.by_outcome.get(&outcome.outcome) {
        return HIP4Market::MultiOutcome(MultiOutcomeMarket {
            base: BaseMarket {
                market_type: MarketType::MultiOutcome,
                outcome_id: outcome.outcome,
                name: outcome.name.clone(),
                description: outcome.description.clone(),
                sides,
                raw: outcome.clone(),
            },
            question_id: question.question,
            question_name: question.name.clone(),
            question_description: question.description.clone(),
            is_fallback: *is_fallback,
            raw_question: question.clone(),
        });
    }

    HIP4Market::LabelledBinary(LabelledBinaryMarket {
        base: BaseMarket {
            market_type: MarketType::LabelledBinary,
            outcome_id: outcome.outcome,
            name: outcome.name.clone(),
            description: outcome.description.clone(),
            sides,
            raw: outcome.clone(),
        },
    })
}

/// Classify all outcomes in one pass. Builds the question index once.
pub fn classify_all_outcomes(outcomes: &[HLOutcome], questions: &[HLQuestion]) -> Vec<HIP4Market> {
    let index = build_question_index(questions);

    outcomes
        .iter()
        .map(|outcome| {
            let sides = build_sides(outcome);

            if let Some(parsed) = parse_description(&outcome.description) {
                return HIP4Market::DefaultBinary(DefaultBinaryMarket {
                    base: BaseMarket {
                        market_type: MarketType::DefaultBinary,
                        outcome_id: outcome.outcome,
                        name: format!(
                            "{} > ${} ({})",
                            parsed.underlying, parsed.target_price, parsed.period
                        ),
                        description: format!(
                            "Will {} be above ${} by expiry?",
                            parsed.underlying, parsed.target_price
                        ),
                        sides,
                        raw: outcome.clone(),
                    },
                    underlying: parsed.underlying,
                    target_price: parsed.target_price,
                    expiry: parsed.expiry,
                    period: parsed.period,
                });
            }

            if let Some((question, is_fallback)) = index.by_outcome.get(&outcome.outcome) {
                return HIP4Market::MultiOutcome(MultiOutcomeMarket {
                    base: BaseMarket {
                        market_type: MarketType::MultiOutcome,
                        outcome_id: outcome.outcome,
                        name: outcome.name.clone(),
                        description: outcome.description.clone(),
                        sides,
                        raw: outcome.clone(),
                    },
                    question_id: question.question,
                    question_name: question.name.clone(),
                    question_description: question.description.clone(),
                    is_fallback: *is_fallback,
                    raw_question: question.clone(),
                });
            }

            HIP4Market::LabelledBinary(LabelledBinaryMarket {
                base: BaseMarket {
                    market_type: MarketType::LabelledBinary,
                    outcome_id: outcome.outcome,
                    name: outcome.name.clone(),
                    description: outcome.description.clone(),
                    sides,
                    raw: outcome.clone(),
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hl_types::HLSideSpec;

    #[test]
    fn test_classify_price_binary() {
        let outcome = HLOutcome {
            outcome: 100,
            name: "Recurring".to_string(),
            description: "class:priceBinary|underlying:BTC|expiry:20260310-0300|targetPrice:66200|period:15m".to_string(),
            side_specs: vec![
                HLSideSpec { name: "Yes".to_string() },
                HLSideSpec { name: "No".to_string() },
            ],
        };

        let market = classify_outcome(&outcome, &[]);
        assert_eq!(market.market_type(), MarketType::DefaultBinary);
        assert_eq!(market.outcome_id(), 100);
    }

    #[test]
    fn test_classify_labelled() {
        let outcome = HLOutcome {
            outcome: 200,
            name: "Custom Market".to_string(),
            description: "Some custom market".to_string(),
            side_specs: vec![
                HLSideSpec { name: "Hypurr".to_string() },
                HLSideSpec { name: "Usain Bolt".to_string() },
            ],
        };

        let market = classify_outcome(&outcome, &[]);
        assert_eq!(market.market_type(), MarketType::LabelledBinary);
    }

    #[test]
    fn test_classify_multi_outcome() {
        let outcome = HLOutcome {
            outcome: 300,
            name: "Option A".to_string(),
            description: "Option A for question".to_string(),
            side_specs: vec![
                HLSideSpec { name: "Yes".to_string() },
                HLSideSpec { name: "No".to_string() },
            ],
        };

        let question = HLQuestion {
            question: 1,
            name: "Who will win?".to_string(),
            description: "Multi-outcome question".to_string(),
            fallback_outcome: 301,
            named_outcomes: vec![300],
            settled_named_outcomes: vec![],
        };

        let market = classify_outcome(&outcome, &[question]);
        assert_eq!(market.market_type(), MarketType::MultiOutcome);
    }
}
