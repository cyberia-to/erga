use super::*;

fn wait(stop: &AtomicBool) -> Result<u64, BenchError> {
    let bound = Instant::now() + Duration::from_secs(2);
    while !stop.load(Relaxed) {
        assert!(Instant::now() < bound, "worker failed to request stop");
        std::thread::yield_now();
    }
    Ok(0)
}
#[test]
fn partial_spawn_and_main_errors_join_every_started_worker() {
    for spawn_failure in [false, true] {
        let done = AtomicU64::new(0);
        let result = workers(
            2,
            &|_, stop| {
                while !stop.load(Relaxed) {
                    std::thread::yield_now();
                }
                done.fetch_add(1, Relaxed);
                Ok(1)
            },
            |_| Err(BenchError::InvalidInput("main failed")),
            |id| {
                if spawn_failure && id == 1 {
                    Err(std::io::Error::other("injected spawn failure"))
                } else {
                    Ok(())
                }
            },
        );
        if spawn_failure {
            assert!(matches!(result, Err(BenchError::WorkerSpawn(_))));
            assert_eq!(done.load(Relaxed), 1);
        } else {
            assert!(matches!(
                result,
                Err(BenchError::InvalidInput("main failed"))
            ));
            assert_eq!(done.load(Relaxed), 2);
        }
    }
}
#[test]
fn worker_failures_publish_after_joins_with_fixed_precedence() {
    assert!(matches!(
        workers(1, &|_, _| panic!("injected worker panic"), wait, |_| Ok(())),
        Err(BenchError::WorkerPanicked)
    ));
    assert!(matches!(
        workers(1, &|_, _| Err(Fault::Reference), wait, |_| Ok(())),
        Err(BenchError::InvalidInput(_))
    ));
    assert!(matches!(
        workers(
            2,
            &|id, _| Ok(if id == 0 { u64::MAX } else { 1 }),
            wait,
            |_| Ok(())
        ),
        Err(BenchError::CounterOverflow)
    ));
    assert!(matches!(
        workers(
            1,
            &|_, _| Err(Fault::Count),
            |_| Err(BenchError::InvalidInput("main")),
            |_| Ok(())
        ),
        Err(BenchError::InvalidInput("main"))
    ));
    assert!(matches!(
        workers(
            2,
            &|id, _| Err(if id == 0 {
                Fault::Count
            } else {
                Fault::Reference
            }),
            wait,
            |_| Ok(())
        ),
        Err(BenchError::CounterOverflow)
    ));
    assert!(matches!(
        workers(0, &|_, _| Ok(0), |_| Ok(0), |_| Ok(())),
        Err(BenchError::InvalidInput(_))
    ));
}
#[test]
fn invalid_durations_and_zero_batch_fail_without_work() {
    for seconds in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(deadline(seconds).is_err());
    }
    assert!(deadline(0.001).is_ok());
    let gpu = Gpu::open().unwrap();
    let table = RTable::open(&gpu, 1).unwrap();
    let mut owner = miner(&gpu, MineVariant::V1Single, &table, None).unwrap();
    assert!(matches!(
        gpu_loop(
            &mut owner,
            deadline(1.0).unwrap(),
            &AtomicBool::new(false),
            0,
            0,
            1
        ),
        Err(BenchError::InvalidInput(_))
    ));
    assert_eq!(owner.run(&M_TEST, 0, 0, 1).unwrap(), [0; 32]);
}
