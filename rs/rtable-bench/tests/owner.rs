use aruminium::Gpu;
use blake_bench::reference::blake2b256;
use rtable_bench::{Probe, RTable, ROW_BYTES};

#[path = "support/vectors.rs"]
mod vectors;
const SEED: u64 = 0xa5a5_a5a5_dead_beef;

#[test]
fn literal_row_function_vectors_include_independently_published_eight_bytes() {
    for &(height, index, expected) in vectors::ROWS {
        let mut input = [0; 8];
        input[..4].copy_from_slice(&index.to_le_bytes());
        input[4..].copy_from_slice(&height.to_le_bytes());
        assert_eq!(
            blake2b256(&input),
            expected,
            "height={height} index={index}"
        );
    }
    // BoringSSL 09f7078f953362d5c5afdd224118845327b60fb4, src/crypto/blake2/
    // blake2b256_tests.txt lines66-67: input0001020304050607, digest77065d25...851621.
    assert!(vectors::ROWS
        .iter()
        .any(|&(h, i, _)| h == 0x0706_0504 && i == 0x0302_0100));
}

#[test]
fn native_rows_and_reused_probe_match_independent_literal_checksums() {
    let gpu = Gpu::open().expect("native Metal setup is required");
    let mut probe = Probe::open(&gpu).expect("native probe compilation is required");
    for rows in [1, 3, 65, 513] {
        let mut table = RTable::open(&gpu, rows).unwrap();
        assert_eq!(table.row_count(), rows);
        assert_eq!(table.bytes().unwrap().len(), rows as usize * ROW_BYTES);
        assert!(table.bytes().unwrap().iter().all(|&b| b == 0));
        assert_eq!(table.cpu_checksum(SEED, 257).unwrap(), 0);
        assert_eq!(probe.run(&mut table, SEED, 257).unwrap(), 0);
        for height in [0, 0x0706_0504] {
            for workers in [1, 2, 3] {
                table.build_parallel(height, workers).unwrap();
                for (i, row) in table.bytes().unwrap().chunks_exact(ROW_BYTES).enumerate() {
                    let mut input = [0; 8];
                    input[..4].copy_from_slice(&(i as u32).to_le_bytes());
                    input[4..].copy_from_slice(&height.to_le_bytes());
                    assert_eq!(row, blake2b256(&input));
                }
                for &(n, h, count, expected) in vectors::CHECKSUMS {
                    if n != rows || h != height {
                        continue;
                    }
                    assert_eq!(
                        table.cpu_checksum(SEED, count).unwrap(),
                        expected,
                        "CPU n={n} height={h} count={count} workers={workers}"
                    );
                    assert_eq!(
                        probe.run(&mut table, SEED, count).unwrap(),
                        expected,
                        "GPU n={n} height={h} count={count} workers={workers}"
                    );
                    if count == 1 {
                        assert_ne!(expected, 0, "built oracle must not be vacuous");
                    }
                }
            }
        }
    }
}
