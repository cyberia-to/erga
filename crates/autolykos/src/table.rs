//! The R table and the table-driven hit. R holds genElement(idx,h,M) for
//! every idx in [0,N); mining reads it instead of recomputing the 8 KiB
//! element hash per nonce. The seed's own `f` element is just R[i], so a
//! nonce costs 33 reads + three small hashes — the shape the GPU kernel
//! mirrors. `pow_hit_via_table` is differential-tested to equal the
//! recompute `pow_hit`, which is itself chain-verified in lib.rs.

use crate::{as_unsigned_byte_array_pub, gen_indexes, K};
use blake_bench::reference::blake2b256;
use num_bigint::BigUint;
use num_traits::Zero;

pub const ROW: usize = 31; // bytes per element (take_right(31, hash))

/// One table element: genElement(idx, h, M) = blake2b(idx_be4 || h || M)[1..].
pub fn gen_element(idx: u32, h: &[u8], m: &[u8]) -> [u8; ROW] {
    let mut concat = idx.to_be_bytes().to_vec();
    concat.extend_from_slice(h);
    concat.extend_from_slice(m);
    let full = blake2b256(&concat);
    let mut out = [0u8; ROW];
    out.copy_from_slice(&full[1..]); // drop the top byte
    out
}

/// The precomputed table for one epoch (one height). `n * 31` bytes.
pub struct Table {
    pub n: u32,
    pub rows: Vec<u8>, // n * ROW
    pub h: [u8; 4],
    pub m: Vec<u8>,
}

impl Table {
    /// Build R for `height` single-threaded (tests / small N).
    pub fn build(height: u32, version: u8) -> Table {
        let n = crate::calc_big_n(version, height);
        let h = crate::height_bytes(height);
        let m = crate::big_m();
        let mut rows = vec![0u8; n as usize * ROW];
        for idx in 0..n {
            let e = gen_element(idx, &h, &m);
            let o = idx as usize * ROW;
            rows[o..o + ROW].copy_from_slice(&e);
        }
        Table { n, rows, h, m }
    }

    #[inline]
    pub fn row(&self, idx: u32) -> &[u8] {
        let o = idx as usize * ROW;
        &self.rows[o..o + ROW]
    }
}

/// Hit computed by reading the table (the mining path), not recomputing.
pub fn pow_hit_via_table(table: &Table, msg: &[u8], nonce: &[u8]) -> BigUint {
    // i = last-8-bytes(hash(msg||nonce)) mod N  → f = R[i]
    let mut c = msg.to_vec();
    c.extend_from_slice(nonce);
    let h1 = blake2b256(&c);
    let pre_i8 = BigUint::from_bytes_be(&h1[24..32]);
    let i = (pre_i8 % table.n).to_u32_digits().first().copied().unwrap_or(0);
    let f = table.row(i);

    // seed = hash(f || msg || nonce)
    let mut c = f.to_vec();
    c.extend_from_slice(msg);
    c.extend_from_slice(nonce);
    let seed = blake2b256(&c);

    // 32 indexes → sum their rows
    let indexes = gen_indexes(&seed, table.n);
    let mut f2 = BigUint::zero();
    for idx in indexes.into_iter().take(K) {
        f2 += BigUint::from_bytes_be(table.row(idx));
    }
    let array = as_unsigned_byte_array_pub(32, &f2);
    BigUint::from_bytes_be(&blake2b256(&array))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pow_hit;

    #[test]
    fn table_path_equals_recompute_path() {
        // Small N so the table fits a unit test. The structure is N-agnostic,
        // and the recompute path is chain-verified in lib.rs, so matching it
        // here proves the table read path is correct.
        let height = 614400; // version-2 height, but we force a tiny N below
        let m = crate::big_m();
        // build a small table by hand at N = 4096
        let small_n = 4096u32;
        let h = crate::height_bytes(height);
        let rows: Vec<u8> = (0..small_n)
            .flat_map(|idx| gen_element(idx, &h, &m).to_vec())
            .collect();
        let table = Table { n: small_n, rows, h, m: m.clone() };

        for nonce_val in [0u64, 1, 42, 0x3105, 999_999] {
            let nonce = nonce_val.to_be_bytes();
            let msg = blake2b256(&nonce_val.to_le_bytes()); // arbitrary 32-byte msg
            let via_table = pow_hit_via_table(&table, &msg, &nonce);
            let recompute = pow_hit(&msg, &nonce, &table.h, small_n, &m);
            assert_eq!(via_table, recompute, "table path must equal recompute at nonce {nonce_val}");
        }
    }
}
