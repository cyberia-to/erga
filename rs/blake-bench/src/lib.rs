//! GPU Blake2b-256 throughput benchmark via honeycrisp/aruminium.
//!
//! Buffers are backed by `unimem::Block` (IOSurface-pinned pages) and
//! wrapped as Metal `MTLBuffer` via `aruminium::Gpu::wrap()`. The same
//! physical pages are addressable by:
//!   - CPU via `block.as_bytes()` / `as_bytes_mut()` (no syscalls, no copies)
//!   - GPU via the wrapped MTLBuffer (Metal's zero-copy access)
//!   - AMX/NEON via raw pointer (`block.address()`)
//!   - ANE via `block.handle()` (IOSurfaceRef)
//!
//! Phase 1 only exercises the CPU+GPU axes, but the same allocation
//! becomes the R-table in Phase 2 — at which point AMX builds it on
//! the CPU side while GPU mines, with zero copies between the two.

use aruminium::{Buffer, Dispatch, Gpu, GpuError, Pipeline, Queue};
use unimem::{Block, MemError};

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

/// Errors specific to blake-bench setup.
#[derive(Debug)]
pub enum BenchError {
    Gpu(GpuError),
    Mem(MemError),
}

impl From<GpuError> for BenchError {
    fn from(e: GpuError) -> Self {
        BenchError::Gpu(e)
    }
}
impl From<MemError> for BenchError {
    fn from(e: MemError) -> Self {
        BenchError::Mem(e)
    }
}

/// IOSurface-backed pinned buffer accessible from CPU directly AND from
/// GPU as an MTLBuffer wrapping the same physical pages. No copies on
/// either side.
///
/// Field order matters: `buffer` is declared first so it drops first,
/// releasing the MTLBuffer reference before the underlying IOSurface
/// `Block` is unmapped.
pub struct ZeroCopyBuf {
    pub buffer: Buffer,
    pub block: Block,
}

impl ZeroCopyBuf {
    pub fn open(gpu: &Gpu, size: usize) -> Result<Self, BenchError> {
        let block = Block::open(size)?;
        let buffer = gpu.wrap(&block)?;
        Ok(Self { block, buffer })
    }

    /// Direct mutable CPU access to the pinned memory. No syscalls,
    /// no Metal map/unmap, no copies.
    #[inline]
    pub fn as_bytes_mut(&self) -> &mut [u8] {
        self.block.as_bytes_mut()
    }

    /// Direct immutable CPU access. After a GPU dispatch+wait, this
    /// returns the GPU's writes — same pages, no readback copy.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.block.as_bytes()
    }

    /// Underlying IOSurface global ID — usable for cross-process or
    /// ANE/AMX sharing of the same allocation.
    #[inline]
    pub fn iosurface_id(&self) -> u32 {
        self.block.id()
    }
}

pub struct GpuBlake2b {
    pub gpu: Gpu,
    _queue: Queue,
    dispatch: Dispatch,
    pipeline: Pipeline,
    pub variant: Variant,
    pub max_threads_per_group: usize,
    pub thread_execution_width: usize,
}

impl GpuBlake2b {
    pub fn open(variant: Variant) -> Result<Self, GpuError> {
        let gpu = Gpu::open()?;
        let queue = gpu.new_command_queue()?;
        let dispatch = Dispatch::new(&queue);

        let lib = gpu.compile(variant.shader_src())?;
        let func = lib.function(variant.function_name())?;
        let pipeline = gpu.pipeline(&func)?;

        let max_threads_per_group = pipeline.max_total_threads_per_threadgroup();
        let thread_execution_width = pipeline.thread_execution_width();

        Ok(Self {
            gpu,
            _queue: queue,
            dispatch,
            pipeline,
            variant,
            max_threads_per_group,
            thread_execution_width,
        })
    }

    pub fn open_v1() -> Result<Self, GpuError> {
        Self::open(Variant::V1Baseline)
    }

    /// Dispatch + wait against IOSurface-backed buffers. CPU writes to
    /// `input` are visible to the GPU and GPU writes to `output` are
    /// visible back on the CPU when this returns — no copies either way.
    pub fn dispatch(
        &self,
        input: &ZeroCopyBuf,
        output: &ZeroCopyBuf,
        count: u32,
        threadgroup_width: usize,
    ) {
        let count_bytes = count.to_le_bytes();
        let hpt = self.variant.hashes_per_thread() as usize;
        let threads = (count as usize).div_ceil(hpt);
        let grid = threads.div_ceil(threadgroup_width) * threadgroup_width;
        unsafe {
            self.dispatch.dispatch_with_bytes(
                &self.pipeline,
                &[(&input.buffer, 0, 0), (&output.buffer, 0, 1)],
                &count_bytes,
                2,
                (grid, 1, 1),
                (threadgroup_width, 1, 1),
            );
        }
    }

    /// Allocate two IOSurface-backed buffers sized for `count` 32-byte
    /// hashes.
    pub fn alloc_buffers(&self, count: u32) -> Result<(ZeroCopyBuf, ZeroCopyBuf), BenchError> {
        let bytes = (count as usize) * 32;
        let in_buf = ZeroCopyBuf::open(&self.gpu, bytes)?;
        let out_buf = ZeroCopyBuf::open(&self.gpu, bytes)?;
        Ok((in_buf, out_buf))
    }
}

/// Fill input buffer with deterministic pseudo-random 32-byte messages.
/// Writes directly into the IOSurface-pinned pages — no Metal map/unmap.
pub fn fill_inputs(buf: &ZeroCopyBuf, count: u32, seed: u64) {
    let bytes = buf.as_bytes_mut();
    let mut s = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    for i in 0..(count as usize) {
        let off = i * 32;
        for k in 0..4 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            bytes[off + k * 8..off + (k + 1) * 8].copy_from_slice(&s.to_le_bytes());
        }
    }
}

/// Verify GPU output against CPU reference with no heap allocation.
/// Reads inputs and outputs directly from IOSurface-pinned pages; the
/// CPU reference hash is produced into a stack-allocated [u8; 32]
/// (Blake2b state is internal to the `blake2` crate; this function
/// adds no heap memory of its own).
pub fn verify_zero_copy(
    in_buf: &ZeroCopyBuf,
    out_buf: &ZeroCopyBuf,
    count: u32,
) -> usize {
    let inputs = in_buf.as_bytes();
    let outputs = out_buf.as_bytes();
    let mut mismatches = 0usize;
    for i in 0..(count as usize) {
        let off = i * 32;
        let cpu: [u8; 32] = reference::blake2b256(&inputs[off..off + 32]);
        let gpu = &outputs[off..off + 32];
        if cpu != *gpu {
            mismatches += 1;
            if mismatches <= 3 {
                eprintln!(
                    "  mismatch at i={}: gpu[..8]={:02x?} cpu[..8]={:02x?}",
                    i,
                    &gpu[..8],
                    &cpu[..8]
                );
            }
        }
    }
    mismatches
}
