use super::*;

#[test]
fn exact_parameter_and_grid_boundaries() {
    let message = [0xa5; 32];
    let bytes = parameters(
        &message,
        0x0807_0605_0403_0201,
        0x1817_1615_1413_1211,
        0x2423_2221,
    );
    assert_eq!(&bytes[..32], &message);
    assert_eq!(
        &bytes[32..],
        &[
            1, 2, 3, 4, 5, 6, 7, 8, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x21, 0x22,
            0x23, 0x24, 0, 0, 0, 0
        ]
    );
    for (count, k, expected) in [
        (0, 1, 0),
        (1, 2, 1),
        (3, 2, 2),
        (5, 4, 2),
        (u32::MAX, 1, 4_294_967_295),
        (u32::MAX, 2, 2_147_483_648),
        (u32::MAX, 4, 1_073_741_824),
    ] {
        assert_eq!(grid(count, k, 64, 64, (64, 1, 1)).unwrap(), expected);
    }
    for (width, pipeline, device) in [
        (0, 64, (64, 1, 1)),
        (65, 64, (64, 1, 1)),
        (64, 63, (64, 1, 1)),
        (64, 64, (63, 1, 1)),
        (64, 64, (64, 0, 1)),
        (64, 64, (64, 1, 0)),
    ] {
        assert!(grid(0, 1, width, pipeline, device).is_err());
    }
}

#[test]
fn completion_failures_close_output_but_keep_read_only_tables_available() {
    let gpu = Gpu::open().unwrap();
    let table = RTable::open(&gpu, 1).unwrap();
    for (status, detail) in [
        (0, None),
        (4, Some("native detail".to_owned())),
        (5, None),
        (5, Some("failed".to_owned())),
    ] {
        let mut owner = GpuMiner::open(&gpu, MineVariant::V1Single, &table, None).unwrap();
        owner.failed = true;
        assert!(owner.completed(status, detail).is_err());
        assert!(matches!(
            owner.run(&[0; 32], 0, 0, 1),
            Err(BenchError::Unavailable)
        ));
        assert_eq!(table.bytes().unwrap(), &[0; 32]);
    }
    let mut owner = GpuMiner::open(&gpu, MineVariant::V1Single, &table, None).unwrap();
    owner.failed = true;
    owner.completed(Commands::STATUS_COMPLETED, None).unwrap();
    assert_eq!(owner.run(&[0; 32], 0, 0, 1).unwrap(), [0; 32]);
    assert_eq!(owner.output.size(), 32);
    assert!(owner.output.is_shared());
}
