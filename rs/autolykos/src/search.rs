//! Share search: given a job (msg, table, target) find a nonce whose hit
//! clears the target. This is the CPU reference searcher — correct, not
//! fast; the GPU kernel is the throughput version, validated to find the
//! same solutions. A found nonce is checked against the chain-verified
//! `pow_hit_via_table`, so "found" means "a pool will accept it".

use crate::table::{pow_hit_via_table, Table};
use num_bigint::BigUint;

/// Target from a pool `b` string (big integer, decimal) — a share is valid
/// when hit < b. Pools send `b` directly in the job.
pub fn target_from_b(b_decimal: &str) -> Option<BigUint> {
    b_decimal.parse::<BigUint>().ok()
}

/// Target from a difficulty D: b = 2^256 / D (the chain's convention).
pub fn target_from_difficulty(d: &BigUint) -> BigUint {
    let two256 = BigUint::from(1u8) << 256;
    two256 / d
}

pub struct Found {
    pub nonce: [u8; 8],
    pub hit: BigUint,
}

/// Scan `count` nonces from `start`; return the first whose hit < target.
pub fn search(table: &Table, msg: &[u8], start: u64, count: u64, target: &BigUint) -> Option<Found> {
    for k in 0..count {
        let nonce = start.wrapping_add(k).to_be_bytes();
        let hit = pow_hit_via_table(table, msg, &nonce);
        if &hit < target {
            return Some(Found { nonce, hit });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pow_hit;
    use blake_bench::reference::blake2b256;

    #[test]
    fn finds_a_nonce_below_an_easy_target() {
        // small table, generous target → a solution appears within a few
        // thousand nonces, and we re-check it the recompute way.
        let height = 614400;
        let m = crate::big_m();
        let n = 4096u32;
        let h = crate::height_bytes(height);
        let rows: Vec<u8> = (0..n)
            .flat_map(|idx| crate::table::gen_element(idx, &h, &m).to_vec())
            .collect();
        let table = Table { n, rows, h, m: m.clone() };
        let msg = blake2b256(b"erga share search test");

        // target = 2^248 → roughly 1 in 256 nonces clear it
        let target = BigUint::from(1u8) << 248;
        let found = search(&table, &msg, 0, 100_000, &target).expect("a share within 100k nonces");
        assert!(found.hit < target);
        // independent confirmation via the recompute path
        let recheck = pow_hit(&msg, &found.nonce, &table.h, n, &m);
        assert_eq!(recheck, found.hit, "found nonce must verify the recompute way");
        assert!(recheck < target, "and clear the target under recompute");
    }
}
