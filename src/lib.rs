//! # hip4
//!
//! Rust SDK for Hyperliquid HIP-4 prediction markets.
//!
//! Provides typed access to all HIP-4 operations:
//! - Market discovery and classification
//! - Real-time order books, prices, and trades (REST + WebSocket)
//! - Order placement and cancellation (from-scratch EIP-712 signing)
//! - Account positions, activity, and balances
//! - Wallet operations (USDH buy/sell, transfers, withdrawals)
//!
//! ## Quick Start
//!
//! ```no_run
//! use hip4::HIP4Adapter;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let adapter = HIP4Adapter::builder().testnet(true).build();
//!     adapter.initialize().await?;
//!
//!     let events = adapter.events().fetch_events(None, true, Some(10), None, None).await?;
//!     for event in &events {
//!         println!("{}: {}", event.id, event.title);
//!     }
//!     Ok(())
//! }
//! ```

pub mod account;
pub mod agent_wallet;
pub mod auth;
pub mod client;
pub mod crypto;
pub mod error;
pub mod events;
pub mod hl_types;
pub mod market_classification;
pub mod market_data;
pub mod market_discovery;
pub mod pricing;
pub mod signing;
pub mod trading;
pub mod types;
pub mod wallet;
pub mod websocket;

use std::sync::Arc;

use auth::HIP4Auth;
use client::HIP4Client;
use events::{new_side_name_resolver, HIP4Events};
use market_data::HIP4MarketData;
use trading::HIP4Trading;
use websocket::WsPool;

// Re-exports
pub use account::HIP4Account;
pub use auth::HIP4Auth as Auth;
pub use error::{HIP4Error, Result};
pub use events::HIP4Events as Events;
pub use market_data::HIP4MarketData as MarketData;
pub use trading::HIP4Trading as Trading;
pub use types::*;
pub use wallet::HIP4Wallet;

/// Top-level adapter composing all HIP-4 sub-adapters.
pub struct HIP4Adapter {
    pub id: &'static str,
    pub name: String,
    #[allow(dead_code)]
    client: Arc<HIP4Client>,
    auth: HIP4Auth,
    events: HIP4Events,
    market_data: HIP4MarketData,
    account: HIP4Account,
    trading: HIP4Trading,
    wallet: HIP4Wallet,
    ws_pool: Arc<WsPool>,
}

impl HIP4Adapter {
    pub fn builder() -> HIP4AdapterBuilder {
        HIP4AdapterBuilder::default()
    }

    /// Initialize the adapter: loads side names from outcomeMeta.
    /// Must be called before querying market data or account positions.
    pub async fn initialize(&self) -> error::Result<()> {
        self.events.ensure_side_names().await
    }

    /// Tear down WebSocket connections and clear auth.
    pub async fn destroy(&self) {
        self.ws_pool.destroy().await;
        self.auth.clear_auth().await;
    }

    // -- Sub-adapter accessors --

    pub fn auth(&self) -> &HIP4Auth {
        &self.auth
    }

    pub fn events(&self) -> &HIP4Events {
        &self.events
    }

    pub fn market_data(&self) -> &HIP4MarketData {
        &self.market_data
    }

    pub fn account(&self) -> &HIP4Account {
        &self.account
    }

    pub fn trading(&self) -> &HIP4Trading {
        &self.trading
    }

    pub fn wallet(&self) -> &HIP4Wallet {
        &self.wallet
    }
}

/// Builder for HIP4Adapter.
pub struct HIP4AdapterBuilder {
    testnet: bool,
    info_url: Option<String>,
    exchange_url: Option<String>,
}

impl Default for HIP4AdapterBuilder {
    fn default() -> Self {
        Self {
            testnet: true,
            info_url: None,
            exchange_url: None,
        }
    }
}

impl HIP4AdapterBuilder {
    pub fn testnet(mut self, testnet: bool) -> Self {
        self.testnet = testnet;
        self
    }

    pub fn info_url(mut self, url: impl Into<String>) -> Self {
        self.info_url = Some(url.into());
        self
    }

    pub fn exchange_url(mut self, url: impl Into<String>) -> Self {
        self.exchange_url = Some(url.into());
        self
    }

    pub fn build(self) -> HIP4Adapter {
        let client = Arc::new(HIP4Client::new(
            self.testnet,
            self.info_url.as_deref(),
            self.exchange_url.as_deref(),
        ));

        let auth = HIP4Auth::new(self.testnet);
        let side_names = new_side_name_resolver();
        let events = HIP4Events::new(client.clone(), side_names.clone());

        let ws_pool = Arc::new(WsPool::new(&client.ws_url));
        let market_data = HIP4MarketData::new(client.clone(), ws_pool.clone(), side_names.clone());
        let account = HIP4Account::new(client.clone(), side_names);

        let auth_inner = auth.inner().clone();
        let trading = HIP4Trading::new(client.clone(), auth_inner.clone());
        let wallet = HIP4Wallet::new(client.clone(), auth_inner);

        let name = if self.testnet {
            "Hyperliquid HIP-4 (Testnet)".to_string()
        } else {
            "Hyperliquid HIP-4".to_string()
        };

        HIP4Adapter {
            id: "hyperliquid",
            name,
            client,
            auth,
            events,
            market_data,
            account,
            trading,
            wallet,
            ws_pool,
        }
    }
}
