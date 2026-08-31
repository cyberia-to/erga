//! Phase 2 — R-table builder + GPU random-read probe on a single
//! IOSurface-pinned allocation.
//!
//! Pipeline:
//!   1. `Block::open(N * 32)`           — allocate IOSurface-pinned table
//!   2. `gpu.wrap(&block)`              — wrap as MTLBuffer (no copy)
//!   3. Parallel CPU build using std::thread::scope + chunks_mut on the
//!      pinned bytes; each thread writes Blake2b256(i_le || h_le) into
//!      its rows. Writes go directly to the IOSurface pages.
//!   4. GPU probe kernel runs over the SAME pages via the wrapped buffer.
//!   5. CPU and GPU each compute a checksum over a fixed pseudorandom
//!      index set. Identical checksums prove zero-copy sharing works.
//!
//! No Vec allocations on the table data path. The only heap memory is
//! tokenized argv parsing in main.

use std::sync::atomic::{AtomicU64, Ordering};

use aruminium::{Buffer, Dispatch, Gpu, GpuError, Pipeline, Queue};
use blake_bench::reference::blake2b256;
use unimem::{Block, MemError};

pub const ROW_BYTES: usize = 32;

const SHADER_PROBE: &str = include_str!("../shaders/probe.metal");

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

/// IOSurface-pinned R-table addressable by both CPU and GPU with no
/// copies. Field order matters: buffer drops first so the MTLBuffer
/// reference releases before the IOSurface block unmaps.
pub struct RTable {
    pub buffer: Buffer,
    pub block: Block,
    pub n: u64,
}

impl RTable {
    pub fn open(gpu: &Gpu, n: u64) -> Result<Self, BenchError> {
        let bytes = (n as usize)
            .checked_mul(ROW_BYTES)
            .expect("n * 32 overflow");
        let block = Block::open(bytes)?;
        let buffer = gpu.wrap(&block)?;
        Ok(Self { buffer, block, n })
    }

    /// IOSurface global ID — same physical pages accessible from any
    /// process or coprocessor that imports the surface.
    pub fn iosurface_id(&self) -> u32 {
        self.block.id()
    }

    /// Build the table on the CPU side using `threads` worker threads,
    /// writing directly into the IOSurface-pinned pages.
    ///
    /// Per-row content: `Blake2b256(LE(row as u32) || LE(h as u32))`.
    /// (Phase 5 will swap in the full Autolykos `Blake2b256(i || h || M)`
    /// with 8 KB protocol pad and `takeRight(31)`. This simplified form
    /// is fine for validating the zero-copy infrastructure and measuring
    /// memory bandwidth — same row size, same access pattern.)
    pub fn build_parallel(&self, h: u32, threads: usize) {
        let n = self.n as usize;
        let chunk_rows = n.div_ceil(threads);
        let chunk_bytes = chunk_rows * ROW_BYTES;
        let bytes: &mut [u8] = self.block.as_bytes_mut();

        std::thread::scope(|s| {
            let mut row_base = 0usize;
            for chunk in bytes.chunks_mut(chunk_bytes) {
                let start = row_base;
                let local_rows = chunk.len() / ROW_BYTES;
                row_base += local_rows;
                s.spawn(move || {
                    let mut input = [0u8; 8];
                    input[4..8].copy_from_slice(&h.to_le_bytes());
                    for local in 0..local_rows {
                        let row = start + local;
                        input[0..4].copy_from_slice(&(row as u32).to_le_bytes());
                        let hash = blake2b256(&input);
                        let off = local * ROW_BYTES;
                        chunk[off..off + ROW_BYTES].copy_from_slice(&hash);
                    }
                });
            }
        });
    }

    /// Compute a checksum over `count` pseudorandom rows. Reads from
    /// the IOSurface-pinned pages directly; no copies. Returns
    /// XOR of the first u64 of each selected row.
    pub fn cpu_checksum(&self, seed: u64, count: u32) -> u64 {
        let bytes: &[u8] = self.block.as_bytes();
        let n = self.n;
        let mut acc: u64 = 0;
        for i in 0..count as u64 {
            let idx = lcg_index(seed, i, n);
            let off = (idx as usize) * ROW_BYTES;
            // First u64 of the row (little-endian).
            let row0 = u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
            acc ^= row0;
        }
        acc
    }
}

/// Deterministic pseudorandom index function shared between CPU and
/// GPU. Must produce identical sequences in both implementations.
#[inline]
pub fn lcg_index(seed: u64, i: u64, n: u64) -> u64 {
    // Splitmix64 mixed with i; reduce by modulo n.
    let mut x = seed
        .wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x % n
}

/// GPU probe runner — reads `count` pseudorandom rows from the R-table
/// and XORs their first u64 into an atomic accumulator. Returns the
/// final accumulator.
pub struct Probe {
    pub gpu: Gpu,
    _queue: Queue,
    dispatch: Dispatch,
    pipeline: Pipeline,
}

impl Probe {
    pub fn open(gpu: Gpu) -> Result<Self, GpuError> {
        let queue = gpu.new_command_queue()?;
        let dispatch = Dispatch::new(&queue);
        let lib = gpu.compile(SHADER_PROBE)?;
        let func = lib.function("rtable_probe")?;
        let pipeline = gpu.pipeline(&func)?;
        Ok(Self {
            gpu,
            _queue: queue,
            dispatch,
            pipeline,
        })
    }

    /// Dispatch the probe kernel. `seed` and `count` are inline bytes.
    /// `acc_buf` is a 8-byte atomic-u64 buffer initialized to 0 by caller.
    pub fn run(&self, table: &RTable, acc_buf: &Buffer, seed: u64, count: u32) {
        #[repr(C)]
        struct Params {
            n: u64,
            seed: u64,
            count: u32,
            _pad: u32,
        }
        let p = Params {
            n: table.n,
            seed,
            count,
            _pad: 0,
        };
        let p_bytes: [u8; 24] = unsafe { std::mem::transmute(p) };

        // Round threads to threadgroup width 64.
        let tg_width = 64usize;
        let threads = (count as usize).max(1);
        let grid = threads.div_ceil(tg_width) * tg_width;

        unsafe {
            self.dispatch.dispatch_with_bytes(
                &self.pipeline,
                &[(&table.buffer, 0, 0), (acc_buf, 0, 1)],
                &p_bytes,
                2,
                (grid, 1, 1),
                (tg_width, 1, 1),
            );
        }
    }
}

/// Read the u64 stored in `buf` at offset 0 (used to read the GPU
/// probe accumulator after a dispatch).
pub fn read_u64(buf: &Buffer) -> u64 {
    buf.read(|b| u64::from_le_bytes(b[..8].try_into().unwrap()))
}

/// Write a single u64 into the first 8 bytes of `buf` (initialize
/// the probe accumulator).
pub fn write_u64(buf: &Buffer, v: u64) {
    buf.write(|b| b[..8].copy_from_slice(&v.to_le_bytes()));
}

// Silence "unused" warning on AtomicU64 import — kept to document intent
// for future contended-counter variants.
#[allow(dead_code)]
fn _unused_atomic_marker(_: &AtomicU64) {
    let _ = Ordering::Relaxed;
}
