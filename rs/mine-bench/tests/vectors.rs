mod support;
use aruminium::Gpu;
use mine_bench::{cpu_accumulate, mine_one, GpuMiner, MineVariant};
use rtable_bench::RTable;
use support::{cases, expected_kind, hex, MESSAGE};

const VARIANTS: [MineVariant; 9] = [
    MineVariant::V1Single,
    MineVariant::V2DualUlong4,
    MineVariant::V3SingleUlong4,
    MineVariant::V4Batch4,
    MineVariant::V5SeqK4,
    MineVariant::V6NoBlake,
    MineVariant::V7NoLoads,
    MineVariant::V8DualTable,
    MineVariant::V9Texture,
];

#[test]
fn every_native_variant_matches_independent_full_digest_batches() {
    let gpu = Gpu::open().unwrap();
    let cases = cases();
    assert_eq!(cases.len(), 360);
    for rows in [1, 3, 65, 8192, 16384] {
        let mut table = RTable::open(&gpu, rows).unwrap();
        table.build_parallel(0, 2).unwrap();
        let mut second = RTable::open(&gpu, rows).unwrap();
        second.build_parallel(0x0706_0504, 2).unwrap();
        for variant in VARIANTS {
            if variant == MineVariant::V9Texture && rows < 8192 {
                continue;
            }
            let mut owner = GpuMiner::open(
                &gpu,
                variant,
                &table,
                variant.needs_dual_table().then_some(&second),
            )
            .unwrap();
            assert_eq!(owner.variant(), variant);
            for case in cases
                .iter()
                .filter(|c| c.rows == rows && c.kind == expected_kind(variant))
            {
                if case.count == 1 {
                    assert_ne!(case.output, [0; 32]);
                }
                for width in [1, 64] {
                    let observed = owner.run(&MESSAGE, case.base, case.count, width).unwrap();
                    assert_eq!(
                        observed, case.output,
                        "{variant:?} rows{rows} base{} count{} width{width}",
                        case.base, case.count
                    );
                    assert_eq!(
                        owner.run(&MESSAGE, case.base, case.count, width).unwrap(),
                        case.output,
                        "reused owner"
                    );
                }
            }
        }
        for case in cases
            .iter()
            .filter(|c| c.rows == rows && c.kind == "reference")
        {
            assert_eq!(
                cpu_accumulate(&MESSAGE, case.base, case.count, &table).unwrap(),
                case.output
            );
        }
    }
}

#[test]
fn individual_nonces_cannot_hide_behind_xor_cancellation() {
    let gpu = Gpu::open().unwrap();
    let mut table = RTable::open(&gpu, 65).unwrap();
    table.build_parallel(0, 2).unwrap();
    for line in include_str!("fixtures/individual.txt").lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        let nonce = parts[2].parse().unwrap();
        let expected = hex(parts[3]);
        let variant = match parts[1] {
            "reference" => MineVariant::V1Single,
            "v6" => MineVariant::V6NoBlake,
            "v7" => MineVariant::V7NoLoads,
            _ => panic!("unknown fixture"),
        };
        let mut owner = GpuMiner::open(&gpu, variant, &table, None).unwrap();
        assert_eq!(owner.run(&MESSAGE, nonce, 1, 64).unwrap(), expected);
        if variant == MineVariant::V1Single {
            assert_eq!(
                mine_one(&MESSAGE, nonce, table.bytes().unwrap()).unwrap(),
                expected
            );
        }
    }
}
