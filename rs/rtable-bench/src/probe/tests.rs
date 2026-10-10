use super::*;

#[test]
fn exact_grid_and_parameters_cover_extreme_count_without_rounding() {
    for count in [0, 1, 63, 64, 65, u32::MAX] {
        assert_eq!(grid(count, 64, (64, 1, 1)).unwrap(), count as usize);
    }
    for limits in [
        (63, (64, 1, 1)),
        (64, (63, 1, 1)),
        (64, (64, 0, 1)),
        (64, (64, 1, 0)),
    ] {
        assert!(grid(1, limits.0, limits.1).is_err());
    }
    assert_eq!(
        parameters(0x0706_0504_0302_0100, 0x0f0e_0d0c_0b0a_0908, 0x1312_1110),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 0, 0, 0, 0]
    );
}

#[test]
fn simulated_completion_facts_gate_both_owners_including_zero_count() {
    let gpu = Gpu::open().unwrap();
    let mut table = RTable::open(&gpu, 1).unwrap();
    let mut probe = Probe::open(&gpu).unwrap();
    for status in [0, 1, 2, 3, 4, 5, u64::MAX] {
        for has_error in [false, true] {
            table.close();
            probe.failed = true;
            let error = has_error.then(|| "simulated native error".to_owned());
            let result = probe.completed(&mut table, status, error);
            if status == Commands::STATUS_COMPLETED && !has_error {
                result.unwrap();
                assert!(!probe.failed);
                assert!(table.bytes().is_ok());
                assert_eq!(probe.run(&mut table, 0, 0).unwrap(), 0);
            } else {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains(&format!("status {status}")));
                assert!(probe.failed);
                assert!(matches!(table.bytes(), Err(BenchError::Unavailable)));
                assert!(matches!(
                    probe.run(&mut table, 0, 0),
                    Err(BenchError::Unavailable)
                ));
                assert!(matches!(
                    table.build_parallel(0, 1),
                    Err(BenchError::Unavailable)
                ));
            }
        }
    }
}

#[test]
fn unavailable_probe_rejects_an_independent_usable_table_without_changes() {
    let gpu = Gpu::open().unwrap();
    let mut table = RTable::open(&gpu, 3).unwrap();
    table.build_parallel(7, 2).unwrap();
    let before = table.bytes().unwrap().to_vec();
    let mut probe = Probe::open(&gpu).unwrap();
    probe.failed = true;
    assert!(matches!(
        probe.run(&mut table, 0, 1),
        Err(BenchError::Unavailable)
    ));
    assert_eq!(table.bytes().unwrap(), before);
    table.build_parallel(8, 1).unwrap();
}
