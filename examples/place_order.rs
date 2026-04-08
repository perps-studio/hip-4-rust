use hip4::{
    HIP4Adapter, OrderType, PredictionCancelParams, PredictionOrderParams, Side,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let private_key =
        std::env::var("HYPERLIQUID_PRIVATE_KEY").expect("Set HYPERLIQUID_PRIVATE_KEY env var");
    let wallet_address =
        std::env::var("HYPERLIQUID_WALLET_ADDRESS").expect("Set HYPERLIQUID_WALLET_ADDRESS env var");

    let adapter = HIP4Adapter::builder().testnet(true).build();
    adapter.initialize().await?;

    // Authenticate
    adapter
        .auth()
        .init_auth(&wallet_address, &private_key)
        .await?;
    println!("Auth status: {:?}", adapter.auth().get_auth_status().await);

    // Fetch markets
    let markets = adapter.events().fetch_markets(Default::default()).await?;

    let market = match markets.first() {
        Some(m) => m,
        None => {
            println!("No markets found");
            return Ok(());
        }
    };

    let market_id = market.outcome_id().to_string();
    let outcome = market.sides()[0].coin.clone();
    println!("Trading market {} outcome {}", market_id, outcome);

    // Place a limit buy order
    let result = adapter
        .trading()
        .place_order(PredictionOrderParams {
            market_id: market_id.clone(),
            outcome: outcome.clone(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Some("0.01".to_string()),
            amount: "1000".to_string(),
            time_in_force: None,
            expiration: None,
            mark_px: None,
            builder_address: None,
            builder_fee: None,
        })
        .await;

    println!("Order result: {:?}", result);

    // Cancel if resting
    if result.success {
        if let Some(order_id) = &result.order_id {
            if result.status.as_deref() == Some("resting") {
                println!("Cancelling resting order {}", order_id);
                adapter
                    .trading()
                    .cancel_order(PredictionCancelParams {
                        market_id,
                        order_id: order_id.clone(),
                        outcome: Some(outcome),
                    })
                    .await?;
                println!("Cancelled.");
            }
        }
    }

    adapter.destroy().await;
    Ok(())
}
