use super::*;
use crate::{check_width, fill_inputs, grid, verify_zero_copy, GpuBlake2b, Variant};
use std::error::Error;

const VARIANTS: [Variant; 4] = [
    Variant::V1Baseline,
    Variant::V2Unrolled,
    Variant::V3DualHash,
    Variant::V4DualHashFastRot,
];

#[test]
fn checked_admission_and_error_causes() {
    assert!(matches!(layout(0, 16), Err(MemError::ZeroSize)));
    assert!(matches!(layout(1, 0), Err(MemError::SizeOverflow)));
    assert!(matches!(layout(1, usize::MAX), Err(MemError::SizeOverflow)));
    assert_eq!(layout(1, 16384).unwrap(), (32, 16384));
    assert_eq!(layout(513, 16384).unwrap(), (16416, 32768));
    assert_eq!(layout(u32::MAX, 16384).unwrap().0, u32::MAX as usize * 32);
    let gpu = GpuBlake2b::open_v1().unwrap();
    assert!(matches!(
        gpu.alloc_batch(0),
        Err(BenchError::Mem(MemError::ZeroSize))
    ));
    for error in [
        BenchError::from(MemError::ZeroSize),
        BenchError::from(import_memory::error("native detail")),
    ] {
        assert_eq!(error.to_string(), error.source().unwrap().to_string());
    }
}

#[test]
fn exact_grid_does_not_wrap_dual_hash_indices() {
    for variant in VARIANTS {
        for count in [1, 2, 3, 63, 64, 65, u32::MAX - 1, u32::MAX] {
            let threads = grid(count, variant) as u64;
            let hpt = u64::from(variant.hashes_per_thread());
            assert!(threads > 0);
            assert!((threads - 1) * hpt < u64::from(count));
            assert!(threads * hpt >= u64::from(count));
            assert!((threads - 1) * hpt + (hpt - 1) <= u64::from(u32::MAX));
        }
    }
    assert!(check_width(64, 64, (64, 1, 1)).is_ok());
    for args in [
        (0, 64, (64, 1, 1)),
        (65, 64, (128, 1, 1)),
        (65, 128, (64, 1, 1)),
        (64, 64, (64, 0, 1)),
        (64, 64, (64, 1, 0)),
    ] {
        assert!(check_width(args.0, args.1, args.2).is_err());
    }
}

#[test]
fn poisoned_full_extents_initialize_and_padding_survives_all_variants() {
    for variant in VARIANTS {
        let gpu = GpuBlake2b::open(variant).unwrap();
        let (logical, request) = layout(1, import_memory::page_size().unwrap()).unwrap();
        let plan = Block::plan(request).unwrap();
        let input = plan.open().unwrap();
        let output = plan.open().unwrap();
        let input_actual = input.size();
        let output_actual = output.size();
        assert!(input_actual > logical && output_actual > logical);
        // SAFETY: Fresh private Blocks, no imports or references. Poison only
        // their actual writable extents before calling the production initializer.
        unsafe {
            input.address().write_bytes(0xa5, input_actual);
            output.address().write_bytes(0xb7, output_actual);
        }
        let input = Storage::initialize(&gpu.gpu, input, logical, plan.allocation_size()).unwrap();
        let output =
            Storage::initialize(&gpu.gpu, output, logical, plan.allocation_size()).unwrap();
        let mut batch = Batch {
            input,
            output,
            device: &gpu.gpu,
            count: 1,
            failed: false,
        };
        // SAFETY: Entire actual backings initialized; imports are private and no
        // command has been encoded. Each view ends before the next mutation.
        assert!(unsafe { batch.input.block.as_bytes() }
            .iter()
            .all(|&b| b == 0));
        assert!(unsafe { batch.output.block.as_bytes() }
            .iter()
            .all(|&b| b == 0));
        assert_eq!(batch.inputs().unwrap().len(), 1);
        assert_eq!(batch.outputs().unwrap(), &[[0; 32]]);
        fill_inputs(&mut batch, 17).unwrap();
        let original = batch.inputs().unwrap()[0];
        // SAFETY: Exclusive owner, no live CPU views or submitted native use.
        unsafe {
            batch.input.block.as_bytes_mut()[logical..].fill(0x47);
            batch.output.block.as_bytes_mut()[logical..].fill(0x6d);
        }
        gpu.dispatch(&mut batch, 64).unwrap();
        assert_eq!(batch.inputs().unwrap()[0], original);
        assert_eq!(verify_zero_copy(&batch).unwrap(), 0);
        // SAFETY: Dispatch completed successfully; no active native or CPU writer.
        assert!(unsafe { batch.input.block.as_bytes() }[logical..]
            .iter()
            .all(|&b| b == 0x47));
        assert!(unsafe { batch.output.block.as_bytes() }[logical..]
            .iter()
            .all(|&b| b == 0x6d));
    }
}

#[test]
fn pre_submit_errors_preserve_live_bytes_and_state() {
    let gpu = GpuBlake2b::open_v1().unwrap();
    let mut batch = gpu.alloc_batch(3).unwrap();
    fill_inputs(&mut batch, 1).unwrap();
    gpu.dispatch(&mut batch, 64).unwrap();
    let inputs = batch.inputs().unwrap().to_vec();
    let outputs = batch.outputs().unwrap().to_vec();
    for width in [0, gpu.max_threads_per_group().checked_add(1).unwrap()] {
        assert!(gpu
            .dispatch(&mut batch, width)
            .unwrap_err()
            .to_string()
            .contains("threadgroup"));
        assert!(!batch.failed);
        assert_eq!(batch.inputs().unwrap(), inputs);
        assert_eq!(batch.outputs().unwrap(), outputs);
    }
    gpu.dispatch(&mut batch, 64).unwrap();
    assert_eq!(verify_zero_copy(&batch).unwrap(), 0);
}

#[test]
fn simulated_completion_observations_fail_closed() {
    let gpu = GpuBlake2b::open_v1().unwrap();
    for status in [0, 1, 2, 3, 4, 5, u64::MAX] {
        for native_error in [None, Some("native test detail".to_string())] {
            let mut batch = gpu.alloc_batch(1).unwrap();
            batch.inputs_mut().unwrap()[0][0] = 73;
            batch.failed = true; // Exact pre-submit state; no malformed native work.
            let result = batch.completed(status, native_error.clone());
            if status == Commands::STATUS_COMPLETED && native_error.is_none() {
                result.unwrap();
                assert_eq!(batch.inputs().unwrap()[0][0], 73);
            } else {
                let message = result.unwrap_err().to_string();
                assert!(message.contains(&status.to_string()));
                if native_error.is_some() {
                    assert!(message.contains("native test detail"));
                }
                for _ in 0..2 {
                    assert!(batch.inputs().is_err());
                    assert!(batch.inputs_mut().is_err());
                    assert!(batch.outputs().is_err());
                    assert!(fill_inputs(&mut batch, 1).is_err());
                    assert!(verify_zero_copy(&batch).is_err());
                    assert!(gpu
                        .dispatch(&mut batch, 64)
                        .unwrap_err()
                        .to_string()
                        .contains("batch unavailable"));
                }
            }
            assert_eq!(batch.count(), 1);
        }
    }
}
