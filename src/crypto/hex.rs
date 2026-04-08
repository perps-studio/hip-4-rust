/// Encode bytes as lowercase hex string (no 0x prefix).
pub fn encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Encode bytes as 0x-prefixed lowercase hex string.
pub fn encode_prefixed(bytes: &[u8]) -> String {
    format!("0x{}", encode(bytes))
}

/// Decode hex string (with or without 0x prefix) into bytes.
pub fn decode(hex: &str) -> Result<Vec<u8>, String> {
    let raw = hex.strip_prefix("0x").unwrap_or(hex);
    if raw.len() % 2 != 0 {
        return Err("Hex string must have even length".into());
    }
    let mut bytes = Vec::with_capacity(raw.len() / 2);
    for i in (0..raw.len()).step_by(2) {
        let byte = u8::from_str_radix(&raw[i..i + 2], 16)
            .map_err(|e| format!("Invalid hex at position {i}: {e}"))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let data = b"hello world";
        let hex = encode(data);
        assert_eq!(hex, "68656c6c6f20776f726c64");
        assert_eq!(decode(&hex).unwrap(), data);
    }

    #[test]
    fn test_prefix() {
        let data = [0xde, 0xad, 0xbe, 0xef];
        assert_eq!(encode_prefixed(&data), "0xdeadbeef");
        assert_eq!(decode("0xdeadbeef").unwrap(), data);
    }
}
