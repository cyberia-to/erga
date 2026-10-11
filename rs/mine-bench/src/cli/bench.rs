use super::*;

fn measure(
    owner: &mut GpuMiner<'_, '_>,
    count: u32,
    width: usize,
    warmups: usize,
) -> Result<f64, BenchError> {
    for _ in 0..warmups {
        owner.run(&M_TEST, 0, count, width)?;
    }
    let mut best = 0.0f64;
    for _ in 0..5 {
        let start = Instant::now();
        std::hint::black_box(owner.run(&M_TEST, 0, count, width)?);
        best = best.max(f64::from(count) / start.elapsed().as_secs_f64() / 1e6);
    }
    Ok(best)
}

pub(super) fn run(log2n: u32) -> Result<(), BenchError> {
    let gpu = Gpu::open()?;
    let table = build_table(&gpu, log2n)?;
    let second = build_table(&gpu, log2n)?;
    let expected = cpu_accumulate(&M_TEST, 0, 64, &table)?;
    for &variant in VARIANTS {
        if variant.is_real() {
            let mut owner = miner(&gpu, variant, &table, Some(&second))?;
            compare(variant, expected, owner.run(&M_TEST, 0, 64, 64)?)?;
        }
    }
    println!("Simplified mining MH/s: best of five, reset/completion/copy included.");
    for &variant in VARIANTS {
        let mut owner = miner(&gpu, variant, &table, Some(&second))?;
        println!("{}", variant.label());
        let mut best = (0.0, 0, 0);
        for count in [262_144, 1_048_576, 4_194_304, 16_777_216] {
            let mut line = format!("{count:>14}");
            for width in [32, 64, 128, 256] {
                let mhs = measure(&mut owner, count, width, 2)?;
                line.push_str(&format!("{mhs:>12.2}"));
                if mhs > best.0 {
                    best = (mhs, count, width);
                }
            }
            println!("{line}");
        }
        println!(
            "peak {:.2} MH/s @ count={} width={}",
            best.0, best.1, best.2
        );
    }
    Ok(())
}

pub(super) fn texture(log2n: u32) -> Result<(), BenchError> {
    // Reject unsupported geometry before the expensive table allocation/build.
    if !(13..=27).contains(&log2n) {
        return Err(BenchError::InvalidInput("V9 log2N must be 13..=27"));
    }
    let gpu = Gpu::open()?;
    let table = build_table(&gpu, log2n)?;
    let mut v1 = miner(&gpu, MineVariant::V1Single, &table, None)?;
    let mut v9 = miner(&gpu, MineVariant::V9Texture, &table, None)?;
    let reference = cpu_accumulate(&M_TEST, 0, 64, &table)?;
    compare(
        MineVariant::V1Single,
        reference,
        v1.run(&M_TEST, 0, 64, 64)?,
    )?;
    compare(
        MineVariant::V9Texture,
        reference,
        v9.run(&M_TEST, 0, 64, 64)?,
    )?;
    println!("V9 vs V1 MH/s: best of five, reset/completion/copy included.");
    for count in [1_048_576, 4_194_304, 16_777_216] {
        for (label, owner) in [("V1 buffer", &mut v1), ("V9 texture", &mut v9)] {
            let mut line = format!("{label:<10} {count:>10}");
            for width in [32, 64, 128] {
                line.push_str(&format!(" {:>12.2}", measure(owner, count, width, 0)?));
            }
            println!("{line}");
        }
    }
    Ok(())
}
