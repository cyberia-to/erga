//! The safety interlock: the GPU kernel must reproduce the chain-verified
//! CPU reference byte for byte before it is trusted to search for shares.

use aruminium::Gpu;
use crate::gpu;

/// Build a small table, compute the hit for a batch of nonces on the GPU,
/// and compare each to the CPU reference. All must match.
pub fn run() {
    let height = 614400u32;
    let n = 4096u32; // small table for the test; structure is N-agnostic
    let m = autolykos::big_m();
    let h = autolykos::height_bytes(height);

    // exact table in GPU limb layout
    let rows = gpu::build_table_limbs(n, &h, &m);

    // arbitrary 32-byte header prehash
    let msg = blake_bench::reference::blake2b256(b"erga difftest header");

    let count = 512u32;
    let gpu_dev = Gpu::open().expect("open gpu");
    println!("device: {}", gpu_dev.name());
    let hits = gpu::gpu_hits(&gpu_dev, &rows, n, &msg, 0, count);

    let mut mismatches = 0;
    for k in 0..count as u64 {
        let nonce = k.to_be_bytes();
        let want = autolykos::pow_hit(&msg, &nonce, &h, n, &m);
        let want_be = left_pad_32(&want.to_bytes_be());
        let got = &hits[(k as usize) * 32..(k as usize) * 32 + 32];
        if got != &want_be[..] {
            if mismatches < 3 {
                eprintln!("MISMATCH nonce {k}");
                eprintln!("  gpu {}", hex(got));
                eprintln!("  cpu {}", hex(&want_be));
            }
            mismatches += 1;
        }
    }
    if mismatches == 0 {
        println!("difftest OK — {count} nonces, GPU hit == chain-verified reference, byte-exact.");
    } else {
        eprintln!("difftest FAILED — {mismatches}/{count} mismatched.");
        std::process::exit(1);
    }
}

fn left_pad_32(b: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[32 - b.len()..].copy_from_slice(b);
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
