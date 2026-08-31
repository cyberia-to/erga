//! erga-miner — the GPU-exact Autolykos v2 miner.
//!
//!   erga-miner difftest        # gate: GPU hit == chain-verified reference
//!
//! The differential test is the safety interlock: the GPU kernel must
//! reproduce `autolykos::pow_hit` byte-for-byte before it is trusted to
//! search for real shares. Only after it passes does mining make sense.

use aruminium::Gpu;
use erga_miner::gpu;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()).unwrap_or("difftest") {
        "difftest" => difftest(),
        // Time one full table build — the cost paid at every block change.
        "buildbench" => {
            let height: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1_862_800);
            let n = autolykos::calc_big_n(2, height);
            let m = autolykos::big_m();
            let gpu = Gpu::open().expect("open gpu");
            println!("device: {}  n: {n}  table: {:.2} GiB", gpu.name(), n as f64 * 32.0 / (1u64 << 30) as f64);
            let t0 = std::time::Instant::now();
            match erga_miner::gpu::ScanMiner::new_gpu_built(gpu, n, height, &m) {
                Ok(_) => println!("build: {:.2} s", t0.elapsed().as_secs_f64()),
                Err(e) => println!("build FAILED: {e}"),
            }
        }
        "mine" => {
            let host = args.get(2).cloned().unwrap_or_else(|| "ergo.herominers.com".into());
            let port: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1180);
            let address = args
                .get(4)
                .cloned()
                .unwrap_or_else(|| "9fRAWhdxEsTcdb8PhGNrZfwqa65zfkuYHAMmkQLcic1gdLSV5vA".into());
            let machine = args.iter().any(|a| a == "--machine");
            erga_miner::cli::mine(host, port, address, machine);
        }
        other => {
            eprintln!("unknown command: {other}");
            std::process::exit(1);
        }
    }
}

/// Build a small table, compute the hit for a batch of nonces on the GPU,
/// and compare each to the CPU reference. All must match.
fn difftest() {
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
