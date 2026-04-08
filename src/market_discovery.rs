use chrono::{DateTime, TimeZone, Utc};
use std::collections::HashMap;

use crate::hl_types::HLOutcomeMeta;

pub type MarketTypeStr = &'static str;

#[derive(Debug, Clone)]
pub struct ParsedDescription {
    pub class: String,
    pub underlying: String,
    pub expiry: DateTime<Utc>,
    pub target_price: f64,
    pub period: String,
}

#[derive(Debug, Clone)]
pub struct PriceBinaryMarket {
    pub outcome_id: u64,
    pub underlying: String,
    pub target_price: f64,
    pub expiry: DateTime<Utc>,
    pub period: String,
    pub yes_coin_num: u64,
    pub no_coin_num: u64,
    pub yes_coin: String,
    pub no_coin: String,
    pub yes_asset: u64,
    pub no_asset: u64,
}

const PREDICTION_ASSET_OFFSET: u64 = 100_000_000;

/// Parse a pipe-delimited outcome description string.
///
/// Format: "class:priceBinary|underlying:BTC|expiry:20260310-0300|targetPrice:66200|period:15m"
pub fn parse_description(desc: &str) -> Option<ParsedDescription> {
    if desc.is_empty() || !desc.contains('|') {
        return None;
    }

    let mut fields = HashMap::new();
    for pair in desc.split('|') {
        if let Some(idx) = pair.find(':') {
            let key = &pair[..idx];
            let value = &pair[idx + 1..];
            fields.insert(key.to_string(), value.to_string());
        }
    }

    if fields.get("class").map(|s| s.as_str()) != Some("priceBinary") {
        return None;
    }

    let underlying = fields.get("underlying")?.clone();
    let expiry_str = fields.get("expiry")?;
    let target_price_str = fields.get("targetPrice")?;
    let period = fields.get("period")?.clone();

    let expiry = parse_expiry(expiry_str)?;
    let target_price: f64 = target_price_str.parse().ok()?;

    Some(ParsedDescription {
        class: "priceBinary".to_string(),
        underlying,
        expiry,
        target_price,
        period,
    })
}

fn parse_expiry(s: &str) -> Option<DateTime<Utc>> {
    if s.len() < 13 {
        return None;
    }
    let year: i32 = s[0..4].parse().ok()?;
    let month: u32 = s[4..6].parse().ok()?;
    let day: u32 = s[6..8].parse().ok()?;
    // s[8] is '-'
    let hour: u32 = s[9..11].parse().ok()?;
    let min: u32 = s[11..13].parse().ok()?;

    Utc.with_ymd_and_hms(year, month, day, hour, min, 0).single()
}

/// Discover all active priceBinary markets from outcomeMeta + allMids.
pub fn discover_price_binary_markets(
    meta: &HLOutcomeMeta,
    mids: &HashMap<String, String>,
) -> Vec<PriceBinaryMarket> {
    let now = Utc::now();
    let mut markets = Vec::new();

    for outcome in &meta.outcomes {
        let parsed = match parse_description(&outcome.description) {
            Some(p) => p,
            None => continue,
        };

        if !mids.contains_key(&parsed.underlying) {
            continue;
        }
        if parsed.expiry <= now {
            continue;
        }

        let yes_coin_num = outcome.outcome * 10;
        let no_coin_num = outcome.outcome * 10 + 1;

        markets.push(PriceBinaryMarket {
            outcome_id: outcome.outcome,
            underlying: parsed.underlying,
            target_price: parsed.target_price,
            expiry: parsed.expiry,
            period: parsed.period,
            yes_coin_num,
            no_coin_num,
            yes_coin: format!("#{}", yes_coin_num),
            no_coin: format!("#{}", no_coin_num),
            yes_asset: PREDICTION_ASSET_OFFSET + yes_coin_num,
            no_asset: PREDICTION_ASSET_OFFSET + no_coin_num,
        });
    }

    markets
}

/// Minutes until market expires. Negative means already expired.
pub fn time_to_expiry(market: &PriceBinaryMarket) -> f64 {
    let diff = market.expiry - Utc::now();
    diff.num_seconds() as f64 / 60.0
}

/// Parse a period string to minutes.
pub fn period_minutes(period: &str) -> u64 {
    let len = period.len();
    if len < 2 {
        return 15;
    }
    let suffix = &period[len - 1..];
    let value: u64 = period[..len - 1].parse().unwrap_or(15);
    match suffix {
        "m" => value,
        "h" => value * 60,
        "d" => value * 1440,
        _ => 15,
    }
}

/// Human-readable label: "BTC-1d", "HYPE-15m"
pub fn format_market_label(market: &PriceBinaryMarket) -> String {
    format!("{}-{}", market.underlying, market.period)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_description_valid() {
        let desc = "class:priceBinary|underlying:BTC|expiry:20260310-0300|targetPrice:66200|period:15m";
        let parsed = parse_description(desc).unwrap();
        assert_eq!(parsed.class, "priceBinary");
        assert_eq!(parsed.underlying, "BTC");
        assert!((parsed.target_price - 66200.0).abs() < 0.01);
        assert_eq!(parsed.period, "15m");
    }

    #[test]
    fn test_parse_description_invalid() {
        assert!(parse_description("").is_none());
        assert!(parse_description("not a description").is_none());
        assert!(parse_description("class:other|underlying:BTC").is_none());
    }

    #[test]
    fn test_period_minutes() {
        assert_eq!(period_minutes("1m"), 1);
        assert_eq!(period_minutes("15m"), 15);
        assert_eq!(period_minutes("1h"), 60);
        assert_eq!(period_minutes("4h"), 240);
        assert_eq!(period_minutes("1d"), 1440);
    }
}
