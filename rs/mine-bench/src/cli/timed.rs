use super::*;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering::Relaxed};
use std::time::Duration;

const REFERENCE_FAULT: u8 = 1;
const COUNT_FAULT: u8 = 2;
struct StopOnExit<'a>(&'a AtomicBool);
impl Drop for StopOnExit<'_> {
    fn drop(&mut self) {
        self.0.store(true, Relaxed);
    }
}
#[derive(Clone, Copy)]
enum Fault {
    Reference,
    Count,
}

pub(super) fn deadline(seconds: f64) -> Result<Instant, BenchError> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(BenchError::InvalidInput(
            "duration must be finite and positive",
        ));
    }
    let duration = Duration::try_from_secs_f64(seconds)
        .map_err(|_| BenchError::InvalidInput("duration exceeds clock range"))?;
    Instant::now()
        .checked_add(duration)
        .ok_or(BenchError::InvalidInput("deadline exceeds clock range"))
}

fn workers(
    threads: usize,
    work: &(impl Fn(usize, &AtomicBool) -> Result<u64, Fault> + Sync),
    main: impl FnOnce(&AtomicBool) -> Result<u64, BenchError>,
    before_spawn: impl Fn(usize) -> std::io::Result<()>,
) -> Result<(u64, u64), BenchError> {
    if threads == 0 {
        return Err(BenchError::InvalidInput("worker count must be positive"));
    }
    let stop = AtomicBool::new(false);
    let faults = AtomicU8::new(0);
    let total = AtomicU64::new(0);
    // Scope joins every worker before this catch returns, including all unwind
    // paths. Worker functions return (), so no dropped handle discards an error.
    let result = catch_unwind(AssertUnwindSafe(|| {
        std::thread::scope(|scope| {
            let _stop = StopOnExit(&stop); // must drop INSIDE scope before joins
            for id in 0..threads {
                before_spawn(id).map_err(BenchError::WorkerSpawn)?;
                let (stop, faults, total) = (&stop, &faults, &total);
                std::thread::Builder::new()
                    .spawn_scoped(scope, move || {
                        let value = catch_unwind(AssertUnwindSafe(|| work(id, stop)));
                        let fault = match value {
                            Ok(Ok(count)) => total
                                .fetch_update(Relaxed, Relaxed, |n| n.checked_add(count))
                                .err()
                                .map(|_| Fault::Count),
                            Ok(Err(fault)) => Some(fault),
                            Err(panic) => {
                                stop.store(true, Relaxed);
                                resume_unwind(panic);
                            }
                        };
                        if let Some(fault) = fault {
                            faults.fetch_or(
                                match fault {
                                    Fault::Reference => REFERENCE_FAULT,
                                    Fault::Count => COUNT_FAULT,
                                },
                                Relaxed,
                            );
                            stop.store(true, Relaxed);
                        }
                    })
                    .map_err(BenchError::WorkerSpawn)?;
            }
            main(&stop)
        })
    }));
    // Deterministic publication after joins: panic, main/spawn/GPU, overflow,
    // reference failure. No incomplete timed count is published on error.
    let gpu = result.map_err(|_| BenchError::WorkerPanicked)??;
    let flags = faults.load(Relaxed);
    if flags & COUNT_FAULT != 0 {
        return Err(BenchError::CounterOverflow);
    }
    if flags & REFERENCE_FAULT != 0 {
        return Err(BenchError::InvalidInput(
            "CPU reference rejected table bytes",
        ));
    }
    Ok((total.load(Relaxed), gpu))
}

fn cpu_worker(id: usize, stop: &AtomicBool, bytes: &[u8], threads: usize) -> Result<u64, Fault> {
    let mut nonce = id as u64;
    let mut total = 0u64;
    while !stop.load(Relaxed) {
        for _ in 0..1024 {
            std::hint::black_box(mine_one(&M_TEST, nonce, bytes).map_err(|_| Fault::Reference)?);
            nonce = nonce.wrapping_add(threads as u64);
            total = total.checked_add(1).ok_or(Fault::Count)?;
        }
    }
    Ok(total)
}
fn gpu_loop(
    owner: &mut GpuMiner<'_, '_>,
    end: Instant,
    stop: &AtomicBool,
    base: u64,
    batch: u32,
    width: usize,
) -> Result<u64, BenchError> {
    if batch == 0 {
        return Err(BenchError::InvalidInput("GPU batch must be positive"));
    }
    let mut total = 0u64;
    let mut nonce = base;
    while Instant::now() < end && !stop.load(Relaxed) {
        std::hint::black_box(owner.run(&M_TEST, nonce, batch, width)?);
        total = total
            .checked_add(u64::from(batch))
            .ok_or(BenchError::CounterOverflow)?;
        nonce = nonce.wrapping_add(u64::from(batch));
    }
    Ok(total)
}
fn cpu_for(bytes: &[u8], seconds: f64, threads: usize) -> Result<u64, BenchError> {
    let end = deadline(seconds)?;
    let (cpu, _) = workers(
        threads,
        &|id, stop| cpu_worker(id, stop, bytes, threads),
        |stop| {
            while Instant::now() < end && !stop.load(Relaxed) {
                std::thread::sleep(
                    end.saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(10)),
                );
            }
            Ok(0)
        },
        |_| Ok(()),
    )?;
    Ok(cpu)
}

pub(super) fn hybrid(log2n: u32, seconds: f64) -> Result<(), BenchError> {
    deadline(seconds)?;
    let gpu = Gpu::open()?;
    let table = build_table(&gpu, log2n)?;
    let mut owner = miner(&gpu, MineVariant::V1Single, &table, None)?;
    let threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(16);
    let start = Instant::now();
    let cpu = cpu_for(table.bytes()?, seconds, threads)?;
    let cpu_mhs = cpu as f64 / start.elapsed().as_secs_f64() / 1e6;
    println!("CPU only: {cpu_mhs:.2} MH/s ({cpu} nonces)");
    let start = Instant::now();
    let gpu_count = gpu_loop(
        &mut owner,
        deadline(seconds)?,
        &AtomicBool::new(false),
        0,
        4_194_304,
        64,
    )?;
    let gpu_mhs = gpu_count as f64 / start.elapsed().as_secs_f64() / 1e6;
    println!("GPU only: {gpu_mhs:.2} MH/s ({gpu_count} nonces)");
    let bytes = table.bytes()?;
    let end = deadline(seconds)?;
    let start = Instant::now();
    let (cpu, gpu) = workers(
        threads,
        &|id, stop| cpu_worker(id, stop, bytes, threads),
        |stop| gpu_loop(&mut owner, end, stop, 1 << 40, 4_194_304, 64),
        |_| Ok(()),
    )?;
    let count = cpu.checked_add(gpu).ok_or(BenchError::CounterOverflow)?;
    let duration = start.elapsed().as_secs_f64();
    let mhs = count as f64 / duration / 1e6;
    println!(
        "Concurrent CPU: {:.2} MH/s ({cpu} nonces)",
        cpu as f64 / duration / 1e6
    );
    println!(
        "Concurrent GPU: {:.2} MH/s ({gpu} nonces)",
        gpu as f64 / duration / 1e6
    );
    println!(
        "Concurrent total: {mhs:.2} MH/s; independent sum {:.2}",
        cpu_mhs + gpu_mhs
    );
    println!(
        "Interference loss: {:.2} MH/s ({:.1}%)",
        cpu_mhs + gpu_mhs - mhs,
        100.0 * (cpu_mhs + gpu_mhs - mhs) / (cpu_mhs + gpu_mhs)
    );
    Ok(())
}

pub(super) fn sustained(log2n: u32, seconds: f64) -> Result<(), BenchError> {
    deadline(seconds)?;
    let gpu = Gpu::open()?;
    let table = build_table(&gpu, log2n)?;
    let mut owner = miner(&gpu, MineVariant::V1Single, &table, None)?;
    let start = Instant::now();
    let total = gpu_loop(
        &mut owner,
        deadline(seconds)?,
        &AtomicBool::new(false),
        0,
        4_194_304,
        64,
    )?;
    let duration = start.elapsed().as_secs_f64();
    println!(
        "V1 sustained: {:.2} MH/s ({total} nonces in {duration:.2}s)",
        total as f64 / duration / 1e6
    );
    Ok(())
}

pub(super) fn profile(
    log2n: u32,
    total_seconds: f64,
    window_seconds: f64,
) -> Result<(), BenchError> {
    deadline(total_seconds)?;
    deadline(window_seconds)?;
    let gpu = Gpu::open()?;
    let table = build_table(&gpu, log2n)?;
    let mut owner = miner(&gpu, MineVariant::V1Single, &table, None)?;
    let start = Instant::now();
    let end = deadline(total_seconds)?;
    let mut window = start;
    let (mut total, mut local, mut nonce) = (0u64, 0u64, 0u64);
    while Instant::now() < end {
        std::hint::black_box(owner.run(&M_TEST, nonce, 4_194_304, 64)?);
        nonce = nonce.wrapping_add(4_194_304);
        total = total
            .checked_add(4_194_304)
            .ok_or(BenchError::CounterOverflow)?;
        local = local
            .checked_add(4_194_304)
            .ok_or(BenchError::CounterOverflow)?;
        let elapsed = window.elapsed().as_secs_f64();
        if elapsed >= window_seconds {
            let duration = start.elapsed().as_secs_f64();
            println!(
                "{duration:.1}s window {:.2} MH/s cumulative {:.2} MH/s",
                local as f64 / elapsed / 1e6,
                total as f64 / duration / 1e6
            );
            window = Instant::now();
            local = 0;
        }
    }
    let duration = start.elapsed().as_secs_f64();
    println!(
        "FINAL: {:.2} MH/s sustained over {duration:.1}s",
        total as f64 / duration / 1e6
    );
    Ok(())
}

#[cfg(test)]
mod tests;
