//! rtable-bench — Phase 2 R-table + GPU random-read benchmark.
//!
//! Subcommands:
//!   verify [log2N]        build small table, verify GPU checksum matches CPU
//!   bench  [log2N]        build large table, measure build time + GPU random-read bandwidth
//!
//! Default log2N = 26 → N = 67,108,864 → 2 GB table. Use 22 (128 MB)
//! for fast iteration; 28 (8 GB) for full-Autolykos-scale.

use std::time::Instant;

use aruminium::Gpu;
use rtable_bench::{Probe, RTable, ROW_BYTES};
use std::error::Error;

fn parse_log2n(args: &[String], i: usize, default: u32) -> Result<u32, Box<dyn Error>> {
    let value = match args.get(i) {
        Some(value) => value.parse::<u32>()?,
        None => default,
    };
    if value > 32 {
        return Err("log2N must be at most 32".into());
    }
    Ok(value)
}

fn dimensions(log2n: u32) -> Result<(u64, u64), Box<dyn Error>> {
    if log2n > 32 {
        return Err("log2N must be at most 32".into());
    }
    let n = 1u64.checked_shl(log2n).ok_or("row count overflow")?;
    Ok((
        n,
        n.checked_mul(ROW_BYTES as u64)
            .ok_or("table byte count overflow")?,
    ))
}

fn human_bytes(b: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB"];
    let mut v = b as f64;
    let mut u = 0;
    while v >= 1024.0 && u + 1 < units.len() {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.2} {}", units[u])
}

fn cmd_verify(log2n: u32) -> Result<(), Box<dyn Error>> {
    let (n, bytes) = dimensions(log2n)?;
    println!("Verify: N = 2^{log2n} = {n} rows  ({})", human_bytes(bytes));

    let gpu = Gpu::open()?;
    println!("  device: {}", gpu.name());

    let mut table = RTable::open(&gpu, n)?;

    let threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(8);
    println!("  building on {threads} CPU threads...");

    let t0 = Instant::now();
    table.build_parallel(0, threads)?;
    let build_dt = t0.elapsed().as_secs_f64();
    let build_mbps = (bytes as f64) / build_dt / 1e6;
    println!(
        "  build: {:.3}s  ({:.1} MB/s of table writes)",
        build_dt, build_mbps
    );

    // CPU and GPU each compute a checksum over a fixed pseudorandom
    // index set. This checks byte agreement for these inputs.
    let seed: u64 = 0xA5A5_A5A5_DEAD_BEEF;
    let probe_count: u32 = 1_048_576;

    let t0 = Instant::now();
    let cpu_sum = table.cpu_checksum(seed, probe_count)?;
    let cpu_dt = t0.elapsed().as_secs_f64();
    println!(
        "  CPU checksum over {probe_count} random reads: {:016x}  ({:.3}s)",
        cpu_sum, cpu_dt
    );

    let mut probe = Probe::open(&gpu)?;

    // Reset accumulator, dispatch, read back.
    let t0 = Instant::now();
    let gpu_sum = probe.run(&mut table, seed, probe_count)?;
    let gpu_dt = t0.elapsed().as_secs_f64();
    println!(
        "  GPU checksum over {probe_count} random reads: {:016x}  ({:.3}s)",
        gpu_sum, gpu_dt
    );

    if cpu_sum == gpu_sum {
        println!("PASS: CPU and GPU checksums agree for the built table");
    } else {
        println!("FAIL: CPU and GPU checksums differ");
        return Err("GPU and CPU checksum mismatch".into());
    }
    Ok(())
}

fn cmd_bench(log2n: u32) -> Result<(), Box<dyn Error>> {
    let (n, bytes) = dimensions(log2n)?;
    println!("Bench: N = 2^{log2n} = {n} rows  ({})", human_bytes(bytes));

    let gpu = Gpu::open()?;
    let mut table = RTable::open(&gpu, n)?;
    let threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(8);

    // Build (timed)
    let t0 = Instant::now();
    table.build_parallel(0, threads)?;
    let build_dt = t0.elapsed().as_secs_f64();
    let build_mbps = (bytes as f64) / build_dt / 1e6;
    println!(
        "  build: {:.3}s  threads={threads}  ({:.1} MB/s writes through IOSurface)",
        build_dt, build_mbps
    );

    // Single correctness check before bandwidth runs.
    let seed: u64 = 0xC0FF_EE00_BEEF_DEAD;
    let check_count: u32 = 65_536;
    let cpu_check = table.cpu_checksum(seed, check_count)?;
    let mut probe = Probe::open(&gpu)?;
    let gpu_check = probe.run(&mut table, seed, check_count)?;
    if cpu_check != gpu_check {
        return Err("GPU and CPU disagree on R-table contents".into());
    }
    println!(
        "  correctness check: PASS ({check_count} reads, both = {:016x})",
        cpu_check
    );

    // Bandwidth sweep
    println!();
    println!("  GPU random-read throughput vs probe count:");
    println!(
        "    {:>14}  {:>10}  {:>14}  {:>14}",
        "probe count", "trials", "best ms", "GB/s"
    );
    let counts: &[u32] = &[
        1_048_576,   // 1M  → 32 MB of reads
        4_194_304,   // 4M  → 128 MB
        16_777_216,  // 16M → 512 MB
        67_108_864,  // 64M → 2 GiB
        268_435_456, // 256M → 8 GiB
    ];
    for &count in counts {
        // Warmup
        for _ in 0..2 {
            probe.run(&mut table, seed, count)?;
        }
        // 5 trials, take best (least system contention).
        let trials = 5;
        let mut best_ms = f64::INFINITY;
        for _ in 0..trials {
            let t0 = Instant::now();
            probe.run(&mut table, seed, count)?;
            let dt = t0.elapsed().as_secs_f64();
            let ms = dt * 1000.0;
            if ms < best_ms {
                best_ms = ms;
            }
        }
        // Each row read = 32 bytes from device memory (one u64 used,
        // but a whole cache line is fetched; reporting full row bytes
        // matches the realistic mining read pattern).
        let bytes_read = (count as u64) * (ROW_BYTES as u64);
        let gbps = (bytes_read as f64) / (best_ms / 1000.0) / 1e9;
        println!(
            "    {:>14}  {:>10}  {:>14.2}  {:>14.2}",
            count, trials, best_ms, gbps
        );
    }
    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("verify") => cmd_verify(parse_log2n(&args, 2, 22)?)?,
        Some("bench") => cmd_bench(parse_log2n(&args, 2, 26)?)?,
        None => {
            // Default: quick verify at small size, then bench at 2 GiB.
            cmd_verify(22)?;
            println!();
            cmd_bench(26)?;
        }
        Some(other) => {
            eprintln!("unknown subcommand: {other}");
            eprintln!("usage: rtable-bench [verify [log2N] | bench [log2N]]");
            return Err("invalid subcommand".into());
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("rtable-bench: {error}");
        std::process::exit(1);
    }
}
