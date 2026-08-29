//! Protocol-exact Autolykos v2 — the CPU reference oracle.
//!
//! Every function here mirrors `ergoplatform/sigma-rust`
//! `ergo-chain-types/src/autolykos_pow_scheme.rs` byte for byte, and the
//! test at the bottom reproduces that repo's own test vector (height
//! 614400, nonce 0x3105 → hit 0x0002fcb1…412a). If that test passes, this
//! implementation computes the same hit the Ergo network does, so a nonce
//! it accepts is a nonce a pool accepts. The GPU kernel is validated
//! against THIS reference; this reference is validated against the chain.

use blake_bench::reference::blake2b256;
use num_bigint::BigUint;
use num_traits::Zero;

pub mod table;
pub mod search;

pub const K: usize = 32; // reads per nonce
pub const N_BASE: u32 = 1 << 26; // 67,108,864

/// M — the 8 KiB pad: 1024 big-endian u64 of 0..1023, concatenated.
pub fn big_m() -> Vec<u8> {
    (0u64..1024).flat_map(|x| x.to_be_bytes()).collect()
}

/// N for a given header version and height (table row count).
pub fn calc_big_n(header_version: u8, header_height: u32) -> u32 {
    if header_version == 1 {
        return N_BASE;
    }
    let n_increasement_height_max = 4_198_400usize;
    let height = (header_height as usize).min(n_increasement_height_max);
    let increase_start = 600 * 1024; // 614400
    if height < increase_start {
        N_BASE
    } else {
        let increase_period = 50 * 1024; // 51200
        let iters = (height - increase_start) / increase_period + 1;
        (1..=iters).fold(N_BASE, |acc, _| acc / 100 * 105)
    }
}

/// Height bytes as fed into the element and seed hashes: 4-byte big-endian.
pub fn height_bytes(height: u32) -> [u8; 4] {
    height.to_be_bytes()
}

/// Public re-export for the table module (same padding rule).
pub fn as_unsigned_byte_array_pub(len: usize, x: &BigUint) -> Vec<u8> {
    as_unsigned_byte_array(len, x)
}

/// Left-zero-padded big-endian bytes of `x`, exactly `len` wide
/// (Java `BigIntegers.asUnsignedByteArray(len, x)`).
fn as_unsigned_byte_array(len: usize, x: &BigUint) -> Vec<u8> {
    let mut b = x.to_bytes_be(); // minimal; zero → [0]
    if x.is_zero() {
        b.clear();
    }
    assert!(b.len() <= len, "value wider than {len} bytes");
    let mut out = vec![0u8; len - b.len()];
    out.extend_from_slice(&b);
    out
}

/// 32 pseudorandom indexes in [0, N) from the seed hash.
pub fn gen_indexes(seed_hash: &[u8; 32], big_n: u32) -> Vec<u32> {
    let mut ext = seed_hash.to_vec();
    ext.extend_from_slice(&seed_hash[..3]); // 35 bytes
    (0..K)
        .map(|i| {
            let w = u32::from_be_bytes([ext[i], ext[i + 1], ext[i + 2], ext[i + 3]]);
            (w as u64 % big_n as u64) as u32
        })
        .collect()
}

/// The seed hash mixing msg, nonce, height and one table element.
pub fn calc_seed_v2(big_n: u32, msg: &[u8], nonce: &[u8], h: &[u8], m: &[u8]) -> [u8; 32] {
    let mut concat = Vec::with_capacity(msg.len() + nonce.len());
    concat.extend_from_slice(msg);
    concat.extend_from_slice(nonce);
    let hash1 = blake2b256(&concat);
    let pre_i8 = BigUint::from_bytes_be(&hash1[24..32]); // last 8 bytes
    let i = as_unsigned_byte_array(4, &(pre_i8 % big_n));

    let mut concat = i;
    concat.extend_from_slice(h);
    concat.extend_from_slice(m);
    let f = blake2b256(&concat);

    let mut concat = f[1..].to_vec();
    concat.extend_from_slice(msg);
    concat.extend_from_slice(nonce);
    blake2b256(&concat)
}

/// The full hit: the 256-bit value a candidate must beat the target with.
pub fn pow_hit(msg: &[u8], nonce: &[u8], h: &[u8], big_n: u32, m: &[u8]) -> BigUint {
    let seed = calc_seed_v2(big_n, msg, nonce, h, m);
    let indexes = gen_indexes(&seed, big_n);
    let mut f2 = BigUint::zero();
    for idx in indexes {
        let mut concat = idx.to_be_bytes().to_vec(); // 4 bytes
        concat.extend_from_slice(h);
        concat.extend_from_slice(m);
        f2 += BigUint::from_bytes_be(&blake2b256(&concat)[1..]); // 31 bytes
    }
    let array = as_unsigned_byte_array(32, &f2);
    BigUint::from_bytes_be(&blake2b256(&array))
}

/// A candidate is valid when its hit is below `target` (= 2^256 / difficulty
/// on the chain, or the share target a pool sets).
pub fn meets_target(hit: &BigUint, target: &BigUint) -> bool {
    hit < target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexb(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn big_m_is_8192_bytes() {
        let m = big_m();
        assert_eq!(m.len(), 8192);
        assert_eq!(&m[0..8], &[0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&m[8..16], &[0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(&m[8184..8192], &[0, 0, 0, 0, 0, 0, 3, 255]); // 1023
    }

    #[test]
    fn n_at_first_increase() {
        // sigma-rust vector: height 614400 → N grows once to 70,464,240
        assert_eq!(calc_big_n(2, 614400), 70_464_240);
        assert_eq!(calc_big_n(2, 0), N_BASE);
        assert_eq!(calc_big_n(2, 614399), N_BASE);
    }

    #[test]
    fn hit_matches_sigma_rust_vector() {
        // The keystone: reproduce the exact hit sigma-rust computes for a
        // real mainnet header. If this matches, the algorithm is chain-exact.
        let msg = hexb("548c3e602a8f36f8f2738f5f643b02425038044d98543a51cabaa9785e7e864f");
        let nonce = hexb("0000000000003105");
        let h = height_bytes(614400);
        let n = calc_big_n(2, 614400);
        let m = big_m();
        let hit = pow_hit(&msg, &nonce, &h, n, &m);
        let expected = BigUint::from_bytes_be(&hexb(
            "0002fcb113fe65e5754959872dfdbffea0489bf830beb4961ddc0e9e66a1412a",
        ));
        assert_eq!(hit, expected, "hit must match the chain's own computation");
    }
}
