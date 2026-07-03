//! mine-bench — Phase 3 integrated Autolykos v2 mining throughput.

use std::time::Instant;

use aruminium::Gpu;
use mine_bench::{
    GpuMiner, GpuMinerV9, MineVariant, cpu_accumulate, cpu_mine_for, gpu_mine_for, mine_one,
    read_acc32,
    texture_ffi::{create_2d_rgba32u_texture, upload_rtable},
    zero_acc32,
};
use rtable_bench::RTable;

const VARIANTS: &[MineVariant] = &[
    MineVariant::V1Single,
    MineVariant::V8DualTable,
    MineVariant::V6NoBlake,
    MineVariant::V7NoLoads,
];

fn parse_log2n(args: &[String], i: usize, default: u32) -> u32 {
    args.get(i)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(default)
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

const M_TEST: [u8; 32] = [
    0xa5, 0xa5, 0xa5, 0xa5, 0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe, 0x12, 0x34, 0x56, 0x78,
    0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
];

fn build_table(gpu: &Gpu, log2n: u32) -> RTable {
    let n = 1u64 << log2n;
    let table = RTable::open(gpu, n).expect("alloc R-table");
    let threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(8);
    let t0 = Instant::now();
    table.build_parallel(0, threads);
    let dt = t0.elapsed().as_secs_f64();
    let bytes = n * rtable_bench::ROW_BYTES as u64;
    eprintln!(
        "  R-table: N=2^{log2n} ({}), build {:.3}s ({:.1} MB/s)",
        human_bytes(bytes),
        dt,
        bytes as f64 / dt / 1e6
    );
    table
}

fn hex32(b: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for x in b {
        s.push_str(&format!("{:02x}", x));
    }
    s
}

fn verify_variant(variant: MineVariant, table: &RTable, table_b: Option<&RTable>) -> bool {
    if !variant.is_real() {
        println!("  {:<38} SKIP  (diagnostic only)", variant.label());
        return true;
    }
    let count: u32 = 64;
    let cpu_acc = cpu_accumulate(&M_TEST, 0, count, table);
    let gpu = Gpu::open().expect("open gpu");
    let miner = GpuMiner::open(gpu, variant).expect("compile");
    let acc_buf = miner.gpu.buffer(32).expect("acc buf");
    zero_acc32(&acc_buf);
    miner.run_dual(table, table_b, &acc_buf, &M_TEST, 0, count, 64);
    let gpu_acc = read_acc32(&acc_buf);
    let ok = cpu_acc == gpu_acc;
    if ok {
        println!(
            "  {:<38} PASS  (XOR over {count} nonces matches)",
            variant.label()
        );
    } else {
        println!("  {:<38} FAIL", variant.label());
        eprintln!("    CPU: {}", hex32(&cpu_acc));
        eprintln!("    GPU: {}", hex32(&gpu_acc));
    }
    ok
}

fn cmd_verify(log2n: u32) {
    let gpu = Gpu::open().expect("open gpu");
    eprintln!("device: {}", gpu.name());
    let table = build_table(&gpu, log2n);
    let table_b = if VARIANTS.iter().any(|v| v.needs_dual_table()) {
        eprintln!("  Building second R-table copy for V8...");
        Some(build_table(&gpu, log2n))
    } else {
        None
    };
    println!("Verifying mining kernels against CPU reference:");
    let nonce_base: u64 = 0;
    let r_bytes = table.block.as_bytes();
    let d0 = mine_one(&M_TEST, nonce_base, r_bytes, table.n);
    eprintln!("  CPU d[nonce=0] = {}", hex32(&d0));
    let mut all_ok = true;
    for &v in VARIANTS {
        all_ok &= verify_variant(v, &table, table_b.as_ref());
    }
    if !all_ok {
        std::process::exit(1);
    }
}

fn bench_once(
    miner: &GpuMiner,
    table: &RTable,
    table_b: Option<&RTable>,
    acc_buf: &aruminium::Buffer,
    count: u32,
    tg: usize,
) -> f64 {
    for _ in 0..2 {
        zero_acc32(acc_buf);
        miner.run_dual(table, table_b, acc_buf, &M_TEST, 0, count, tg);
    }
    let mut best_mhs = 0.0f64;
    for _ in 0..5 {
        zero_acc32(acc_buf);
        let t0 = Instant::now();
        miner.run_dual(table, table_b, acc_buf, &M_TEST, 0, count, tg);
        let dt = t0.elapsed().as_secs_f64();
        let mhs = (count as f64) / dt / 1e6;
        if mhs > best_mhs {
            best_mhs = mhs;
        }
    }
    best_mhs
}

fn cmd_bench(log2n: u32) {
    let gpu = Gpu::open().expect("open gpu");
    eprintln!("device: {}", gpu.name());
    let table = build_table(&gpu, log2n);
    let table_b = if VARIANTS.iter().any(|v| v.needs_dual_table()) {
        eprintln!("  Building second R-table copy for V8...");
        Some(build_table(&gpu, log2n))
    } else {
        None
    };

    let cpu_check = cpu_accumulate(&M_TEST, 0, 64, &table);
    println!();
    println!("Correctness check (64 nonces, must match CPU reference):");
    for &variant in VARIANTS {
        if !variant.is_real() {
            println!("  {:<38} SKIP  (diagnostic only)", variant.label());
            continue;
        }
        let gpu = Gpu::open().expect("open");
        let miner = GpuMiner::open(gpu, variant).expect("compile");
        let acc_buf = miner.gpu.buffer(32).expect("acc buf");
        zero_acc32(&acc_buf);
        miner.run_dual(&table, table_b.as_ref(), &acc_buf, &M_TEST, 0, 64, 64);
        let gpu_acc = read_acc32(&acc_buf);
        assert_eq!(
            cpu_check, gpu_acc,
            "{} produced wrong output — kernel is broken",
            variant.label()
        );
        println!("  {:<38} PASS", variant.label());
    }

    println!();
    println!("Autolykos v2 mining throughput on M4 Max (best of 5 trials):");
    let counts: &[u32] = &[262_144, 1_048_576, 4_194_304, 16_777_216];
    let tgs: &[usize] = &[32, 64, 128, 256];

    for &variant in VARIANTS {
        let gpu = Gpu::open().expect("open");
        let miner = GpuMiner::open(gpu, variant).expect("compile");
        let acc_buf = miner.gpu.buffer(32).expect("acc buf");
        println!("--- {} ---", variant.label());
        print!("{:>14}", "count\\tg");
        for &tg in tgs {
            print!("{:>12}", tg);
        }
        println!();
        let mut best_overall = 0.0f64;
        let mut best_at = (0u32, 0usize);
        for &count in counts {
            print!("{:>14}", count);
            for &tg in tgs {
                let mhs = bench_once(&miner, &table, table_b.as_ref(), &acc_buf, count, tg);
                print!("{:>12.2}", mhs);
                if mhs > best_overall {
                    best_overall = mhs;
                    best_at = (count, tg);
                }
            }
            println!();
        }
        println!(
            "  peak {:.2} MH/s  @ count={} tg={}",
            best_overall, best_at.0, best_at.1
        );
        println!();
    }
}

fn cmd_hybrid(log2n: u32, duration_secs: f64) {
    let gpu = Gpu::open().expect("open gpu");
    eprintln!("device: {}", gpu.name());
    let table = build_table(&gpu, log2n);

    let miner = GpuMiner::open(gpu, MineVariant::V1Single).expect("v1 miner");
    let acc_buf = miner.gpu.buffer(32).expect("acc");

    let cpu_threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(16);

    let batch: u32 = 4_194_304; // 4M nonces per GPU dispatch
    let tg: usize = 64;

    println!();
    println!("=== A1: Hybrid CPU+GPU mining ===");
    println!("duration each phase: {:.1}s", duration_secs);
    println!();

    // Phase A: CPU only
    eprintln!("Phase A: CPU only ({cpu_threads} threads)...");
    let t0 = Instant::now();
    let cpu_only = cpu_mine_for(&table, &M_TEST, 0, duration_secs, cpu_threads);
    let cpu_dt = t0.elapsed().as_secs_f64();
    let cpu_only_mhs = (cpu_only as f64) / cpu_dt / 1e6;
    println!(
        "  CPU only: {:>10.2} MH/s   ({} nonces in {:.2}s)",
        cpu_only_mhs, cpu_only, cpu_dt
    );

    // Phase B: GPU only
    eprintln!("Phase B: GPU only (V1 kernel)...");
    zero_acc32(&acc_buf);
    let t0 = Instant::now();
    let gpu_only = gpu_mine_for(&miner, &table, &acc_buf, &M_TEST, 0, duration_secs, batch, tg);
    let gpu_dt = t0.elapsed().as_secs_f64();
    let gpu_only_mhs = (gpu_only as f64) / gpu_dt / 1e6;
    println!(
        "  GPU only: {:>10.2} MH/s   ({} nonces in {:.2}s)",
        gpu_only_mhs, gpu_only, gpu_dt
    );

    // Phase C: concurrent CPU+GPU
    eprintln!("Phase C: CPU+GPU concurrent...");
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::{Duration, Instant as TI};

    zero_acc32(&acc_buf);
    let stop = AtomicBool::new(false);
    let cpu_done = AtomicU64::new(0);
    let gpu_done = AtomicU64::new(0);
    let r_bytes_ptr_len = table.block.as_bytes();
    let n = table.n;

    let t0 = TI::now();
    let cpu_done_ref = &cpu_done;
    let stop_ref = &stop;
    std::thread::scope(|s| {
        // CPU pool
        for tid in 0..cpu_threads {
            s.spawn(move || {
                let mut local: u64 = 0;
                let mut nonce: u64 = tid as u64;
                let stride: u64 = cpu_threads as u64;
                let mut next_check = 1024u64;
                loop {
                    let _d = mine_one(&M_TEST, nonce, r_bytes_ptr_len, n);
                    nonce = nonce.wrapping_add(stride);
                    local += 1;
                    if local == next_check {
                        if stop_ref.load(Ordering::Relaxed) {
                            break;
                        }
                        next_check += 1024;
                    }
                }
                cpu_done_ref.fetch_add(local, Ordering::Relaxed);
            });
        }

        // GPU on the scope's main thread — keep dispatching until duration elapses.
        let deadline = TI::now() + Duration::from_secs_f64(duration_secs);
        // Use a much higher nonce base for GPU so CPU and GPU don't collide
        // on the same nonce ranges (irrelevant for throughput but cleaner).
        let mut gpu_nonce_base: u64 = 1 << 40;
        let mut gpu_total: u64 = 0;
        while TI::now() < deadline {
            miner.run(&table, &acc_buf, &M_TEST, gpu_nonce_base, batch, tg);
            gpu_nonce_base = gpu_nonce_base.wrapping_add(batch as u64);
            gpu_total += batch as u64;
        }
        gpu_done.store(gpu_total, Ordering::Relaxed);
        // Tell CPU to stop. CPU threads will check on next 1024-boundary.
        stop.store(true, Ordering::Relaxed);
    });
    let dt = t0.elapsed().as_secs_f64();
    let cpu_n = cpu_done.load(Ordering::Relaxed);
    let gpu_n = gpu_done.load(Ordering::Relaxed);
    let cpu_mhs = (cpu_n as f64) / dt / 1e6;
    let gpu_mhs = (gpu_n as f64) / dt / 1e6;
    let total_mhs = ((cpu_n + gpu_n) as f64) / dt / 1e6;
    println!(
        "  Concurrent CPU: {:>8.2} MH/s   ({} nonces)",
        cpu_mhs, cpu_n
    );
    println!(
        "  Concurrent GPU: {:>8.2} MH/s   ({} nonces)",
        gpu_mhs, gpu_n
    );
    println!(
        "  Concurrent TOT: {:>8.2} MH/s",
        total_mhs
    );

    println!();
    println!("=== Summary ===");
    println!(
        "  CPU only:        {:>10.2} MH/s",
        cpu_only_mhs
    );
    println!(
        "  GPU only:        {:>10.2} MH/s",
        gpu_only_mhs
    );
    println!(
        "  CPU+GPU sum:     {:>10.2} MH/s (additive theoretical)",
        cpu_only_mhs + gpu_only_mhs
    );
    println!(
        "  CPU+GPU measured:{:>10.2} MH/s (actual concurrent)",
        total_mhs
    );
    let interference = cpu_only_mhs + gpu_only_mhs - total_mhs;
    let interference_pct = 100.0 * interference / (cpu_only_mhs + gpu_only_mhs);
    println!(
        "  Interference loss: {:>6.2} MH/s ({:.1}%)",
        interference, interference_pct
    );
}

fn cmd_v1_profile(log2n: u32, total_secs: f64, window_secs: f64) {
    let gpu = Gpu::open().expect("open gpu");
    eprintln!("device: {}", gpu.name());
    let table = build_table(&gpu, log2n);
    let miner = GpuMiner::open(gpu, MineVariant::V1Single).expect("v1");
    let acc_buf = miner.gpu.buffer(32).expect("acc");
    zero_acc32(&acc_buf);
    let batch: u32 = 4_194_304;
    let tg: usize = 64;
    println!("Profile: V1 mining for {:.0}s, window={:.0}s", total_secs, window_secs);
    println!("Each window prints sustained MH/s over that window.");
    println!();
    println!("  {:>8} {:>10} {:>10}", "elapsed", "window_MH/s", "cumul_MH/s");
    let t_start = Instant::now();
    let mut window_start = t_start;
    let mut window_nonces: u64 = 0;
    let mut total_nonces: u64 = 0;
    let mut nonce_base: u64 = 0;
    let deadline = t_start + std::time::Duration::from_secs_f64(total_secs);
    while Instant::now() < deadline {
        miner.run(&table, &acc_buf, &M_TEST, nonce_base, batch, tg);
        nonce_base = nonce_base.wrapping_add(batch as u64);
        window_nonces += batch as u64;
        total_nonces += batch as u64;
        let wdt = window_start.elapsed().as_secs_f64();
        if wdt >= window_secs {
            let window_mhs = (window_nonces as f64) / wdt / 1e6;
            let tdt = t_start.elapsed().as_secs_f64();
            let cumul_mhs = (total_nonces as f64) / tdt / 1e6;
            println!(
                "  {:>7.1}s {:>10.2} {:>10.2}",
                tdt, window_mhs, cumul_mhs
            );
            window_start = Instant::now();
            window_nonces = 0;
        }
    }
    let total_dt = t_start.elapsed().as_secs_f64();
    let total_mhs = (total_nonces as f64) / total_dt / 1e6;
    println!();
    println!("FINAL: {:.2} MH/s sustained over {:.1}s", total_mhs, total_dt);
}

fn cmd_v1_sustained(log2n: u32, duration_secs: f64) {
    let gpu = Gpu::open().expect("open gpu");
    eprintln!("device: {}", gpu.name());
    let table = build_table(&gpu, log2n);
    let miner = GpuMiner::open(gpu, MineVariant::V1Single).expect("v1");
    let acc_buf = miner.gpu.buffer(32).expect("acc");
    zero_acc32(&acc_buf);
    let batch: u32 = 4_194_304;
    let tg: usize = 64;
    eprintln!("Sustained V1 mining for {:.1}s...", duration_secs);
    let t0 = Instant::now();
    let nonces = gpu_mine_for(&miner, &table, &acc_buf, &M_TEST, 0, duration_secs, batch, tg);
    let dt = t0.elapsed().as_secs_f64();
    let mhs = (nonces as f64) / dt / 1e6;
    println!("V1 sustained: {:.2} MH/s ({} nonces in {:.2}s)", mhs, nonces, dt);
}

fn cmd_v9(log2n: u32) {
    let gpu = Gpu::open().expect("open gpu");
    eprintln!("device: {}", gpu.name());
    let table = build_table(&gpu, log2n);

    // Texture dims: width = 16384 texels (16 B/texel = 256 KB/texture-row).
    // height = (2 * N) / 16384, total bytes = N * 32 = R-table size.
    let n = table.n as usize;
    let bytes = n * 32;
    let width = 16384usize;
    let bytes_per_row = width * 16;
    if bytes % bytes_per_row != 0 {
        eprintln!(
            "skip V9: N*32={} not divisible by bytes_per_row={}",
            bytes, bytes_per_row
        );
        return;
    }
    let height = bytes / bytes_per_row;
    eprintln!("V9 texture: {} × {} (RGBA32Uint, {} MiB)", width, height, bytes / 1024 / 1024);

    let texture = create_2d_rgba32u_texture(&gpu, width, height).expect("texture");

    // Upload R-table bytes into the texture (one-time copy — not zero-copy
    // for this experiment; if V9 wins we'll do IOSurface-backed variant).
    let t0 = std::time::Instant::now();
    let src_ptr = table.block.as_bytes().as_ptr();
    upload_rtable(&texture, src_ptr, bytes, width, height);
    eprintln!("  R upload: {:.3}s", t0.elapsed().as_secs_f64());

    // Correctness: compare V1 GPU result vs V9 GPU result at small count.
    let count: u32 = 64;
    let v1_miner = GpuMiner::open(Gpu::open().unwrap(), MineVariant::V1Single).expect("v1");
    let v1_acc = v1_miner.gpu.buffer(32).expect("acc");
    zero_acc32(&v1_acc);
    v1_miner.run(&table, &v1_acc, &M_TEST, 0, count, 64);
    let v1_result = read_acc32(&v1_acc);

    let v9 = GpuMinerV9::open(gpu).expect("v9 open");
    let v9_acc = v9.gpu.buffer(32).expect("acc");
    zero_acc32(&v9_acc);
    v9.run(&texture, &v9_acc, &M_TEST, table.n, 0, count, 64);
    let v9_result = read_acc32(&v9_acc);

    if v1_result == v9_result {
        println!("V9 correctness: PASS (matches V1 XOR over {count} nonces)");
    } else {
        println!("V9 correctness: FAIL");
        eprintln!("  V1: {}", hex32(&v1_result));
        eprintln!("  V9: {}", hex32(&v9_result));
        return;
    }

    println!();
    println!("V9 vs V1 throughput (M4 Max, best of 5 trials):");
    let counts: &[u32] = &[1_048_576, 4_194_304, 16_777_216];
    let tgs: &[usize] = &[32, 64, 128];

    println!("  {:<10} {:>10}  {:>14}  {:>14}  {:>14}", "kernel", "count", "tg=32", "tg=64", "tg=128");
    for &count in counts {
        // V1
        let mut row_v1 = format!("  {:<10} {:>10}", "V1 buffer", count);
        for &tg in tgs {
            let mut best = 0.0f64;
            for _ in 0..5 {
                zero_acc32(&v1_acc);
                let t = std::time::Instant::now();
                v1_miner.run(&table, &v1_acc, &M_TEST, 0, count, tg);
                let dt = t.elapsed().as_secs_f64();
                let mhs = count as f64 / dt / 1e6;
                if mhs > best { best = mhs; }
            }
            row_v1.push_str(&format!("  {:>12.2}", best));
        }
        println!("{row_v1}");

        // V9
        let mut row_v9 = format!("  {:<10} {:>10}", "V9 texture", count);
        for &tg in tgs {
            let mut best = 0.0f64;
            for _ in 0..5 {
                zero_acc32(&v9_acc);
                let t = std::time::Instant::now();
                v9.run(&texture, &v9_acc, &M_TEST, table.n, 0, count, tg);
                let dt = t.elapsed().as_secs_f64();
                let mhs = count as f64 / dt / 1e6;
                if mhs > best { best = mhs; }
            }
            row_v9.push_str(&format!("  {:>12.2}", best));
        }
        println!("{row_v9}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("verify") => cmd_verify(parse_log2n(&args, 2, 20)),
        Some("bench") => cmd_bench(parse_log2n(&args, 2, 26)),
        Some("v9") => cmd_v9(parse_log2n(&args, 2, 26)),
        Some("v1-sustained") => {
            let log2n = parse_log2n(&args, 2, 26);
            let dur: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(60.0);
            cmd_v1_sustained(log2n, dur);
        }
        Some("v1-profile") => {
            let log2n = parse_log2n(&args, 2, 26);
            let total: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(300.0);
            let window: f64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(5.0);
            cmd_v1_profile(log2n, total, window);
        }
        Some("hybrid") => {
            let log2n = parse_log2n(&args, 2, 26);
            let dur: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(5.0);
            cmd_hybrid(log2n, dur);
        }
        None => {
            cmd_verify(20);
            println!();
            cmd_bench(26);
        }
        Some(other) => {
            eprintln!("unknown subcommand: {other}");
            eprintln!("usage: mine-bench [verify [log2N] | bench [log2N] | v9 [log2N]]");
            std::process::exit(1);
        }
    }
}
