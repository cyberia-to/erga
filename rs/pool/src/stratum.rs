//! A minimal Ergo stratum client, framed exactly as captured in
//! `autolykos/STRATUM.md` from a live herominers session.

use num_bigint::BigUint;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver};

#[derive(Clone, Debug)]
pub struct Job {
    pub job_id: String,
    pub height: u32,
    pub msg: Vec<u8>,   // 32-byte header prehash
    pub version: u8,
    pub target_b: BigUint, // share is valid when hit < target_b
}

pub enum PoolEvent {
    Job(Job),
    Difficulty(f64),
    // consumed once a running miner calls submit(); parsed already so the
    // wiring is complete and verified, awaiting the GPU miner to drive it
    #[allow(dead_code)]
    SubmitResult { id: u64, accepted: bool, error: Option<String> },
    Closed,
}

pub struct Stratum {
    // held for submit(); the probe path only reads jobs
    #[allow(dead_code)]
    stream: TcpStream,
    pub extranonce1: Vec<u8>,
    pub events: Receiver<PoolEvent>,
    #[allow(dead_code)]
    submit_id: u64,
}

impl Stratum {
    /// Connect, subscribe, authorize. Spawns a reader thread that turns
    /// each JSON line into a `PoolEvent`.
    pub fn connect(host: &str, port: u16, address: &str, worker: &str) -> std::io::Result<Stratum> {
        let stream = TcpStream::connect((host, port))?;
        stream.set_nodelay(true).ok();
        let mut w = stream.try_clone()?;

        writeln!(w, "{}", json!({"id":1,"method":"mining.subscribe","params":["erga/0.1.0", null]}))?;
        w.flush()?;

        let mut reader = BufReader::new(stream.try_clone()?);
        let en1 = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));

        // Read the subscribe reply synchronously so extranonce1 is captured
        // before we ever mine — no race. (id:1 reply carries [sub, en1, size].)
        {
            let mut line = String::new();
            for _ in 0..10 {
                line.clear();
                if reader.read_line(&mut line)? == 0 {
                    break;
                }
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    if v.get("id").and_then(|x| x.as_u64()) == Some(1) {
                        if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
                            if let Some(h) = arr.get(1).and_then(|x| x.as_str()) {
                                *en1.lock().unwrap() = hex_to_bytes(h);
                            }
                        }
                        break;
                    }
                }
            }
        }

        writeln!(w, "{}", json!({"id":2,"method":"mining.authorize","params":[format!("{address}.{worker}"),"x"]}))?;
        w.flush()?;

        let (tx, rx) = channel();
        let en1_reader = en1.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                if std::env::var("ERGA_DEBUG").is_ok() {
                    eprint!("← {line}");
                }
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    if let Some(ev) = parse_line(&v, &en1_reader) {
                        if tx.send(ev).is_err() {
                            break;
                        }
                    }
                }
            }
            let _ = tx.send(PoolEvent::Closed);
        });

        let extranonce1 = en1.lock().unwrap().clone();
        Ok(Stratum { stream, extranonce1, events: rx, submit_id: 100 })
    }

    /// Submit a share. Ergo/herominers uses the Bitcoin-style 5-param array
    /// `[worker, jobId, extraNonce2, nTime, nonce]` (per ErgoStratumProxy /
    /// ErgoStratumServer): the full nonce keeps the pool's extraNonce1 prefix,
    /// extraNonce2 is the suffix the miner searched.
    pub fn submit(
        &mut self,
        address: &str,
        worker: &str,
        job_id: &str,
        en2_hex: &str,
        nonce_hex: &str,
        ntime: &str,
    ) -> std::io::Result<u64> {
        self.submit_id += 1;
        let id = self.submit_id;
        let mut w = self.stream.try_clone()?;
        let msg = json!({"id":id,"method":"mining.submit",
            "params":[format!("{address}.{worker}"), job_id, en2_hex, ntime, nonce_hex]});
        if std::env::var("ERGA_DEBUG").is_ok() {
            eprintln!("→ {msg}");
        }
        writeln!(w, "{msg}")?;
        w.flush()?;
        Ok(id)
    }
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    (0..s.len()).step_by(2).filter_map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

fn parse_line(v: &Value, en1: &std::sync::Mutex<Vec<u8>>) -> Option<PoolEvent> {
    // subscribe reply: {"id":1,"result":[<sub>, "<en1 hex>", <en2size>]}
    if v.get("id").and_then(|x| x.as_u64()) == Some(1) {
        if let Some(arr) = v.get("result").and_then(|r| r.as_array()) {
            if let Some(en1_hex) = arr.get(1).and_then(|x| x.as_str()) {
                *en1.lock().unwrap() = hex_to_bytes(en1_hex);
            }
        }
        return None;
    }
    // submit reply: {"id":N,"result":true/false,"error":...}
    if let Some(id) = v.get("id").and_then(|x| x.as_u64()) {
        if id >= 100 {
            let accepted = v.get("result").and_then(|x| x.as_bool()).unwrap_or(false);
            let error = v.get("error").and_then(|e| if e.is_null() { None } else { Some(e.to_string()) });
            return Some(PoolEvent::SubmitResult { id, accepted, error });
        }
    }
    match v.get("method").and_then(|m| m.as_str()) {
        Some("mining.set_difficulty") => {
            let d = v.get("params").and_then(|p| p.get(0)).and_then(|x| x.as_f64()).unwrap_or(1.0);
            Some(PoolEvent::Difficulty(d))
        }
        Some("mining.notify") => {
            let p = v.get("params")?.as_array()?;
            let job_id = p.first()?.as_str().unwrap_or("0").to_string();
            let height = p.get(1)?.as_u64()? as u32;
            let msg = hex_to_bytes(p.get(2)?.as_str()?);
            let version = p.get(5).and_then(|x| x.as_u64()).unwrap_or(2) as u8;
            let target_b = p.get(6)?.as_str()?.parse::<BigUint>().ok()?;
            Some(PoolEvent::Job(Job { job_id, height, msg, version, target_b }))
        }
        _ => None,
    }
}
