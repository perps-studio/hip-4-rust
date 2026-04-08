/// EIP-712 typed structured data hashing.
///
/// Implements both signing flows used by Hyperliquid:
///   1. L1 Agent signing (orders, cancels, USDH spot)
///   2. User-signed HyperliquidSignTransaction (transfers, withdrawals, sends)
///
/// Direct port from the TypeScript HIP-4 SDK (signing.ts).
use super::keccak::keccak256;
use super::msgpack;
use super::signer::{EthSignature, Signer};

// ---------------------------------------------------------------------------
// L1 Action Hash
// ---------------------------------------------------------------------------

/// Create the Keccak-256 hash of an L1 action for signing.
///
/// Hash input = msgpack(action) + nonce_be64 + vault_marker
/// vault_marker = 0x00 (no vault) | 0x01 + 20-byte address
pub fn create_l1_action_hash(
    action: &serde_json::Value,
    nonce: u64,
    vault_address: Option<&[u8; 20]>,
) -> [u8; 32] {
    let msgpack_bytes = msgpack::encode(action);

    let nonce_bytes = nonce.to_be_bytes();

    let vault_bytes = match vault_address {
        Some(addr) => {
            let mut v = vec![0x01];
            v.extend_from_slice(addr);
            v
        }
        None => vec![0x00],
    };

    let mut combined = Vec::with_capacity(msgpack_bytes.len() + 8 + vault_bytes.len());
    combined.extend_from_slice(&msgpack_bytes);
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&vault_bytes);

    keccak256(&combined)
}

// ---------------------------------------------------------------------------
// L1 Agent EIP-712 Signing
// ---------------------------------------------------------------------------

/// Sign a Hyperliquid L1 action.
///
/// 1. Compute actionHash = keccak256(msgpack(action) + nonce_be64 + vault_marker)
/// 2. EIP-712 sign with Agent type: { source: "a"/"b", connectionId: actionHash }
pub fn sign_l1_action(
    signer: &Signer,
    action: &serde_json::Value,
    nonce: u64,
    is_testnet: bool,
    vault_address: Option<&[u8; 20]>,
) -> Result<EthSignature, String> {
    let action_hash = create_l1_action_hash(action, nonce, vault_address);

    let source = if is_testnet { "b" } else { "a" };

    // EIP-712 domain separator for Agent
    // domain: { name: "Exchange", version: "1", chainId: 1337, verifyingContract: 0x0...0 }
    let domain_separator = {
        let type_hash = keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );
        let name_hash = keccak256(b"Exchange");
        let version_hash = keccak256(b"1");
        let chain_id = u256_be(1337);
        let verifying_contract = [0u8; 32]; // address zero padded to 32 bytes

        keccak256(
            &[
                type_hash.as_slice(),
                name_hash.as_slice(),
                version_hash.as_slice(),
                &chain_id,
                &verifying_contract,
            ]
            .concat(),
        )
    };

    // Struct hash for Agent { source: string, connectionId: bytes32 }
    let struct_hash = {
        let type_hash = keccak256(b"Agent(string source,bytes32 connectionId)");
        let source_hash = keccak256(source.as_bytes());

        // connectionId is bytes32, encoded as-is (left-padded to 32 bytes)
        keccak256(
            &[
                type_hash.as_slice(),
                source_hash.as_slice(),
                action_hash.as_slice(),
            ]
            .concat(),
        )
    };

    // EIP-712 hash = keccak256(0x19 || 0x01 || domainSeparator || structHash)
    let digest = keccak256(
        &[
            &[0x19, 0x01],
            domain_separator.as_slice(),
            struct_hash.as_slice(),
        ]
        .concat(),
    );

    signer.sign_digest(&digest)
}

// ---------------------------------------------------------------------------
// User-Signed EIP-712 (HyperliquidSignTransaction domain)
// ---------------------------------------------------------------------------

/// Sign a user-signed action (EIP-712 on HyperliquidSignTransaction domain).
///
/// Used for: withdraw, usdClassTransfer, usdSend.
/// NOT used for orders/cancels (those use L1 agent signing above).
pub fn sign_user_signed_action(
    signer: &Signer,
    action_type: &str,
    fields: &serde_json::Value,
    is_testnet: bool,
) -> Result<EthSignature, String> {
    let chain_str = if is_testnet { "Testnet" } else { "Mainnet" };

    // Domain: HyperliquidSignTransaction, chainId: 0x66eee
    let domain_separator = {
        let type_hash = keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
        );
        let name_hash = keccak256(b"HyperliquidSignTransaction");
        let version_hash = keccak256(b"1");
        let chain_id = u256_be(0x66eee);
        let verifying_contract = [0u8; 32];

        keccak256(
            &[
                type_hash.as_slice(),
                name_hash.as_slice(),
                version_hash.as_slice(),
                &chain_id,
                &verifying_contract,
            ]
            .concat(),
        )
    };

    let struct_hash = match action_type {
        "withdraw3" => {
            let type_hash = keccak256(
                b"HyperliquidTransaction:Withdraw(string hyperliquidChain,string destination,string amount,uint64 time)",
            );
            let chain_hash = keccak256(chain_str.as_bytes());
            let dest_hash = keccak256(field_str(fields, "destination").as_bytes());
            let amount_hash = keccak256(field_str(fields, "amount").as_bytes());
            let time = field_u64(fields, "time");

            keccak256(
                &[
                    type_hash.as_slice(),
                    chain_hash.as_slice(),
                    dest_hash.as_slice(),
                    amount_hash.as_slice(),
                    &u256_be(time),
                ]
                .concat(),
            )
        }
        "usdClassTransfer" => {
            let type_hash = keccak256(
                b"HyperliquidTransaction:UsdClassTransfer(string hyperliquidChain,string amount,bool toPerp,uint64 nonce)",
            );
            let chain_hash = keccak256(chain_str.as_bytes());
            let amount_hash = keccak256(field_str(fields, "amount").as_bytes());
            let to_perp = field_bool(fields, "toPerp");
            let nonce = field_u64(fields, "nonce");

            keccak256(
                &[
                    type_hash.as_slice(),
                    chain_hash.as_slice(),
                    amount_hash.as_slice(),
                    &u256_be(to_perp as u64),
                    &u256_be(nonce),
                ]
                .concat(),
            )
        }
        "usdSend" => {
            let type_hash = keccak256(
                b"HyperliquidTransaction:UsdSend(string hyperliquidChain,string destination,string amount,uint64 time)",
            );
            let chain_hash = keccak256(chain_str.as_bytes());
            let dest_hash = keccak256(field_str(fields, "destination").as_bytes());
            let amount_hash = keccak256(field_str(fields, "amount").as_bytes());
            let time = field_u64(fields, "time");

            keccak256(
                &[
                    type_hash.as_slice(),
                    chain_hash.as_slice(),
                    dest_hash.as_slice(),
                    amount_hash.as_slice(),
                    &u256_be(time),
                ]
                .concat(),
            )
        }
        _ => {
            return Err(format!("Unsupported user action type: {action_type}"));
        }
    };

    let digest = keccak256(
        &[
            &[0x19, 0x01],
            domain_separator.as_slice(),
            struct_hash.as_slice(),
        ]
        .concat(),
    );

    signer.sign_digest(&digest)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Encode a u64 as a 32-byte big-endian uint256.
fn u256_be(val: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[24..].copy_from_slice(&val.to_be_bytes());
    bytes
}

fn field_str<'a>(fields: &'a serde_json::Value, key: &str) -> &'a str {
    fields[key].as_str().unwrap_or("")
}

fn field_u64(fields: &serde_json::Value, key: &str) -> u64 {
    fields[key].as_u64().unwrap_or(0)
}

fn field_bool(fields: &serde_json::Value, key: &str) -> bool {
    fields[key].as_bool().unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Action sorting (canonical key order for msgpack)
// ---------------------------------------------------------------------------

/// Strip trailing zeros from a decimal string.
/// "0.5000" -> "0.5", "10.00" -> "10", "3" -> "3"
fn format_decimal(s: &str) -> String {
    if !s.contains('.') {
        return s.to_string();
    }
    let trimmed = s.trim_end_matches('0');
    trimmed.trim_end_matches('.').to_string()
}

/// Sort an order action into canonical key order for MessagePack encoding.
/// Key order: type, orders, grouping, builder?
/// Order wire key order: a, b, p, s, r, t, c?
pub fn sort_order_action(action: &serde_json::Value) -> serde_json::Value {
    let orders = action["orders"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|o| {
            let mut wire = serde_json::Map::new();
            wire.insert("a".into(), o["a"].clone());
            wire.insert("b".into(), o["b"].clone());
            wire.insert(
                "p".into(),
                serde_json::json!(format_decimal(o["p"].as_str().unwrap_or("0"))),
            );
            wire.insert(
                "s".into(),
                serde_json::json!(format_decimal(o["s"].as_str().unwrap_or("0"))),
            );
            wire.insert("r".into(), o["r"].clone());
            wire.insert("t".into(), o["t"].clone());
            if let Some(c) = o.get("c") {
                if !c.is_null() {
                    wire.insert("c".into(), c.clone());
                }
            }
            serde_json::Value::Object(wire)
        })
        .collect::<Vec<_>>();

    let mut sorted = serde_json::Map::new();
    sorted.insert("type".into(), action["type"].clone());
    sorted.insert("orders".into(), serde_json::json!(orders));
    sorted.insert("grouping".into(), action["grouping"].clone());

    if let Some(builder) = action.get("builder") {
        if !builder.is_null() {
            let mut b = serde_json::Map::new();
            b.insert(
                "b".into(),
                serde_json::json!(builder["b"].as_str().unwrap_or("").to_lowercase()),
            );
            b.insert("f".into(), builder["f"].clone());
            sorted.insert("builder".into(), serde_json::Value::Object(b));
        }
    }

    serde_json::Value::Object(sorted)
}

/// Sort a cancel action into canonical key order.
/// Key order: type, cancels. Cancel wire key order: a, o
pub fn sort_cancel_action(action: &serde_json::Value) -> serde_json::Value {
    let cancels = action["cancels"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|c| {
            let mut wire = serde_json::Map::new();
            wire.insert("a".into(), c["a"].clone());
            wire.insert("o".into(), c["o"].clone());
            serde_json::Value::Object(wire)
        })
        .collect::<Vec<_>>();

    let mut sorted = serde_json::Map::new();
    sorted.insert("type".into(), action["type"].clone());
    sorted.insert("cancels".into(), serde_json::json!(cancels));
    serde_json::Value::Object(sorted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l1_action_hash_deterministic() {
        let action = serde_json::json!({
            "type": "order",
            "orders": [{"a": 100000100, "b": true, "p": "0.5", "s": "100", "r": false, "t": {"limit": {"tif": "Gtc"}}}],
            "grouping": "na"
        });
        let hash1 = create_l1_action_hash(&action, 1000, None);
        let hash2 = create_l1_action_hash(&action, 1000, None);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_l1_action_hash_nonce_matters() {
        let action = serde_json::json!({"type": "cancel", "cancels": [{"a": 100, "o": 1}]});
        let h1 = create_l1_action_hash(&action, 1000, None);
        let h2 = create_l1_action_hash(&action, 2000, None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_sign_l1_action() {
        let signer = Signer::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let action = sort_order_action(&serde_json::json!({
            "type": "order",
            "orders": [{"a": 100000100, "b": true, "p": "0.5", "s": "100", "r": false, "t": {"limit": {"tif": "Gtc"}}}],
            "grouping": "na"
        }));
        let sig = sign_l1_action(&signer, &action, 1234567890, true, None).unwrap();
        assert!(sig.v == 27 || sig.v == 28);
        assert_ne!(sig.r, [0u8; 32]);
    }

    #[test]
    fn test_sort_order_action_strips_zeros() {
        let action = serde_json::json!({
            "type": "order",
            "orders": [{"a": 100, "b": true, "p": "0.5000", "s": "100.00", "r": false, "t": {"limit": {"tif": "Gtc"}}}],
            "grouping": "na"
        });
        let sorted = sort_order_action(&action);
        let order = &sorted["orders"][0];
        assert_eq!(order["p"].as_str().unwrap(), "0.5");
        assert_eq!(order["s"].as_str().unwrap(), "100");
    }

    #[test]
    fn test_u256_be() {
        let bytes = u256_be(1337);
        assert_eq!(bytes[31], 0x39);
        assert_eq!(bytes[30], 0x05);
        assert_eq!(bytes[0], 0);
    }

    #[test]
    fn test_user_signed_action() {
        let signer = Signer::from_hex(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let fields = serde_json::json!({
            "amount": "100",
            "toPerp": false,
            "nonce": 1234567890u64,
        });
        let sig =
            sign_user_signed_action(&signer, "usdClassTransfer", &fields, true).unwrap();
        assert!(sig.v == 27 || sig.v == 28);
    }
}
