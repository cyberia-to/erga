//! erga — one command.
//!
//! With no arguments it opens the window; with any, it does that instead.
//! One binary, which is also why the window can spawn the miner: it spawns
//! *itself*, and the bundle ships a single file rather than a pair that must
//! be kept in step.

mod face;

use aruminium::Gpu;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argv: Vec<String> = std::env::args().collect();
    match argv.get(1).map(|s| s.as_str()) {
        None => {
            // Installed as an app, `erga` should also be a command. Costs
            // nothing, asks nothing, and only happens from inside a bundle.
            face::link_quietly();
            erga_app::run().map_err(|e| e.into())
        }
        Some("mine") => {
            let (host, port) = erga_app::chosen_pool();
            let host = argv.get(2).cloned().unwrap_or(host);
            let port = argv.get(3).and_then(|s| s.parse().ok()).unwrap_or(port);
            let address = match argv.get(4) {
                Some(a) => a.clone(),
                None => match erga_app::payout_address() {
                    Ok(a) => {
                        println!("mining to: {a}");
                        a
                    }
                    Err(e) => {
                        eprintln!("no address given and no wallet available: {e}");
                        std::process::exit(1);
                    }
                },
            };
            println!("pool: {host}:{port}");
            // In-process here, which the window deliberately does not do: with
            // no window there is no second graphics API to pair with Metal.
            erga_miner::cli::mine(host, port, address, argv.iter().any(|a| a == "--machine"));
            Ok(())
        }
        Some("status") => {
            face::status();
            Ok(())
        }
        Some("link") => {
            face::link();
            Ok(())
        }
        Some("difftest") => {
            erga_miner::difftest::run();
            Ok(())
        }
        Some("buildbench") => {
            let height: u32 = argv.get(2).and_then(|s| s.parse().ok()).unwrap_or(1_862_800);
            let n = autolykos::calc_big_n(2, height);
            let m = autolykos::big_m();
            let gpu = Gpu::open().expect("open gpu");
            println!(
                "device: {}  n: {n}  table: {:.2} GiB",
                gpu.name(),
                n as f64 * 32.0 / (1u64 << 30) as f64
            );
            let t0 = std::time::Instant::now();
            match erga_miner::gpu::ScanMiner::new_gpu_built(gpu, n, height, &m, &|_| {}) {
                Ok(_) => println!("build: {:.2} s", t0.elapsed().as_secs_f64()),
                Err(e) => println!("build FAILED: {e}"),
            }
            Ok(())
        }
        Some("help" | "--help" | "-h") => {
            face::help();
            Ok(())
        }
        Some(other) => {
            eprintln!("unknown command `{other}`");
            face::help();
            std::process::exit(1);
        }
    }
}
