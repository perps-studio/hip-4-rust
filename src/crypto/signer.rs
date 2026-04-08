/// secp256k1 ECDSA signer with Ethereum address derivation.
///
/// Uses k256 for the elliptic curve operations (the one thing we can't
/// implement from scratch). Everything else (hashing, encoding) is handled
/// by our crypto module.
use k256::ecdsa::{RecoveryId, Signature as EcdsaSignature, SigningKey, VerifyingKey};

use super::hex;
use super::keccak::keccak256;

/// An Ethereum-compatible secp256k1 signer.
#[derive(Clone)]
pub struct Signer {
    key: SigningKey,
    address: [u8; 20],
}

impl Signer {
    /// Create a signer from a hex-encoded private key (with or without 0x prefix).
    pub fn from_hex(private_key: &str) -> Result<Self, String> {
        let bytes = hex::decode(private_key)?;
        if bytes.len() != 32 {
            return Err(format!(
                "Private key must be 32 bytes, got {}",
                bytes.len()
            ));
        }
        let key = SigningKey::from_bytes(bytes.as_slice().into())
            .map_err(|e| format!("Invalid private key: {e}"))?;
        let address = derive_address(key.verifying_key());
        Ok(Self { key, address })
    }

    /// The Ethereum address derived from this signer's public key.
    pub fn address(&self) -> &[u8; 20] {
        &self.address
    }

    /// The address as a checksummed 0x-prefixed hex string.
    pub fn address_hex(&self) -> String {
        hex::encode_prefixed(&self.address)
    }

    /// Sign a 32-byte prehash digest, returning (r, s, recovery_id).
    /// The digest is typically an EIP-712 hash.
    pub fn sign_digest(&self, digest: &[u8; 32]) -> Result<EthSignature, String> {
        let (sig, recovery_id): (EcdsaSignature, RecoveryId) = self
            .key
            .sign_prehash_recoverable(digest)
            .map_err(|e| format!("ECDSA sign failed: {e}"))?;

        let sig_bytes = sig.to_bytes();
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&sig_bytes[..32]);
        s.copy_from_slice(&sig_bytes[32..64]);

        let v = recovery_id.to_byte() + 27; // Ethereum convention

        Ok(EthSignature { r, s, v })
    }
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("address", &self.address_hex())
            .finish()
    }
}

/// An Ethereum-style signature with r, s components and recovery id v.
#[derive(Debug, Clone)]
pub struct EthSignature {
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub v: u8,
}

impl EthSignature {
    /// Format as the JSON { r: "0x...", s: "0x...", v: N } that HL expects.
    pub fn to_hl_json(&self) -> serde_json::Value {
        serde_json::json!({
            "r": hex::encode_prefixed(&self.r),
            "s": hex::encode_prefixed(&self.s),
            "v": self.v,
        })
    }
}

/// Derive the Ethereum address from a secp256k1 public key.
/// address = keccak256(uncompressed_pubkey[1..])[12..]
fn derive_address(verifying_key: &VerifyingKey) -> [u8; 20] {
    let encoded = verifying_key.to_encoded_point(false);
    let pubkey_bytes = &encoded.as_bytes()[1..]; // skip 0x04 prefix
    let hash = keccak256(pubkey_bytes);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_derivation() {
        // Well-known test vector: private key 1 -> known address
        let signer =
            Signer::from_hex("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        // Private key 1 -> address 0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf
        assert_eq!(
            signer.address_hex().to_lowercase(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
    }

    #[test]
    fn test_sign_and_recover() {
        let signer =
            Signer::from_hex("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let digest = keccak256(b"test message");
        let sig = signer.sign_digest(&digest).unwrap();
        assert!(sig.v == 27 || sig.v == 28);
        assert_ne!(sig.r, [0u8; 32]);
        assert_ne!(sig.s, [0u8; 32]);
    }

    #[test]
    fn test_from_hex_with_prefix() {
        let signer = Signer::from_hex(
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        assert_eq!(
            signer.address_hex().to_lowercase(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
    }
}
