use super::*;
use crate::Probe;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn layout_domains_are_checked_without_native_allocations() {
    assert!(matches!(
        layout(0, 16),
        Err(BenchError::Mem(MemError::ZeroSize))
    ));
    assert_eq!(layout(1, 16).unwrap(), (32, 32));
    assert_eq!(layout(3, 64).unwrap(), (96, 128));
    assert_eq!(layout(1 << 32, 16384).unwrap(), (1 << 37, 1 << 37));
    for rows in [(1 << 32) + 1, u64::MAX] {
        assert!(
            matches!(layout(rows, 16), Err(BenchError::RowCountOutOfRange { rows: actual }) if actual == rows)
        );
    }
    for page in [0, usize::MAX] {
        assert!(matches!(
            layout(1, page),
            Err(BenchError::Mem(MemError::SizeOverflow))
        ));
    }
    for n in [1, 3, 65, 513, 1 << 32] {
        for i in [0, 1, u64::MAX] {
            assert!(lcg_index(u64::MAX, i, n) < n);
        }
    }
}

#[test]
fn real_poisoned_actual_extent_is_zeroed_and_padding_stays_untouched() {
    let gpu = Gpu::open().unwrap();
    let (logical, request) = layout(3, import_memory::page_size().unwrap()).unwrap();
    let plan = Block::plan(request).unwrap();
    let block = plan.open().unwrap();
    let actual = block.size();
    assert!(actual > logical);
    // SAFETY: Exclusive unpublished allocation; full extent exists, no views/imports.
    unsafe { block.address().write_bytes(0xa5, actual) };
    let mut table = RTable::initialize(&gpu, 3, logical, block, actual).unwrap();
    // SAFETY: Production initializer wrote every actual byte; idle private owner.
    assert!(unsafe { table.block.as_bytes() }.iter().all(|&b| b == 0));
    assert_eq!(table.bytes().unwrap().len(), logical);
    assert_eq!(table.cpu_checksum(9, 257).unwrap(), 0);
    let mut probe = Probe::open(&gpu).unwrap();
    assert_eq!(probe.run(&mut table, 9, 257).unwrap(), 0);
    // SAFETY: Exclusive idle owner; all actual bytes initialized, no retained views.
    unsafe { table.block.as_bytes_mut()[logical..].fill(0xc7) };
    for height in [0, 0x0706_0504] {
        for threads in [1, 2, 3, usize::MAX] {
            table.build_parallel(height, threads).unwrap();
            let expected = table.cpu_checksum(9, 257).unwrap();
            assert_ne!(expected, 0);
            assert_eq!(probe.run(&mut table, 9, 257).unwrap(), expected);
            // SAFETY: Synchronous probe successfully completed; initialized tail.
            assert!(unsafe { table.block.as_bytes() }[logical..]
                .iter()
                .all(|&b| b == 0xc7));
        }
    }
}

#[test]
fn invalid_thread_count_preserves_access_and_successful_rows() {
    let gpu = Gpu::open().unwrap();
    let mut table = RTable::open(&gpu, 3).unwrap();
    table.build_parallel(7, 2).unwrap();
    let before = table.bytes().unwrap().to_vec();
    assert!(matches!(
        table.build_parallel(8, 0),
        Err(BenchError::InvalidThreadCount)
    ));
    assert_eq!(table.bytes().unwrap(), before);
}

fn closed(table: &mut RTable<'_>, probe: &mut Probe<'_>) {
    assert!(matches!(table.bytes(), Err(BenchError::Unavailable)));
    assert!(matches!(
        table.cpu_checksum(0, 0),
        Err(BenchError::Unavailable)
    ));
    assert!(matches!(
        table.build_parallel(0, 1),
        Err(BenchError::Unavailable)
    ));
    assert!(matches!(
        probe.run(table, 0, 0),
        Err(BenchError::Unavailable)
    ));
    assert_eq!(table.row_count(), 3);
}

#[test]
fn worker_panic_joins_other_workers_and_permanently_closes_access() {
    let gpu = Gpu::open().unwrap();
    let mut table = RTable::open(&gpu, 3).unwrap();
    let mut probe = Probe::open(&gpu).unwrap();
    let finished = AtomicUsize::new(0);
    let result = table.build_with(
        3,
        &|start, bytes| {
            if start == 0 {
                panic!("simulated worker panic");
            }
            bytes.fill(0xa6);
            finished.fetch_add(1, Ordering::SeqCst);
        },
        |_| Ok(()),
    );
    assert!(matches!(result, Err(BenchError::WorkerPanicked)));
    assert_eq!(finished.load(Ordering::SeqCst), 2);
    closed(&mut table, &mut probe);
}

#[test]
fn simulated_spawn_failure_joins_started_worker_and_closes_access() {
    let gpu = Gpu::open().unwrap();
    let mut table = RTable::open(&gpu, 3).unwrap();
    let mut probe = Probe::open(&gpu).unwrap();
    let finished = AtomicUsize::new(0);
    let result = table.build_with(
        3,
        &|_, bytes| {
            bytes.fill(0x96);
            finished.fetch_add(1, Ordering::SeqCst);
        },
        |worker| {
            if worker == 1 {
                Err(std::io::Error::other("simulated spawn failure"))
            } else {
                Ok(())
            }
        },
    );
    assert!(matches!(result, Err(BenchError::WorkerSpawn(_))));
    assert_eq!(finished.load(Ordering::SeqCst), 1);
    closed(&mut table, &mut probe);
}
