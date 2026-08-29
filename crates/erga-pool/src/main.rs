//! erga-pool — Ergo stratum client.
//!
//!   erga-pool <host> <port> <address>   # connect, parse and print a live job
//!
//! This verifies the stratum client against a real pool: subscribe,
//! authorize, and parse a `mining.notify` into a `Job` the chain-verified
//! `autolykos` engine can mine. Submitting shares at pool difficulty needs
//! the GPU-exact kernel (finds a share in seconds; CPU would take ~20 min),
//! which is the next build — this crate is the connection + protocol half.

mod stratum;

use stratum::{PoolEvent, Stratum};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let host = args.get(1).cloned().unwrap_or_else(|| "ergo.herominers.com".into());
    let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1180);
    let address = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "9fRAWhdxEsTcdb8PhGNrZfwqa65zfkuYHAMmkQLcic1gdLSV5vA".into());

    println!("connecting {host}:{port} …");
    let s = match Stratum::connect(&host, port, &address, "erga") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("connect failed: {e}");
            std::process::exit(1);
        }
    };
    println!("extranonce1 = {}", hex(&s.extranonce1));

    let mut diff = 1.0;
    for ev in s.events.iter() {
        match ev {
            PoolEvent::Difficulty(d) => {
                diff = d;
                println!("set_difficulty {d}");
            }
            PoolEvent::Job(j) => {
                let n = autolykos::calc_big_n(j.version, j.height);
                println!("JOB {}", j.job_id);
                println!("  height  {}", j.height);
                println!("  msg     {}", hex(&j.msg));
                println!("  version {}", j.version);
                println!("  N       {n}");
                println!("  target  {}", j.target_b);
                println!("  diff    {diff}");
                println!("\nstratum client OK — parsed a live job into the mining engine.");
                return;
            }
            PoolEvent::Closed => {
                eprintln!("connection closed before a job arrived");
                return;
            }
            _ => {}
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
