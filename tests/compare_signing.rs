//! Comparison tests: verify from-scratch signing matches hypersdk/alloy output.
//!
//! Run with: cargo test --features compare --test compare_signing

#![cfg(feature = "compare")]

use alloy::signers::SignerSync;
use hip4::crypto::eip712::{create_l1_action_hash, sign_l1_action, sort_order_action};
use hip4::crypto::keccak::keccak256;
use hip4::crypto::msgpack;
use hip4::crypto::signer::Signer;

/// Shared test key (private key 1 - well-known test vector, never use on mainnet).
const TEST_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000001";

// ---------------------------------------------------------------------------
// Keccak-256: compare our implementation against alloy's
// ---------------------------------------------------------------------------

#[test]
fn compare_keccak256_empty() {
    let ours = keccak256(b"");
    let theirs = alloy::primitives::keccak256(b"");
    assert_eq!(ours.as_slice(), theirs.as_slice(), "keccak256('') mismatch");
}

#[test]
fn compare_keccak256_hello() {
    let ours = keccak256(b"hello world");
    let theirs = alloy::primitives::keccak256(b"hello world");
    assert_eq!(
        ours.as_slice(),
        theirs.as_slice(),
        "keccak256('hello world') mismatch"
    );
}

#[test]
fn compare_keccak256_long_input() {
    // Input longer than one keccak block (136 bytes)
    let data: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let ours = keccak256(&data);
    let theirs = alloy::primitives::keccak256(&data);
    assert_eq!(
        ours.as_slice(),
        theirs.as_slice(),
        "keccak256(256 bytes) mismatch"
    );
}

// ---------------------------------------------------------------------------
// Address derivation: compare against alloy PrivateKeySigner
// ---------------------------------------------------------------------------

#[test]
fn compare_address_derivation() {
    let our_signer = Signer::from_hex(TEST_KEY).unwrap();
    let alloy_signer: alloy::signers::local::PrivateKeySigner = TEST_KEY.parse().unwrap();

    let our_addr = our_signer.address_hex().to_lowercase();
    let alloy_addr = format!("{:?}", alloy_signer.address()).to_lowercase();

    assert_eq!(our_addr, alloy_addr, "Address derivation mismatch");
}

// ---------------------------------------------------------------------------
// EIP-712 digest: compare Agent signing digest against manual alloy construction
// ---------------------------------------------------------------------------

#[test]
fn compare_l1_action_hash() {
    // Build a canonical order action
    let action = sort_order_action(&serde_json::json!({
        "type": "order",
        "orders": [{
            "a": 100000100,
            "b": true,
            "p": "0.55",
            "s": "100",
            "r": false,
            "t": {"limit": {"tif": "Gtc"}}
        }],
        "grouping": "na"
    }));

    let nonce: u64 = 1712345678000;

    // Our action hash
    let our_hash = create_l1_action_hash(&action, nonce, None);

    // Manually replicate the same process using alloy's keccak
    let msgpack_bytes = msgpack::encode(&action);
    let nonce_bytes = nonce.to_be_bytes();
    let vault_bytes = [0x00u8];

    let mut combined = Vec::new();
    combined.extend_from_slice(&msgpack_bytes);
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&vault_bytes);

    let alloy_hash = alloy::primitives::keccak256(&combined);

    assert_eq!(
        our_hash.as_slice(),
        alloy_hash.as_slice(),
        "L1 action hash mismatch"
    );
}

// ---------------------------------------------------------------------------
// ECDSA signature: compare our k256 signing against alloy signing
// ---------------------------------------------------------------------------

#[test]
fn compare_ecdsa_signatures() {
    // Both signers should produce identical signatures for the same digest
    // (ECDSA with RFC 6979 deterministic k is fully deterministic)
    let our_signer = Signer::from_hex(TEST_KEY).unwrap();
    let alloy_signer: alloy::signers::local::PrivateKeySigner = TEST_KEY.parse().unwrap();

    let digest = keccak256(b"test order signing payload");

    // Our signature
    let our_sig = our_signer.sign_digest(&digest).unwrap();

    // Alloy signature
    let alloy_digest = alloy::primitives::B256::from(digest);
    let alloy_sig = alloy_signer
        .sign_hash_sync(&alloy_digest)
        .expect("alloy sign failed");

    let alloy_bytes = alloy_sig.as_bytes();
    let alloy_r = &alloy_bytes[..32];
    let alloy_s = &alloy_bytes[32..64];
    let alloy_v = alloy_bytes[64];

    assert_eq!(our_sig.r, alloy_r, "r component mismatch");
    assert_eq!(our_sig.s, alloy_s, "s component mismatch");
    assert_eq!(our_sig.v, alloy_v, "v (recovery id) mismatch");
}

// ---------------------------------------------------------------------------
// Full L1 action sign: compare end-to-end
// ---------------------------------------------------------------------------

#[test]
fn compare_full_l1_sign() {
    let our_signer = Signer::from_hex(TEST_KEY).unwrap();
    let alloy_signer: alloy::signers::local::PrivateKeySigner = TEST_KEY.parse().unwrap();

    let action = sort_order_action(&serde_json::json!({
        "type": "order",
        "orders": [{
            "a": 100005160,
            "b": true,
            "p": "0.35",
            "s": "50",
            "r": false,
            "t": {"limit": {"tif": "FrontendMarket"}}
        }],
        "grouping": "na"
    }));

    let nonce: u64 = 1712345678000;
    let is_testnet = true;

    // Our full signing
    let our_sig = sign_l1_action(&our_signer, &action, nonce, is_testnet, None).unwrap();

    // Replicate with alloy: compute the same EIP-712 digest manually
    let action_hash = create_l1_action_hash(&action, nonce, None);
    let connection_id = alloy::primitives::B256::from(action_hash);

    // Agent EIP-712 domain
    let domain_type_hash = alloy::primitives::keccak256(
        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
    );
    let name_hash = alloy::primitives::keccak256(b"Exchange");
    let version_hash = alloy::primitives::keccak256(b"1");

    let mut domain_input = Vec::new();
    domain_input.extend_from_slice(domain_type_hash.as_slice());
    domain_input.extend_from_slice(name_hash.as_slice());
    domain_input.extend_from_slice(version_hash.as_slice());
    domain_input.extend_from_slice(&alloy::primitives::U256::from(1337).to_be_bytes::<32>());
    domain_input.extend_from_slice(&[0u8; 32]); // zero address
    let domain_separator = alloy::primitives::keccak256(&domain_input);

    let struct_type_hash =
        alloy::primitives::keccak256(b"Agent(string source,bytes32 connectionId)");
    let source_hash = alloy::primitives::keccak256(b"b"); // testnet

    let mut struct_input = Vec::new();
    struct_input.extend_from_slice(struct_type_hash.as_slice());
    struct_input.extend_from_slice(source_hash.as_slice());
    struct_input.extend_from_slice(connection_id.as_slice());
    let struct_hash = alloy::primitives::keccak256(&struct_input);

    let mut eip712_input = Vec::new();
    eip712_input.extend_from_slice(&[0x19, 0x01]);
    eip712_input.extend_from_slice(domain_separator.as_slice());
    eip712_input.extend_from_slice(struct_hash.as_slice());
    let digest = alloy::primitives::keccak256(&eip712_input);

    let alloy_sig = alloy_signer
        .sign_hash_sync(&digest)
        .expect("alloy sign failed");
    let alloy_bytes = alloy_sig.as_bytes();

    assert_eq!(our_sig.r, &alloy_bytes[..32], "Full L1 sign: r mismatch");
    assert_eq!(
        our_sig.s,
        &alloy_bytes[32..64],
        "Full L1 sign: s mismatch"
    );
    assert_eq!(our_sig.v, alloy_bytes[64], "Full L1 sign: v mismatch");
}
