/// Signing module - thin wrappers around crypto::eip712 for the public API.
///
/// All signing logic lives in crypto/eip712.rs. This module re-exports
/// what the trading and wallet adapters need.
pub use crate::crypto::eip712::{
    create_l1_action_hash, sign_l1_action, sign_user_signed_action, sort_cancel_action,
    sort_order_action,
};
