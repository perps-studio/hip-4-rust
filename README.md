<h1 align="center">hip4</h1>

<p align="center">
  <a href="https://crates.io/crates/hip4"><img src="https://img.shields.io/crates/v/hip4.svg" alt="crates.io version" /></a>
  <img src="https://img.shields.io/badge/runtime-tokio-blue" alt="tokio" />
  <img src="https://img.shields.io/crates/l/hip4" alt="license" />
</p>

---

Rust SDK for Hyperliquid HIP-4 prediction markets. Built on `hypersdk` for signing and `tokio` for async.

## Built for developers

The SDK is structured around a single adapter with typed sub-modules for each domain - events, market data, account state, trading, wallet, and auth. Everything returns typed responses, WebSocket subscriptions return drop-to-unsubscribe handles, and all signing (L1 agent + EIP-712) is handled internally via `hypersdk` and `alloy`.

```toml
[dependencies]
hip4 = "0.1"
tokio = { version = "1", features = ["full"] }
```

```rust
use hip4::HIP4Adapter;

let hip4 = HIP4Adapter::builder().testnet(true).build();
hip4.initialize().await?;
```

## Examples

- [`fetch_markets.rs`](examples/fetch_markets.rs) - Fetch all markets grouped by type with live prices
- [`place_order.rs`](examples/place_order.rs) - Auth + limit order placement + cancellation

## API

### `hip4.events()`

| Method | Description |
|--------|-------------|
| `fetch_events(category?, active, limit?, offset?, query?)` | List events. Filters: category, active, limit, offset, query |
| `fetch_event(event_id)` | Single event by ID |
| `fetch_categories()` | Available categories |
| `fetch_markets(params)` | Typed HIP-4 markets with optional type filtering and pagination |

### `hip4.market_data()`

| Method | Description |
|--------|-------------|
| `fetch_order_book(market_id, side_index)` | L2 snapshot |
| `fetch_price(market_id)` | Both sides, 5s cache |
| `fetch_trades(market_id, limit, side_index)` | Recent trades |
| `fetch_candles(market_id, interval, start?, end?)` | OHLCV candles |
| `subscribe_order_book(market_id, cb)` | Real-time L2 book |
| `subscribe_price(market_id, cb)` | Real-time prices |
| `subscribe_trades(market_id, cb)` | Real-time trades |

### `hip4.account()`

| Method | Description |
|--------|-------------|
| `fetch_positions(address)` | Outcome positions with resolved side names |
| `fetch_activity(address)` | Fills, last 30 days |
| `fetch_balance(address)` | Spot balances |
| `fetch_open_orders(address)` | Resting orders |

### `hip4.trading()`

| Method | Description |
|--------|-------------|
| `place_order(params)` | Place market or limit order. Returns `PredictionOrderResult { success, order_id?, error? }` |
| `cancel_order(params)` | Cancel a resting order |

### `hip4.wallet()`

| Method | Signing | Description |
|--------|---------|-------------|
| `buy_usdh(amount)` | L1 agent | Buy USDH on spot |
| `sell_usdh(amount)` | L1 agent | Sell USDH on spot |
| `transfer_to_spot(amount)` | EIP-712 | Perp -> Spot |
| `transfer_to_perps(amount)` | EIP-712 | Spot -> Perp |
| `withdraw(destination, amount)` | EIP-712 | Withdraw to external address |
| `usd_send(destination, amount)` | EIP-712 | Send to another HL address |

### `hip4.auth()`

| Method | Description |
|--------|-------------|
| `init_auth(wallet_address, private_key)` | Parse private key, initialize `hypersdk` client |
| `get_auth_status()` | `Disconnected \| PendingApproval \| Ready` |
| `clear_auth()` | Reset auth state |

## Market Types

`fetch_markets()` classifies every HIP-4 outcome into one of three types:

| Type | Description | Parsed Fields |
|------|-------------|---------------|
| `DefaultBinary` | Recurring price markets (BTC > $67250 1d) | `underlying`, `target_price`, `expiry`, `period` |
| `LabelledBinary` | Standalone binary with custom sides (Hypurr vs Usain Bolt) | Custom side labels |
| `MultiOutcome` | Grouped under a question, with fallback | `question_id`, `question_name`, `is_fallback` |

```rust
use hip4::hip4_market::{FetchMarketsParams, MarketType, HIP4Market};

// Filter by type
let binaries = hip4.events().fetch_markets(FetchMarketsParams {
    market_type: Some(MarketType::DefaultBinary),
    ..Default::default()
}).await?;

// Iterate with pattern matching
for market in &binaries {
    match market {
        HIP4Market::DefaultBinary(m) => {
            println!("{} > ${} ({})", m.underlying, m.target_price, m.period);
        }
        _ => {}
    }
}
```

## Signing

Both Hyperliquid signing flows are handled internally.

**L1 agent signing** (orders, cancels, USDH spot):

Delegated to `hypersdk` which handles:
1. Sort action keys in canonical order
2. MessagePack encode
3. Append nonce as BE u64
4. Append vault marker byte
5. Keccak-256 hash -> `connectionId`
6. EIP-712 sign with `Agent` type on chainId `1337`

**User-signed EIP-712** (transfers, withdrawals, sends):

- Domain: `HyperliquidSignTransaction`, `signatureChainId: 0x66eee`
- Implemented via `alloy` primitives (keccak256, secp256k1 signing)
- Requires user wallet private key, agent keys rejected

## Acknowledgements

L1 signing powered by [`hypersdk`](https://crates.io/crates/hypersdk). EIP-712 signing via [`alloy`](https://crates.io/crates/alloy).

## License

BUSL-1.1
# hip-4-rust
