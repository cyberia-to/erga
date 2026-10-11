use crate::BenchError;
use blake_bench::reference::blake2b256;
use rtable_bench::{RTable, ROW_BYTES};

fn row_count(length: usize) -> Result<u64, BenchError> {
    let rows = length / ROW_BYTES;
    if length == 0 || !length.is_multiple_of(ROW_BYTES) || rows as u64 > 1u64 << 32 {
        return Err(BenchError::InvalidInput(
            "table bytes require 1..=2^32 complete rows",
        ));
    }
    Ok(rows as u64)
}

/// Full simplified benchmark digest. The slice supplies all initialized rows.
pub fn mine_one(m: &[u8; 32], nonce: u64, rows: &[u8]) -> Result<[u8; 32], BenchError> {
    Ok(hash(m, nonce, rows, row_count(rows.len())?))
}

/// XOR full digests over wrapping consecutive nonces from a retained table.
pub fn cpu_accumulate(
    m: &[u8; 32],
    base: u64,
    count: u32,
    table: &RTable<'_>,
) -> Result<[u8; 32], BenchError> {
    let bytes = table.bytes()?;
    let rows = row_count(bytes.len())?;
    let mut output = [0; 32];
    for i in 0..u64::from(count) {
        let digest = hash(m, base.wrapping_add(i), bytes, rows);
        for (out, value) in output.iter_mut().zip(digest) {
            *out ^= value;
        }
    }
    Ok(output)
}

// Only validated complete nonempty slices reach pointer arithmetic below.
fn hash(m: &[u8; 32], nonce: u64, r_bytes: &[u8], n: u64) -> [u8; 32] {
    let mut input = [0u8; 40];
    input[..32].copy_from_slice(m);
    input[32..40].copy_from_slice(&nonce.to_le_bytes());
    let seed_hash: [u8; 32] = blake2b256(&input);

    let mut ext = [0u8; 35];
    ext[..32].copy_from_slice(&seed_hash);
    ext[32] = seed_hash[0];
    ext[33] = seed_hash[1];
    ext[34] = seed_hash[2];

    // Precompute all 32 row offsets upfront.
    let mut offsets = [0usize; 32];
    for i in 0..32 {
        let be = u32::from_be_bytes([ext[i], ext[i + 1], ext[i + 2], ext[i + 3]]);
        let idx = (be as u64) % n;
        offsets[i] = (idx as usize) * ROW_BYTES;
    }

    // Issue prefetches for the first few rows BEFORE entering the loop.
    const PREFETCH_AHEAD: usize = 4;
    for &off in offsets.iter().take(PREFETCH_AHEAD.min(32)) {
        // SAFETY: validated rows bound every offset within the immutable slice.
        prefetch_read(unsafe { r_bytes.as_ptr().add(off) });
    }

    let mut sum: [u64; 4] = [0; 4];
    for i in 0..32 {
        // Prefetch row i+PREFETCH_AHEAD (if in range) before consuming row i.
        if i + PREFETCH_AHEAD < 32 {
            // SAFETY: each validated index addresses a complete initialized row.
            prefetch_read(unsafe { r_bytes.as_ptr().add(offsets[i + PREFETCH_AHEAD]) });
        }
        let off = offsets[i];
        let row = &r_bytes[off..off + ROW_BYTES];
        let mut word0 = [0; 8];
        word0.copy_from_slice(&row[0..8]);
        let r0 = u64::from_le_bytes(word0);
        let mut word1 = [0; 8];
        word1.copy_from_slice(&row[8..16]);
        let r1 = u64::from_le_bytes(word1);
        let mut word2 = [0; 8];
        word2.copy_from_slice(&row[16..24]);
        let r2 = u64::from_le_bytes(word2);
        let mut word3 = [0; 8];
        word3.copy_from_slice(&row[24..32]);
        let r3 = u64::from_le_bytes(word3);
        add_256(&mut sum, [r0, r1, r2, r3]);
    }

    let mut sum_bytes = [0u8; 32];
    sum_bytes[0..8].copy_from_slice(&sum[0].to_le_bytes());
    sum_bytes[8..16].copy_from_slice(&sum[1].to_le_bytes());
    sum_bytes[16..24].copy_from_slice(&sum[2].to_le_bytes());
    sum_bytes[24..32].copy_from_slice(&sum[3].to_le_bytes());
    blake2b256(&sum_bytes)
}

#[inline(always)]
fn prefetch_read(ptr: *const u8) {
    // ARM64 `prfm pldl1keep, [addr]` — prefetch for read, keep in L1.
    // No-op if the data is already cached, or if prefetcher is disabled.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        std::arch::asm!(
            "prfm pldl1keep, [{0}]",
            in(reg) ptr,
            options(nostack, readonly, preserves_flags)
        );
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = ptr;
    }
}

#[inline]
fn add_256(sum: &mut [u64; 4], r: [u64; 4]) {
    let mut carry: u128 = 0;
    for i in 0..4 {
        let s = sum[i] as u128 + r[i] as u128 + carry;
        sum[i] = s as u64;
        carry = s >> 64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn row_domain_precedes_any_pointer_arithmetic() {
        assert_eq!(row_count(32).unwrap(), 1);
        assert_eq!(row_count(1usize << 37).unwrap(), 1u64 << 32);
        for length in [0, 31, 33, (1usize << 37) + 32, usize::MAX] {
            assert!(row_count(length).is_err());
        }
    }
}
