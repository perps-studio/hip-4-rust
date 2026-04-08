use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::client::{side_coin, HIP4Client};
use crate::error::Result;
use crate::hl_types::{HLOutcome, HLOutcomeMeta, HLQuestion};
use crate::market_classification::classify_all_outcomes;
use crate::types::event::*;
use crate::types::hip4_market::*;

const CACHE_TTL_MS: i64 = 30_000;

fn is_recurring(outcome: &HLOutcome) -> bool {
    outcome.name == "Recurring"
}

fn parse_recurring_fields(desc: &str) -> Option<HashMap<String, String>> {
    if !desc.contains('|') {
        return None;
    }
    let mut result = HashMap::new();
    for segment in desc.split('|') {
        if let Some(idx) = segment.find(':') {
            let key = &segment[..idx];
            let value = &segment[idx + 1..];
            result.insert(key.to_string(), value.to_string());
        }
    }
    if result.is_empty() { None } else { Some(result) }
}

fn recurring_title(outcome: &HLOutcome) -> String {
    let parsed = match parse_recurring_fields(&outcome.description) {
        Some(p) => p,
        None => return format!("Outcome #{}", outcome.outcome),
    };
    let underlying = parsed.get("underlying").map(|s| s.as_str()).unwrap_or("???");
    let target = parsed.get("targetPrice").map(|s| s.as_str()).unwrap_or("???");
    let period = parsed.get("period").map(|s| s.as_str()).unwrap_or("");

    if parsed.get("class").map(|s| s.as_str()) == Some("priceBinary") {
        format!("{underlying} > ${target} ({period})")
    } else {
        let class = parsed.get("class").map(|s| s.as_str()).unwrap_or("outcome");
        format!("{underlying} {class} ({period})")
    }
}

fn recurring_description(outcome: &HLOutcome) -> String {
    let parsed = match parse_recurring_fields(&outcome.description) {
        Some(p) => p,
        None => return outcome.description.clone(),
    };
    let underlying = parsed.get("underlying").map(|s| s.as_str()).unwrap_or("asset");
    let target = parsed.get("targetPrice").map(|s| s.as_str()).unwrap_or("?");
    let expiry = parsed.get("expiry").map(|s| s.as_str()).unwrap_or("unknown");
    format!("Will {underlying} be above ${target} by {expiry}?")
}

fn map_outcome_to_market(outcome: &HLOutcome, event_id: &str) -> PredictionMarket {
    let outcomes: Vec<PredictionOutcome> = outcome
        .side_specs
        .iter()
        .enumerate()
        .map(|(side_index, spec)| PredictionOutcome {
            name: spec.name.clone(),
            token_id: side_coin(outcome.outcome, side_index as u64),
            price: "0".to_string(),
        })
        .collect();

    PredictionMarket {
        id: outcome.outcome.to_string(),
        event_id: event_id.to_string(),
        question: if is_recurring(outcome) {
            recurring_description(outcome)
        } else {
            outcome.name.clone()
        },
        outcomes,
        volume: "0".to_string(),
        liquidity: "0".to_string(),
        is_neg_risk: None,
    }
}

fn map_question_to_event(
    question: &HLQuestion,
    outcome_map: &HashMap<u64, HLOutcome>,
) -> PredictionEvent {
    let event_id = format!("q{}", question.question);
    let all_outcome_ids: Vec<u64> = question
        .named_outcomes
        .iter()
        .chain(std::iter::once(&question.fallback_outcome))
        .filter(|id| outcome_map.contains_key(id))
        .copied()
        .collect();

    let markets: Vec<PredictionMarket> = all_outcome_ids
        .iter()
        .filter_map(|id| outcome_map.get(id))
        .map(|o| map_outcome_to_market(o, &event_id))
        .collect();

    let settled: HashSet<u64> = question.settled_named_outcomes.iter().copied().collect();
    let has_unsettled = question.named_outcomes.iter().any(|id| !settled.contains(id));

    PredictionEvent {
        id: event_id,
        title: question.name.clone(),
        description: question.description.clone(),
        category: "custom".to_string(),
        markets,
        total_volume: "0".to_string(),
        end_date: String::new(),
        status: if has_unsettled {
            PredictionEventStatus::Active
        } else {
            PredictionEventStatus::Resolved
        },
        image_url: None,
        resolution_source: None,
    }
}

fn map_standalone_outcome_to_event(outcome: &HLOutcome) -> PredictionEvent {
    let event_id = format!("o{}", outcome.outcome);
    let recurring = is_recurring(outcome);

    PredictionEvent {
        id: event_id.clone(),
        title: if recurring {
            recurring_title(outcome)
        } else {
            outcome.name.clone()
        },
        description: if recurring {
            recurring_description(outcome)
        } else {
            outcome.description.clone()
        },
        category: if recurring { "recurring" } else { "custom" }.to_string(),
        markets: vec![map_outcome_to_market(outcome, &event_id)],
        total_volume: "0".to_string(),
        end_date: if recurring {
            parse_recurring_fields(&outcome.description)
                .and_then(|f| f.get("expiry").cloned())
                .unwrap_or_default()
        } else {
            String::new()
        },
        status: PredictionEventStatus::Active,
        image_url: None,
        resolution_source: None,
    }
}

fn build_events_from_meta(meta: &HLOutcomeMeta) -> Vec<PredictionEvent> {
    let outcome_map: HashMap<u64, HLOutcome> = meta
        .outcomes
        .iter()
        .map(|o| (o.outcome, o.clone()))
        .collect();

    let mut claimed: HashSet<u64> = HashSet::new();
    let mut events = Vec::new();

    for q in &meta.questions {
        for id in &q.named_outcomes {
            claimed.insert(*id);
        }
        claimed.insert(q.fallback_outcome);
        events.push(map_question_to_event(q, &outcome_map));
    }

    for o in &meta.outcomes {
        if !claimed.contains(&o.outcome) {
            events.push(map_standalone_outcome_to_event(o));
        }
    }

    events
}

/// Side name resolver: maps outcome ID to (side0Name, side1Name).
pub type SideNameResolver = Arc<RwLock<HashMap<u64, (String, String)>>>;

/// Create a new empty side name resolver.
pub fn new_side_name_resolver() -> SideNameResolver {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Resolve side names for an outcome ID.
pub async fn resolve_side_names(
    resolver: &SideNameResolver,
    outcome_id: u64,
) -> Option<(String, String)> {
    resolver.read().await.get(&outcome_id).cloned()
}

/// HIP4 event adapter with caching and side name resolution.
pub struct HIP4Events {
    client: Arc<HIP4Client>,
    side_names: SideNameResolver,
    cache: RwLock<Option<(Vec<PredictionEvent>, i64)>>,
    market_cache: RwLock<Option<(Vec<HIP4Market>, i64)>>,
}

impl HIP4Events {
    pub fn new(client: Arc<HIP4Client>, side_names: SideNameResolver) -> Self {
        Self {
            client,
            side_names,
            cache: RwLock::new(None),
            market_cache: RwLock::new(None),
        }
    }

    pub fn side_name_resolver(&self) -> &SideNameResolver {
        &self.side_names
    }

    /// Ensure side names are loaded from outcomeMeta.
    pub async fn ensure_side_names(&self) -> Result<()> {
        {
            let names = self.side_names.read().await;
            if !names.is_empty() {
                return Ok(());
            }
        }
        let meta = self.client.fetch_outcome_meta().await?;
        self.populate_side_names(&meta).await;
        Ok(())
    }

    async fn populate_side_names(&self, meta: &HLOutcomeMeta) {
        let mut names = self.side_names.write().await;
        if !names.is_empty() {
            return;
        }
        for o in &meta.outcomes {
            if o.side_specs.len() >= 2 {
                names.insert(
                    o.outcome,
                    (o.side_specs[0].name.clone(), o.side_specs[1].name.clone()),
                );
            }
        }
    }

    pub async fn fetch_events(
        &self,
        category: Option<&str>,
        active: bool,
        limit: Option<usize>,
        offset: Option<usize>,
        query: Option<&str>,
    ) -> Result<Vec<PredictionEvent>> {
        let mut events = self.load_events().await?;

        if let Some(cat) = category {
            if cat != "all" {
                events.retain(|e| e.category == cat);
            }
        }

        if active {
            events.retain(|e| e.status == PredictionEventStatus::Active);
        }

        if let Some(q) = query {
            let q_lower = q.to_lowercase();
            events.retain(|e| {
                e.title.to_lowercase().contains(&q_lower)
                    || e.description.to_lowercase().contains(&q_lower)
            });
        }

        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(50);
        Ok(events.into_iter().skip(offset).take(limit).collect())
    }

    pub async fn fetch_event(&self, event_id: &str) -> Result<PredictionEvent> {
        let events = self.load_events().await?;
        events
            .into_iter()
            .find(|e| e.id == event_id)
            .ok_or_else(|| crate::error::HIP4Error::Validation(format!("Event not found: {event_id}")))
    }

    pub async fn fetch_categories(&self) -> Vec<PredictionCategory> {
        vec![
            PredictionCategory { id: "custom".into(), name: "Custom".into(), slug: "custom".into() },
            PredictionCategory { id: "recurring".into(), name: "Recurring".into(), slug: "recurring".into() },
        ]
    }

    /// Fetch and classify all HIP-4 markets.
    pub async fn fetch_markets(&self, params: FetchMarketsParams) -> Result<Vec<HIP4Market>> {
        let all_markets = self.load_markets().await?;

        let mut filtered: Vec<HIP4Market> = if let Some(mt) = params.market_type {
            all_markets.into_iter().filter(|m| m.market_type() == mt).collect()
        } else {
            all_markets
        };

        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(filtered.len());
        filtered = filtered.into_iter().skip(offset).take(limit).collect();

        Ok(filtered)
    }

    async fn load_markets(&self) -> Result<Vec<HIP4Market>> {
        let now = chrono::Utc::now().timestamp_millis();
        {
            let cache = self.market_cache.read().await;
            if let Some((ref markets, ts)) = *cache {
                if now - ts < CACHE_TTL_MS {
                    return Ok(markets.clone());
                }
            }
        }

        let meta = self.client.fetch_outcome_meta().await?;
        self.populate_side_names(&meta).await;
        let markets = classify_all_outcomes(&meta.outcomes, &meta.questions);

        {
            let mut cache = self.market_cache.write().await;
            *cache = Some((markets.clone(), now));
        }

        Ok(markets)
    }

    async fn load_events(&self) -> Result<Vec<PredictionEvent>> {
        let now = chrono::Utc::now().timestamp_millis();
        {
            let cache = self.cache.read().await;
            if let Some((ref events, ts)) = *cache {
                if now - ts < CACHE_TTL_MS {
                    return Ok(events.clone());
                }
            }
        }

        let (meta, mids) = tokio::try_join!(
            self.client.fetch_outcome_meta(),
            self.client.fetch_all_mids(),
        )?;

        self.populate_side_names(&meta).await;

        let mut events = build_events_from_meta(&meta);

        // Populate prices from allMids
        for event in &mut events {
            for market in &mut event.markets {
                for outcome in &mut market.outcomes {
                    if let Some(mid) = mids.get(&outcome.token_id) {
                        outcome.price = mid.clone();
                    }
                }
            }
        }

        {
            let mut cache = self.cache.write().await;
            *cache = Some((events.clone(), now));
        }

        Ok(events)
    }
}
