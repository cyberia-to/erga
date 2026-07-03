//! blake-bench — GPU Blake2b-256 throughput benchmark on Apple Silicon.
//!
//! Subcommands:
//!   verify         verify all kernel variants against CPU reference
//!   bench          throughput sweep: batch sizes × threadgroup widths × variants
//!   bench-once V N W   one measurement: variant V, N hashes, threadgroup W
//!
//! Variants:
//!   v1   baseline (12-round loop, SIGMA table indexed at runtime)
//!   v2   fully unrolled rounds, SIGMA indices baked into m-word references

use std::time::Instant;

use blake_bench::{GpuBlake2b, Variant, fill_inputs, verify_zero_copy};

const VARIANTS: &[Variant] = &[
    Variant::V1Baseline,
    Variant::V2Unrolled,
    Variant::V3DualHash,
    Variant::V4DualHashFastRot,
];

fn parse_variant(s: &str) -> Option<Variant> {
    match s {
        "v1" => Some(Variant::V1Baseline),
        "v2" => Some(Variant::V2Unrolled),
        "v3" => Some(Variant::V3DualHash),
        "v4" => Some(Variant::V4DualHashFastRot),
        _ => None,
    }
}

fn print_pipeline_info(g: &GpuBlake2b) {
    println!(
        "  {:<8} thread_execution_width={}  max_threads_per_group={}",
        format!("[{:?}]", g.variant),
        g.thread_execution_width,
        g.max_threads_per_group,
    );
}

fn bench_once(g: &GpuBlake2b, count: u32, tg_width: usize) -> Option<f64> {
    if tg_width > g.max_threads_per_group {
        return None;
    }
    let (in_buf, out_buf) = g.alloc_buffers(count).ok()?;
    fill_inputs(&in_buf, count, 0xCAFE_BABE_DEAD_BEEF);

    // Warmup
    g.dispatch(&in_buf, &out_buf, count, tg_width);

    let iters = if count >= 4_000_000 {
        4
    } else if count >= 1_000_000 {
        8
    } else {
        16
    };
    let t0 = Instant::now();
    for _ in 0..iters {
        g.dispatch(&in_buf, &out_buf, count, tg_width);
    }
    let dt = t0.elapsed().as_secs_f64();
    let total_hashes = (count as u64) * (iters as u64);
    let mhs = (total_hashes as f64) / dt / 1e6;
    Some(mhs)
}

fn verify_variant(variant: Variant) -> bool {
    let g = match GpuBlake2b::open(variant) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("  open {:?} failed: {e:?}", variant);
            return false;
        }
    };
    let count: u32 = 4096;
    let (in_buf, out_buf) = g.alloc_buffers(count).unwrap();
    fill_inputs(&in_buf, count, 1234);
    g.dispatch(&in_buf, &out_buf, count, 64);
    let mismatches = verify_zero_copy(&in_buf, &out_buf, count);
    if mismatches == 0 {
        println!("  {:<24} PASS  ({count} hashes match CPU reference)", variant.label());
        true
    } else {
        println!("  {:<24} FAIL  {mismatches}/{count} mismatches", variant.label());
        false
    }
}

fn cmd_verify_all() -> bool {
    println!("Verifying GPU kernels against CPU reference:");
    let mut all_ok = true;
    for &v in VARIANTS {
        all_ok &= verify_variant(v);
    }
    all_ok
}

fn bench_stable(g: &GpuBlake2b, count: u32, tg_width: usize, trials: usize) -> Option<f64> {
    let (in_buf, out_buf) = g.alloc_buffers(count).ok()?;
    fill_inputs(&in_buf, count, 0xCAFE_BABE_DEAD_BEEF);

    // 3 warmup dispatches
    for _ in 0..3 {
        g.dispatch(&in_buf, &out_buf, count, tg_width);
    }

    let mut best = 0.0f64;
    for _ in 0..trials {
        let iters = 8;
        let t0 = Instant::now();
        for _ in 0..iters {
            g.dispatch(&in_buf, &out_buf, count, tg_width);
        }
        let dt = t0.elapsed().as_secs_f64();
        let total = (count as u64) * (iters as u64);
        let mhs = (total as f64) / dt / 1e6;
        if mhs > best {
            best = mhs;
        }
    }
    Some(best)
}

fn cmd_stable() {
    // Use the largest reasonable batch and a known-good threadgroup width.
    let count: u32 = 16_777_216;
    let widths: &[usize] = &[32, 64, 128, 256];
    let trials = 8;
    println!();
    println!(
        "Stable measurement: count={count}, best of {trials} trials per (variant, tg_width)"
    );
    println!();
    print!("{:<32}", "variant");
    for &w in widths {
        print!("{:>10}", format!("tg={w}"));
    }
    println!("{:>10}", "peak");
    for &variant in VARIANTS {
        let g = match GpuBlake2b::open(variant) {
            Ok(g) => g,
            Err(_) => continue,
        };
        print!("{:<32}", variant.label());
        let mut peak = 0.0f64;
        for &w in widths {
            match bench_stable(&g, count, w, trials) {
                Some(mhs) => {
                    print!("{:>10.1}", mhs);
                    if mhs > peak {
                        peak = mhs;
                    }
                }
                None => print!("{:>10}", "-"),
            }
        }
        println!("{:>10.1}", peak);
    }
    println!();
}

fn cmd_bench_all() {
    println!();
    println!("Throughput sweep (MH/s, higher is better):");
    println!("  rows = batch size  cols = threadgroup width");
    println!();

    let batches: &[u32] = &[262_144, 1_048_576, 4_194_304, 16_777_216, 67_108_864];
    let widths: &[usize] = &[32, 64, 128, 256, 512, 1024];

    for &variant in VARIANTS {
        let g = match GpuBlake2b::open(variant) {
            Ok(g) => g,
            Err(e) => {
                println!("--- {} --- open failed: {e:?}", variant.label());
                continue;
            }
        };
        println!("--- {} ---", variant.label());
        print_pipeline_info(&g);
        print!("{:>12}", "count\\tg_w");
        for &w in widths {
            print!("{:>10}", w);
        }
        println!();
        for &count in batches {
            print!("{:>12}", count);
            for &w in widths {
                match bench_once(&g, count, w) {
                    Some(mhs) => print!("{:>10.1}", mhs),
                    None => print!("{:>10}", "-"),
                }
            }
            println!();
        }
        println!();
    }

    // Summary: peak achieved per variant
    println!("Peak per variant:");
    for &variant in VARIANTS {
        if let Ok(g) = GpuBlake2b::open(variant) {
            let mut best = 0.0f64;
            let mut best_at = (0u32, 0usize);
            for &count in batches {
                for &w in widths {
                    if let Some(mhs) = bench_once(&g, count, w) {
                        if mhs > best {
                            best = mhs;
                            best_at = (count, w);
                        }
                    }
                }
            }
            println!(
                "  {:<40} peak {:.1} MH/s  @ count={} tg={}",
                variant.label(),
                best,
                best_at.0,
                best_at.1
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("verify") => {
            if !cmd_verify_all() {
                std::process::exit(1);
            }
        }
        Some("bench-once") => {
            let variant = args.get(2).and_then(|s| parse_variant(s)).unwrap_or(Variant::V2Unrolled);
            let count: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1_048_576);
            let w: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(64);
            let g = GpuBlake2b::open(variant).expect("open");
            print_pipeline_info(&g);
            match bench_once(&g, count, w) {
                Some(mhs) => println!("{:?} count={count} tg={w} -> {mhs:.2} MH/s", variant),
                None => println!("unsupported tg width"),
            }
        }
        Some("bench") | None => {
            if !cmd_verify_all() {
                eprintln!("verification failed — not running benchmark");
                std::process::exit(1);
            }
            cmd_bench_all();
        }
        Some("stable") => {
            if !cmd_verify_all() {
                eprintln!("verification failed — not running benchmark");
                std::process::exit(1);
            }
            cmd_stable();
        }
        Some(other) => {
            eprintln!("unknown subcommand: {other}");
            eprintln!("usage: blake-bench [verify|bench|bench-once {{v1|v2}} N W]");
            std::process::exit(1);
        }
    }
}
