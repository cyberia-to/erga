mod support;
use aruminium::Gpu;
use mine_bench::{mine_one, BenchError, GpuMiner, MineVariant};
use rtable_bench::RTable;
use support::{hex, MESSAGE};

#[test]
fn invalid_shapes_preserve_live_inputs_and_executor() {
    for length in [0, 1, 31, 33, 63] {
        assert!(matches!(
            mine_one(&MESSAGE, 0, &vec![0; length]),
            Err(BenchError::InvalidInput(_))
        ));
    }
    let gpu = Gpu::open().unwrap();
    let table = RTable::open(&gpu, 3).unwrap();
    let second = RTable::open(&gpu, 1).unwrap();
    assert!(GpuMiner::open(&gpu, MineVariant::V8DualTable, &table, None).is_err());
    assert!(GpuMiner::open(&gpu, MineVariant::V1Single, &table, Some(&second)).is_err());
    assert!(GpuMiner::open(&gpu, MineVariant::V8DualTable, &table, Some(&second)).is_err());
    assert!(GpuMiner::open(&gpu, MineVariant::V9Texture, &table, None).is_err());
    assert_eq!(table.bytes().unwrap(), &[0; 96]);
    let mut owner = GpuMiner::open(&gpu, MineVariant::V1Single, &table, None).unwrap();
    assert!(owner.run(&MESSAGE, 0, 1, 0).is_err());
    assert!(owner.run(&MESSAGE, 0, 0, usize::MAX).is_err());
    let expected = mine_one(&MESSAGE, 0, table.bytes().unwrap()).unwrap();
    assert_eq!(owner.run(&MESSAGE, 0, 1, 1).unwrap(), expected);
    assert_eq!(owner.run(&MESSAGE, 0, 0, 1).unwrap(), [0; 32]);
    assert_eq!(owner.run(&MESSAGE, 0, 1, 1).unwrap(), expected);
}

#[test]
fn immutable_cpu_and_gpu_reads_overlap_and_join_before_results() {
    let gpu = Gpu::open().unwrap();
    let mut table = RTable::open(&gpu, 65).unwrap();
    table.build_parallel(0, 2).unwrap();
    let bytes = table.bytes().unwrap();
    let expected = hex("bea2b70a0a1af310f36d777ed06137216822181309bd14a4b1793370de36a71b");
    let mut owner = GpuMiner::open(&gpu, MineVariant::V1Single, &table, None).unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let barrier = &barrier;
        let worker = scope.spawn(move || {
            barrier.wait();
            for _ in 0..64 {
                assert_eq!(mine_one(&MESSAGE, 0, bytes).unwrap(), expected);
            }
        });
        barrier.wait();
        for _ in 0..4 {
            assert_eq!(owner.run(&MESSAGE, 0, 1, 64).unwrap(), expected);
        }
        worker.join().unwrap();
    });
}
