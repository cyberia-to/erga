//! Live mining: connect a pool, build the epoch table, scan on the GPU,
//! re-verify every candidate on the CPU (chain-verified reference) before
//! submitting. Nothing invalid is ever sent.

use crate::gpu::ScanMiner;
use aruminium::Gpu;
use erga_pool::stratum::{Job, PoolEvent, Stratum};
use num_bigint::BigUint;

pub fn run(host: &str, port: u16, address: &str) {
    let mut s = match Stratum::connect(host, port, address, "erga") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("connect failed: {e}");
            std::process::exit(1);
        }
    };
    // extranonce1 prefix — top bytes of every nonce we mine
    let en1 = s.extranonce1.clone();
    let en1_bits = en1.len() as u32 * 8;
    let search_bits = 64 - en1_bits;
    let en1_val: u64 = en1.iter().fold(0u64, |a, &b| (a << 8) | b as u64);
    let en1_prefix = if en1_bits == 0 { 0 } else { en1_val << search_bits };
    let tail_mask: u64 = if search_bits >= 64 { u64::MAX } else { (1u64 << search_bits) - 1 };
    println!("mining → {address} @ {host}:{port}  (extranonce1={})", hex(&en1));

    let m = autolykos::big_m();
    let mut miner: Option<ScanMiner> = None;
    let mut cur_height = 0u32;
    let mut job: Option<Job> = None;
    let mut cursor: u64 = 0; // low-48-bit search cursor
    let batch: u32 = 8_388_608; // 8M nonces per dispatch
    let mut hashed: u64 = 0;
    let t_start = std::time::Instant::now();

    loop {
        // absorb pool messages
        while let Ok(ev) = s.events.try_recv() {
            match ev {
                PoolEvent::Job(j) => {
                    if j.height != cur_height {
                        cur_height = j.height;
                        let n = autolykos::calc_big_n(j.version, j.height);
                        let gib = n as f64 * 32.0 / (1u64 << 30) as f64;
                        println!("new epoch: height {} → N={n} ({gib:.1} GiB table). building on GPU…", j.height);
                        let t0 = std::time::Instant::now();
                        match ScanMiner::new_gpu_built(Gpu::open().expect("gpu"), n, j.height, &m) {
                            Ok(mn) => {
                                println!("  table built + verified in {:.1}s", t0.elapsed().as_secs_f64());
                                miner = Some(mn);
                            }
                            Err(e) => {
                                eprintln!("  GPU table build failed: {e}");
                                std::process::exit(1);
                            }
                        }
                        cursor = 0;
                    }
                    // keep the cursor advancing across jobs (each msg is a
                    // fresh lottery); resetting per job pinned it near zero
                    job = Some(j);
                }
                PoolEvent::Difficulty(d) => println!("difficulty → {d}"),
                PoolEvent::SubmitResult { accepted, error, .. } => {
                    if accepted {
                        println!("\n★ SHARE ACCEPTED — it earns. (elapsed {:.0}s, {} MH hashed)",
                            t_start.elapsed().as_secs_f64(), hashed / 1_000_000);
                    } else {
                        println!("share rejected: {}", error.unwrap_or_default());
                    }
                }
                PoolEvent::Closed => {
                    eprintln!("pool connection closed");
                    return;
                }
            }
        }

        let (Some(mn), Some(j)) = (&miner, &job) else {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        };
        let mut msg = [0u8; 32];
        if j.msg.len() == 32 {
            msg.copy_from_slice(&j.msg);
        }
        let target = left_pad_32(&j.target_b.to_bytes_be());

        let nonce_base = en1_prefix | (cursor & tail_mask);
        if let Some(nonce) = mn.scan(&msg, &target, nonce_base, batch) {
            // CPU re-verify with the chain-verified reference before trusting
            let nb = nonce.to_be_bytes();
            let hit = autolykos::pow_hit(&msg, &nb, &j.height.to_be_bytes(), mn.n, &m);
            if hit < j.target_b {
                let nonce_hex = hex(&nb); // full nonce, keeps the en1 prefix
                let en2_hex = hex(&nb[en1.len()..]); // searched suffix
                println!("share found: nonce {nonce_hex} (hit verified, submitting)");
                if let Err(e) = s.submit(address, "erga", &j.job_id, &en2_hex, &nonce_hex, "") {
                    eprintln!("submit io error: {e}");
                }
            } else {
                eprintln!("candidate failed CPU re-check (kernel/target drift) — not submitting");
            }
        }
        cursor = cursor.wrapping_add(batch as u64);
        hashed += batch as u64;

        if hashed % (batch as u64 * 20) == 0 {
            let mhs = hashed as f64 / t_start.elapsed().as_secs_f64() / 1e6;
            println!("  {:.1} MH/s, {} MH hashed", mhs, hashed / 1_000_000);
        }
    }
}

fn left_pad_32(b: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let n = b.len().min(32);
    out[32 - n..].copy_from_slice(&b[b.len() - n..]);
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[allow(unused_imports)]
use num_traits::Zero as _;
#[allow(dead_code)]
fn _uses(_: BigUint) {}
