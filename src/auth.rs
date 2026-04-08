use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::crypto::signer::Signer;
use crate::types::account::{AuthStatus, PredictionAuthState};

/// Atomic nonce counter. Increments on each call to next().
pub struct NonceCounter {
    val: AtomicU64,
}

impl Default for NonceCounter {
    fn default() -> Self {
        Self {
            val: AtomicU64::new(0),
        }
    }
}

impl NonceCounter {
    pub fn next(&self) -> u64 {
        let ts = chrono::Utc::now().timestamp_millis() as u64;
        self.val
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |prev| {
                Some(prev.max(ts) + 1)
            })
            .unwrap_or(ts)
    }
}

/// Internal auth state holding the signer.
pub struct AuthInner {
    pub state: PredictionAuthState,
    pub signer: Option<Signer>,
    pub nonce: NonceCounter,
    pub testnet: bool,
}

/// HIP4 auth manager. Stores wallet address and signer.
pub struct HIP4Auth {
    inner: Arc<RwLock<AuthInner>>,
}

impl HIP4Auth {
    pub fn new(testnet: bool) -> Self {
        Self {
            inner: Arc::new(RwLock::new(AuthInner {
                state: PredictionAuthState {
                    status: AuthStatus::Disconnected,
                    address: None,
                },
                signer: None,
                nonce: NonceCounter::default(),
                testnet,
            })),
        }
    }

    /// Initialize auth with a wallet address and private key.
    pub async fn init_auth(
        &self,
        wallet_address: &str,
        private_key: &str,
    ) -> crate::error::Result<PredictionAuthState> {
        let signer = Signer::from_hex(private_key)
            .map_err(|e| crate::error::HIP4Error::Auth(format!("Invalid private key: {e}")))?;

        let mut inner = self.inner.write().await;

        inner.signer = Some(signer);
        inner.state = PredictionAuthState {
            status: AuthStatus::Ready,
            address: Some(wallet_address.to_string()),
        };

        Ok(inner.state.clone())
    }

    pub async fn get_auth_status(&self) -> PredictionAuthState {
        self.inner.read().await.state.clone()
    }

    pub async fn clear_auth(&self) {
        let mut inner = self.inner.write().await;
        inner.signer = None;
        inner.state = PredictionAuthState {
            status: AuthStatus::Disconnected,
            address: None,
        };
    }

    /// Access the inner auth state for trading/wallet operations.
    pub fn inner(&self) -> &Arc<RwLock<AuthInner>> {
        &self.inner
    }
}
