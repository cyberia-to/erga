use aruminium::Gpu;
use mine_bench::{cpu_accumulate, mine_one, BenchError, GpuMiner, MineVariant};
use rtable_bench::RTable;
use std::time::Instant;
mod bench;
mod timed;

const VARIANTS: &[MineVariant] = &[
    MineVariant::V1Single,
    MineVariant::V8DualTable,
    MineVariant::V6NoBlake,
    MineVariant::V7NoLoads,
];
const M_TEST: [u8; 32] = [
    0xa5, 0xa5, 0xa5, 0xa5, 0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe, 0x12, 0x34, 0x56, 0x78,
    0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
];
fn exponent(args: &[String], default: u32) -> Result<u32, BenchError> {
    let n = args
        .get(2)
        .map(|s| s.parse::<u32>())
        .transpose()
        .map_err(|_| BenchError::InvalidInput("log2N integer"))?
        .unwrap_or(default);
    if n > 32 {
        return Err(BenchError::InvalidInput("log2N exceeds 32"));
    }
    Ok(n)
}
fn seconds(args: &[String], index: usize, default: f64) -> Result<f64, BenchError> {
    let n = args
        .get(index)
        .map(|s| s.parse::<f64>())
        .transpose()
        .map_err(|_| BenchError::InvalidInput("duration number"))?
        .unwrap_or(default);
    timed::deadline(n)?;
    Ok(n)
}
fn build_table(gpu: &Gpu, log2n: u32) -> Result<RTable<'_>, BenchError> {
    let rows = 1u64
        .checked_shl(log2n)
        .filter(|&n| n <= 1 << 32)
        .ok_or(BenchError::InvalidInput("log2N exceeds 32"))?;
    let mut table = RTable::open(gpu, rows)?;
    let threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(8);
    let start = Instant::now();
    table.build_parallel(0, threads)?;
    eprintln!(
        "R-table: {rows} rows, {} bytes, build {:.3}s",
        rows * 32,
        start.elapsed().as_secs_f64()
    );
    Ok(table)
}
fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn miner<'a, 'g>(
    gpu: &'g Gpu,
    variant: MineVariant,
    table: &'a RTable<'g>,
    second: Option<&'a RTable<'g>>,
) -> Result<GpuMiner<'a, 'g>, BenchError> {
    GpuMiner::open(
        gpu,
        variant,
        table,
        if variant.needs_dual_table() {
            second
        } else {
            None
        },
    )
}
fn compare(variant: MineVariant, expected: [u8; 32], actual: [u8; 32]) -> Result<(), BenchError> {
    if expected != actual {
        eprintln!(
            "{} mismatch: CPU {} GPU {}",
            variant.label(),
            hex32(&expected),
            hex32(&actual)
        );
        return Err(BenchError::InvalidInput(
            "kernel digest differs from reference",
        ));
    }
    println!("{} PASS", variant.label());
    Ok(())
}
fn verify(log2n: u32) -> Result<(), BenchError> {
    let gpu = Gpu::open()?;
    let table = build_table(&gpu, log2n)?;
    let second = build_table(&gpu, log2n)?;
    println!("Verifying selected buffer kernels; V9 uses separate texture tests.");
    eprintln!(
        "CPU d[nonce=0] = {}",
        hex32(&mine_one(&M_TEST, 0, table.bytes()?)?)
    );
    let expected = cpu_accumulate(&M_TEST, 0, 64, &table)?;
    for &variant in VARIANTS {
        if !variant.is_real() {
            println!(
                "{} diagnostic; covered by independent vectors",
                variant.label()
            );
            continue;
        }
        let mut owner = miner(&gpu, variant, &table, Some(&second))?;
        compare(variant, expected, owner.run(&M_TEST, 0, 64, 64)?)?;
    }
    Ok(())
}

pub(super) fn run(args: Vec<String>) -> Result<(), BenchError> {
    match args.get(1).map(String::as_str) {
        Some("verify") => verify(exponent(&args, 20)?),
        Some("bench") => bench::run(exponent(&args, 26)?),
        Some("v9") => bench::texture(exponent(&args, 26)?),
        Some("v1-sustained") => timed::sustained(exponent(&args, 26)?, seconds(&args, 3, 60.0)?),
        Some("v1-profile") => timed::profile(
            exponent(&args, 26)?,
            seconds(&args, 3, 300.0)?,
            seconds(&args, 4, 5.0)?,
        ),
        Some("hybrid") => timed::hybrid(exponent(&args, 26)?, seconds(&args, 3, 5.0)?),
        None => {
            verify(20)?;
            bench::run(26)
        }
        Some(_) => Err(BenchError::InvalidInput(
            "command: verify/bench/v9/v1-sustained/v1-profile/hybrid",
        )),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bad_arguments_fail_before_native_setup() {
        for args in [
            vec!["x", "verify", "33"],
            vec!["x", "verify", "text"],
            vec!["x", "hybrid", "4", "NaN"],
            vec!["x", "v1-profile", "4", "1", "inf"],
        ] {
            assert!(run(args.into_iter().map(String::from).collect()).is_err());
        }
    }
}
