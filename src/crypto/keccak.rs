/// Keccak-256 hash (Ethereum variant).
///
/// Parameters: rate=1088 bits (136 bytes), capacity=512 bits, output=256 bits.
/// Padding: Keccak (suffix 0x01), NOT SHA-3 (suffix 0x06).
///
/// Direct port from the TypeScript HIP-4 SDK (signing.ts).

const ROUND_CONSTANTS: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

const ROTATION_OFFSETS: [u32; 25] = [
    0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56,
    14,
];

// Pi permutation: for linear index i = x + 5*y, new index = y + 5*(2*x + 3*y) % 5
const PI_PERMUTATION: [usize; 25] = {
    let mut perm = [0usize; 25];
    let mut x = 0;
    while x < 5 {
        let mut y = 0;
        while y < 5 {
            let from_idx = x + 5 * y;
            let new_x = y;
            let new_y = (2 * x + 3 * y) % 5;
            perm[from_idx] = new_x + 5 * new_y;
            y += 1;
        }
        x += 1;
    }
    perm
};

#[inline]
fn rotl64(x: u64, n: u32) -> u64 {
    if n == 0 {
        x
    } else {
        (x << n) | (x >> (64 - n))
    }
}

fn keccak_f1600(state: &mut [u64; 25]) {
    for round in 0..24 {
        // Theta
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        for x in 0..5 {
            let d = c[(x + 4) % 5] ^ rotl64(c[(x + 1) % 5], 1);
            for y in 0..5 {
                state[x + 5 * y] ^= d;
            }
        }

        // Rho + Pi (combined)
        let mut temp = [0u64; 25];
        for i in 0..25 {
            temp[PI_PERMUTATION[i]] = rotl64(state[i], ROTATION_OFFSETS[i]);
        }

        // Chi
        for y in 0..5 {
            let base = 5 * y;
            let t0 = temp[base];
            let t1 = temp[base + 1];
            let t2 = temp[base + 2];
            let t3 = temp[base + 3];
            let t4 = temp[base + 4];
            state[base] = t0 ^ (!t1 & t2);
            state[base + 1] = t1 ^ (!t2 & t3);
            state[base + 2] = t2 ^ (!t3 & t4);
            state[base + 3] = t3 ^ (!t4 & t0);
            state[base + 4] = t4 ^ (!t0 & t1);
        }

        // Iota
        state[0] ^= ROUND_CONSTANTS[round];
    }
}

fn xor_block(state: &mut [u64; 25], data: &[u8], offset: usize, len: usize) {
    let words = len / 8;
    for i in 0..words {
        let mut word = 0u64;
        let base = offset + i * 8;
        for b in 0..8 {
            word |= (data[base + b] as u64) << (b * 8);
        }
        state[i] ^= word;
    }
}

/// Compute the Keccak-256 hash of the input data.
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let rate = 136; // bytes
    let mut state = [0u64; 25];

    // Absorb
    let mut offset = 0;
    while offset + rate <= data.len() {
        xor_block(&mut state, data, offset, rate);
        keccak_f1600(&mut state);
        offset += rate;
    }

    // Pad last block
    let remaining = data.len() - offset;
    let mut last_block = vec![0u8; rate];
    last_block[..remaining].copy_from_slice(&data[offset..offset + remaining]);
    // Keccak padding: 0x01 after message, 0x80 at end of block
    last_block[remaining] = 0x01;
    last_block[rate - 1] |= 0x80;
    xor_block(&mut state, &last_block, 0, rate);
    keccak_f1600(&mut state);

    // Squeeze (only need 32 bytes = 256 bits, which is < rate)
    let mut output = [0u8; 32];
    for i in 0..4 {
        let word = state[i];
        for b in 0..8 {
            output[i * 8 + b] = ((word >> (b * 8)) & 0xff) as u8;
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hex;

    #[test]
    fn test_empty_input() {
        // Known test vector: keccak256("") = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
        let hash = keccak256(b"");
        assert_eq!(
            hex::encode(&hash),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    #[test]
    fn test_hello_world() {
        // keccak256("hello world")
        let hash = keccak256(b"hello world");
        assert_eq!(
            hex::encode(&hash),
            "47173285a8d7341e5e972fc677286384f802f8ef42a5ec5f03bbfa254cb01fad"
        );
    }

    #[test]
    fn test_ethereum_address_preimage() {
        // keccak256("abc") - common test vector
        let hash = keccak256(b"abc");
        assert_eq!(
            hex::encode(&hash),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
    }
}
