use crate::error::{HIP4Error, Result};
use crate::hl_types::HLSignature;

/// Build EIP-712 typed data for an agent-wallet approval signature.
///
/// Returns the data needed to sign with a wallet (e.g. via alloy).
pub fn get_agent_approval_typed_data(
    agent_address: &str,
    agent_name: &str,
    nonce: u64,
    is_mainnet: bool,
) -> serde_json::Value {
    let chain = if is_mainnet { "Mainnet" } else { "Testnet" };

    serde_json::json!({
        "domain": {
            "name": "HyperliquidSignTransaction",
            "version": "1",
            "chainId": 42161,
            "verifyingContract": "0x0000000000000000000000000000000000000000"
        },
        "types": {
            "HyperliquidTransaction:ApproveAgent": [
                {"name": "hyperliquidChain", "type": "string"},
                {"name": "agentAddress", "type": "address"},
                {"name": "agentName", "type": "string"},
                {"name": "nonce", "type": "uint64"}
            ]
        },
        "primaryType": "HyperliquidTransaction:ApproveAgent",
        "message": {
            "hyperliquidChain": chain,
            "agentAddress": agent_address.to_lowercase(),
            "agentName": agent_name,
            "nonce": nonce
        }
    })
}

/// Submit a signed agent-approval transaction to the Hyperliquid exchange API.
pub async fn submit_agent_approval(
    signature: &str,
    agent_address: &str,
    agent_name: &str,
    nonce: u64,
    is_mainnet: bool,
    exchange_url: Option<&str>,
) -> Result<()> {
    let parsed_sig = HLSignature::from_hex(signature);

    let action = serde_json::json!({
        "type": "approveAgent",
        "signatureChainId": "0xa4b1",
        "hyperliquidChain": if is_mainnet { "Mainnet" } else { "Testnet" },
        "agentAddress": agent_address.to_lowercase(),
        "agentName": agent_name,
        "nonce": nonce,
    });

    let url = exchange_url.unwrap_or(if is_mainnet {
        "https://api.hyperliquid.xyz/exchange"
    } else {
        "https://api.hyperliquid-testnet.xyz/exchange"
    });

    let body = serde_json::json!({
        "action": action,
        "nonce": nonce,
        "signature": parsed_sig,
    });

    let client = reqwest::Client::new();
    let resp = client.post(url).json(&body).send().await.map_err(|e| {
        HIP4Error::Api {
            status: 0,
            message: format!("Network error: {e}"),
        }
    })?;

    let result: serde_json::Value = resp.json().await.map_err(|e| HIP4Error::Api {
        status: 0,
        message: format!("JSON parse error: {e}"),
    })?;

    if result.get("status").and_then(|s| s.as_str()) == Some("ok") {
        Ok(())
    } else {
        let error_msg = result
            .get("response")
            .and_then(|r| r.get("error"))
            .and_then(|e| e.as_str())
            .unwrap_or("Failed to approve agent");
        Err(HIP4Error::Api {
            status: 0,
            message: error_msg.to_string(),
        })
    }
}
