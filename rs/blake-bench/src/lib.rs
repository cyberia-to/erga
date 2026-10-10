//! Synchronous Blake2b-256 benchmark batches for macOS Apple Silicon.
//!
//! CPU and GPU access must address the same physical pages. The closed owner
//! initializes and retains both backings, lends logical CPU records and excludes
//! conflicting access through successful synchronous GPU completion.

mod batch;
mod import_memory;

use aruminium::{Gpu, GpuError, Pipeline, Queue};
use std::{error::Error, fmt};
use unimem::MemError;

pub use batch::Batch;
pub mod reference;

const SHADER_V1: &str = include_str!("../shaders/blake2b_v1.metal");
const SHADER_V2: &str = include_str!("../shaders/blake2b_v2.metal");
const SHADER_V3: &str = include_str!("../shaders/blake2b_v3.metal");
const SHADER_V4: &str = include_str!("../shaders/blake2b_v4.metal");

#[derive(Copy, Clone, Debug)]
pub enum Variant {
    V1Baseline,
    V2Unrolled,
    V3DualHash,
    V4DualHashFastRot,
}

impl Variant {
    pub fn shader_src(self) -> &'static str {
        match self {
            Variant::V1Baseline => SHADER_V1,
            Variant::V2Unrolled => SHADER_V2,
            Variant::V3DualHash => SHADER_V3,
            Variant::V4DualHashFastRot => SHADER_V4,
        }
    }
    pub fn function_name(self) -> &'static str {
        match self {
            Variant::V1Baseline => "blake2b256_v1",
            Variant::V2Unrolled => "blake2b256_v2",
            Variant::V3DualHash => "blake2b256_v3",
            Variant::V4DualHashFastRot => "blake2b256_v4",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Variant::V1Baseline => "V1 baseline (loop, SIGMA table)",
            Variant::V2Unrolled => "V2 unrolled (no loop, inlined SIGMA)",
            Variant::V3DualHash => "V3 dual hash per thread (ILP)",
            Variant::V4DualHashFastRot => "V4 dual hash + per-amount rotates",
        }
    }
    pub fn hashes_per_thread(self) -> u32 {
        match self {
            Variant::V1Baseline | Variant::V2Unrolled => 1,
            Variant::V3DualHash | Variant::V4DualHashFastRot => 2,
        }
    }
}

/// Batch allocation failed before an owner was published.
#[derive(Debug)]
pub enum BenchError {
    Gpu(GpuError),
    Mem(MemError),
}

impl fmt::Display for BenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gpu(error) => error.fmt(f),
            Self::Mem(error) => error.fmt(f),
        }
    }
}
impl Error for BenchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Gpu(error) => error,
            Self::Mem(error) => error,
        })
    }
}
impl From<GpuError> for BenchError {
    fn from(error: GpuError) -> Self {
        Self::Gpu(error)
    }
}
impl From<MemError> for BenchError {
    fn from(error: MemError) -> Self {
        Self::Mem(error)
    }
}

pub struct GpuBlake2b {
    gpu: Gpu,
    queue: Queue,
    pipeline: Pipeline,
    variant: Variant,
}

fn grid(count: u32, variant: Variant) -> usize {
    let hpt = variant.hashes_per_thread();
    (count / hpt + u32::from(!count.is_multiple_of(hpt))) as usize
}

fn check_width(
    width: usize,
    pipeline: usize,
    device: (usize, usize, usize),
) -> Result<(), GpuError> {
    if width == 0 || width > pipeline || width > device.0 || device.1 == 0 || device.2 == 0 {
        return Err(GpuError::CommandBufferError(
            "threadgroup width exceeds pipeline/device limits or is zero".into(),
        ));
    }
    Ok(())
}

impl GpuBlake2b {
    pub fn open(variant: Variant) -> Result<Self, GpuError> {
        let gpu = Gpu::open()?;
        let queue = gpu.new_command_queue()?;
        let lib = gpu.compile(variant.shader_src())?;
        let func = lib.function(variant.function_name())?;
        let pipeline = gpu.pipeline(&func)?;
        Ok(Self {
            gpu,
            queue,
            pipeline,
            variant,
        })
    }

    pub fn open_v1() -> Result<Self, GpuError> {
        Self::open(Variant::V1Baseline)
    }

    pub fn variant(&self) -> Variant {
        self.variant
    }

    pub fn max_threads_per_group(&self) -> usize {
        self.pipeline.max_total_threads_per_threadgroup()
    }

    pub fn thread_execution_width(&self) -> usize {
        self.pipeline.thread_execution_width()
    }

    /// Allocate and initialize two native backings for a fixed nonzero count.
    pub fn alloc_batch(&self, count: u32) -> Result<Batch<'_>, BenchError> {
        Batch::allocate(&self.gpu, count)
    }

    /// Dispatch and wait for successful completion before CPU readback.
    /// Pre-submit errors preserve usable input; failed completion closes the batch.
    pub fn dispatch(&self, batch: &mut Batch<'_>, width: usize) -> Result<(), GpuError> {
        batch.validate(&self.gpu)?;
        let maximum = self.gpu.max_threads_per_threadgroup();
        check_width(
            width,
            self.max_threads_per_group(),
            (maximum.width, maximum.height, maximum.depth),
        )?;
        // Both private imports and their backing Blocks outlive the exclusive
        // batch borrow. Locals drop encoder then command, after wait, before that
        // borrow ends. No CPU/native alias is exported or can submit other work.
        let command = self.queue.commands()?;
        let encoder = command.encoder()?;
        encoder.bind(&self.pipeline);
        batch.bind(&encoder);
        // Exact nonuniform grid: dual-hash gid*2+1 fits uint even at count=u32MAX.
        // Apple M-series supports nonuniform groups; no padded surplus lanes.
        encoder.launch((grid(batch.count(), self.variant), 1, 1), (width, 1, 1));
        encoder.finish();
        batch.failed = true;
        command.submit();
        command.wait();
        batch.completed(command.status(), command.error())
    }
}

/// Fill logical input records with deterministic pseudo-random messages.
pub fn fill_inputs(batch: &mut Batch<'_>, seed: u64) -> Result<(), GpuError> {
    let mut s = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    for record in batch.inputs_mut()? {
        for word in record.chunks_exact_mut(8) {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            word.copy_from_slice(&s.to_le_bytes());
        }
    }
    Ok(())
}

/// Count byte-exact mismatches against the CPU reference using logical records.
/// Call after successful dispatch to verify hashes for the current inputs.
pub fn verify_zero_copy(batch: &Batch<'_>) -> Result<usize, GpuError> {
    let mut mismatches = 0;
    for (i, (input, output)) in batch.inputs()?.iter().zip(batch.outputs()?).enumerate() {
        let cpu = reference::blake2b256(input);
        if cpu != *output {
            mismatches += 1;
            if mismatches <= 3 {
                eprintln!(
                    "  mismatch at i={i}: gpu[..8]={:02x?} cpu[..8]={:02x?}",
                    &output[..8],
                    &cpu[..8]
                );
            }
        }
    }
    Ok(mismatches)
}
