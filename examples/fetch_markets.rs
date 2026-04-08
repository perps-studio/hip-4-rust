use hip4::HIP4Adapter;
use hip4::hip4_market::HIP4Market;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let adapter = HIP4Adapter::builder().testnet(true).build();
    adapter.initialize().await?;

    // Fetch all events
    println!("=== Events ===");
    let events = adapter
        .events()
        .fetch_events(None, true, Some(10), None, None)
        .await?;

    for event in &events {
        let status = format!("{:?}", event.status);
        println!("[{}] {} ({})", event.id, event.title, status);
        for market in &event.markets {
            println!("  Market {}: {}", market.id, market.question);
            for outcome in &market.outcomes {
                println!("    {} ({}): ${}", outcome.name, outcome.token_id, outcome.price);
            }
        }
    }

    // Fetch typed HIP-4 markets
    println!("\n=== Typed Markets ===");
    let markets = adapter
        .events()
        .fetch_markets(Default::default())
        .await?;

    for market in &markets {
        match market {
            HIP4Market::DefaultBinary(m) => {
                println!(
                    "[DefaultBinary] {} > ${} ({}) - expires {:?}",
                    m.underlying, m.target_price, m.period, m.expiry
                );
            }
            HIP4Market::LabelledBinary(m) => {
                println!(
                    "[LabelledBinary] {} - {}/{}",
                    m.base.name, m.base.sides[0].name, m.base.sides[1].name
                );
            }
            HIP4Market::MultiOutcome(m) => {
                println!(
                    "[MultiOutcome] {} (question: {})",
                    m.base.name, m.question_name
                );
            }
        }
    }

    // Fetch price for first market
    if let Some(first) = markets.first() {
        let market_id = first.outcome_id().to_string();
        let price = adapter.market_data().fetch_price(&market_id).await?;
        println!("\n=== Price for {} ===", market_id);
        for outcome in &price.outcomes {
            println!("  {}: ${}", outcome.name, outcome.price);
        }
    }

    adapter.destroy().await;
    Ok(())
}
