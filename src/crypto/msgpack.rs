/// Minimal MessagePack encoder supporting: maps, arrays, strings, integers,
/// booleans, and null. Prices are strings in wire format so floats are not needed.
///
/// Direct port from the TypeScript HIP-4 SDK (signing.ts).
/// Spec: https://github.com/msgpack/msgpack/blob/master/spec.md

pub fn encode(value: &serde_json::Value) -> Vec<u8> {
    let mut buf = Vec::new();
    encode_value(value, &mut buf);
    buf
}

fn encode_value(value: &serde_json::Value, buf: &mut Vec<u8>) {
    match value {
        serde_json::Value::Null => buf.push(0xc0),
        serde_json::Value::Bool(b) => buf.push(if *b { 0xc3 } else { 0xc2 }),
        serde_json::Value::Number(n) => encode_number(n, buf),
        serde_json::Value::String(s) => encode_string(s, buf),
        serde_json::Value::Array(arr) => encode_array(arr, buf),
        serde_json::Value::Object(map) => encode_map(map, buf),
    }
}

fn encode_number(n: &serde_json::Number, buf: &mut Vec<u8>) {
    if let Some(i) = n.as_i64() {
        encode_integer(i, buf);
    } else if let Some(u) = n.as_u64() {
        encode_uint64(u, buf);
    } else if let Some(f) = n.as_f64() {
        // float64: 0xcb + 8 bytes big-endian
        buf.push(0xcb);
        buf.extend_from_slice(&f.to_be_bytes());
    }
}

fn encode_integer(n: i64, buf: &mut Vec<u8>) {
    if n >= 0 {
        encode_uint64(n as u64, buf);
    } else if n >= -32 {
        // negative fixint: 0xe0 | (n & 0x1f)
        buf.push((n as u8) & 0xff);
    } else if n >= -128 {
        buf.push(0xd0);
        buf.push(n as i8 as u8);
    } else if n >= -32768 {
        buf.push(0xd1);
        buf.extend_from_slice(&(n as i16).to_be_bytes());
    } else if n >= -2147483648 {
        buf.push(0xd2);
        buf.extend_from_slice(&(n as i32).to_be_bytes());
    } else {
        buf.push(0xd3);
        buf.extend_from_slice(&n.to_be_bytes());
    }
}

fn encode_uint64(n: u64, buf: &mut Vec<u8>) {
    if n <= 0x7f {
        buf.push(n as u8);
    } else if n <= 0xff {
        buf.push(0xcc);
        buf.push(n as u8);
    } else if n <= 0xffff {
        buf.push(0xcd);
        buf.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= 0xffffffff {
        buf.push(0xce);
        buf.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        buf.push(0xcf);
        buf.extend_from_slice(&n.to_be_bytes());
    }
}

fn encode_string(s: &str, buf: &mut Vec<u8>) {
    let bytes = s.as_bytes();
    let len = bytes.len();
    if len <= 31 {
        buf.push(0xa0 | len as u8);
    } else if len <= 0xff {
        buf.push(0xd9);
        buf.push(len as u8);
    } else if len <= 0xffff {
        buf.push(0xda);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        buf.push(0xdb);
        buf.extend_from_slice(&(len as u32).to_be_bytes());
    }
    buf.extend_from_slice(bytes);
}

fn encode_array(arr: &[serde_json::Value], buf: &mut Vec<u8>) {
    let len = arr.len();
    if len <= 15 {
        buf.push(0x90 | len as u8);
    } else if len <= 0xffff {
        buf.push(0xdc);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        buf.push(0xdd);
        buf.extend_from_slice(&(len as u32).to_be_bytes());
    }
    for item in arr {
        encode_value(item, buf);
    }
}

fn encode_map(map: &serde_json::Map<String, serde_json::Value>, buf: &mut Vec<u8>) {
    let len = map.len();
    if len <= 15 {
        buf.push(0x80 | len as u8);
    } else if len <= 0xffff {
        buf.push(0xde);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        buf.push(0xdf);
        buf.extend_from_slice(&(len as u32).to_be_bytes());
    }
    // serde_json::Map preserves insertion order - critical for canonical hashing
    for (key, value) in map {
        encode_string(key, buf);
        encode_value(value, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_null() {
        assert_eq!(encode(&serde_json::Value::Null), vec![0xc0]);
    }

    #[test]
    fn test_encode_bool() {
        assert_eq!(encode(&serde_json::json!(true)), vec![0xc3]);
        assert_eq!(encode(&serde_json::json!(false)), vec![0xc2]);
    }

    #[test]
    fn test_encode_positive_fixint() {
        assert_eq!(encode(&serde_json::json!(0)), vec![0x00]);
        assert_eq!(encode(&serde_json::json!(42)), vec![42]);
        assert_eq!(encode(&serde_json::json!(127)), vec![127]);
    }

    #[test]
    fn test_encode_uint8() {
        assert_eq!(encode(&serde_json::json!(128)), vec![0xcc, 128]);
        assert_eq!(encode(&serde_json::json!(255)), vec![0xcc, 255]);
    }

    #[test]
    fn test_encode_fixstr() {
        let result = encode(&serde_json::json!("abc"));
        assert_eq!(result[0], 0xa0 | 3);
        assert_eq!(&result[1..], b"abc");
    }

    #[test]
    fn test_encode_map() {
        let val = serde_json::json!({"a": 1, "b": true});
        let result = encode(&val);
        // fixmap with 2 entries
        assert_eq!(result[0] & 0xf0, 0x80);
    }

    #[test]
    fn test_encode_array() {
        let val = serde_json::json!([1, 2, 3]);
        let result = encode(&val);
        assert_eq!(result[0], 0x90 | 3);
    }
}
