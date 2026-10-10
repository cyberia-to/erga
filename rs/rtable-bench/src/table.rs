use crate::{import_memory, BenchError, ROW_BYTES};
use aruminium::{Buffer, Encoder, Gpu, GpuError};
use blake_bench::reference::blake2b256;
use std::panic::{catch_unwind, AssertUnwindSafe};
use unimem::{Block, MemError};

/// Fixed rows with initialized native padding and exclusive builds/dispatch.
///
/// An unbuilt table contains zero bytes. Only a successful build establishes the
/// row hash relation. A partial worker failure permanently closes all byte access.
/// No native alias escapes; the borrowed device outlives the private import.
///
/// ```compile_fail,E0277
/// fn send<T: Send>() {}
/// send::<rtable_bench::RTable<'static>>();
/// ```
/// ```compile_fail,E0277
/// fn sync<T: Sync>() {}
/// sync::<rtable_bench::RTable<'static>>();
/// ```
/// ```compile_fail,E0505
/// let gpu = aruminium::Gpu::open().unwrap();
/// let table = rtable_bench::RTable::open(&gpu, 1).unwrap();
/// drop(gpu);
/// std::hint::black_box(table);
/// ```
/// ```compile_fail,E0616
/// fn raw(table: &rtable_bench::RTable<'_>) { std::hint::black_box(&table.buffer); }
/// ```
pub struct RTable<'gpu> {
    // Declaration order releases the import before its retained backing.
    buffer: Buffer,
    block: Block,
    device: &'gpu Gpu,
    rows: u64,
    logical: usize,
    failed: bool,
}

fn layout(rows: u64, page: usize) -> Result<(usize, usize), BenchError> {
    if rows == 0 {
        return Err(MemError::ZeroSize.into());
    }
    if rows > (1u64 << 32) {
        return Err(BenchError::RowCountOutOfRange { rows });
    }
    let logical = usize::try_from(rows)
        .ok()
        .and_then(|n| n.checked_mul(ROW_BYTES))
        .ok_or(MemError::SizeOverflow)?;
    let request = page
        .checked_sub(1)
        .and_then(|pad| logical.checked_add(pad))
        .and_then(|n| n.checked_div(page))
        .and_then(|n| n.checked_mul(page))
        .filter(|&n| n <= isize::MAX as usize)
        .ok_or(MemError::SizeOverflow)?;
    Ok((logical, request))
}

fn fill_rows(start: usize, bytes: &mut [u8], height: u32) {
    let mut input = [0; 8];
    input[4..].copy_from_slice(&height.to_le_bytes());
    for (local, row) in bytes.chunks_exact_mut(ROW_BYTES).enumerate() {
        // Validated rows<=2^32 and logical-only chunks bound every index by u32MAX.
        input[..4].copy_from_slice(&((start + local) as u32).to_le_bytes());
        row.copy_from_slice(&blake2b256(&input));
    }
}

// Both callbacks are private test seams; public builds always use fill_rows and
// an infallible pre-spawn check. Scoped joining is not dependent on handle Drop.
fn workers(
    bytes: &mut [u8],
    threads: usize,
    fill: &(impl Fn(usize, &mut [u8]) + Sync),
    before_spawn: impl Fn(usize) -> std::io::Result<()>,
) -> Result<(), BenchError> {
    let rows = bytes.len() / ROW_BYTES;
    let chunk_bytes = rows.div_ceil(threads.min(rows)) * ROW_BYTES;
    // AssertUnwindSafe covers mutable chunks, not recovery of partial data:
    // caller closes the owner first. scope joins every started worker before
    // returning or propagating panic, and only total success reopens access.
    catch_unwind(AssertUnwindSafe(|| {
        std::thread::scope(|scope| {
            for (worker, chunk) in bytes.chunks_mut(chunk_bytes).enumerate() {
                before_spawn(worker).map_err(BenchError::WorkerSpawn)?;
                std::thread::Builder::new()
                    .spawn_scoped(scope, move || {
                        fill(worker * (chunk_bytes / ROW_BYTES), chunk);
                    })
                    .map_err(BenchError::WorkerSpawn)?;
            }
            Ok(())
        })
    }))
    .map_err(|_| BenchError::WorkerPanicked)?
}

impl<'gpu> RTable<'gpu> {
    /// Allocate 1..=2^32 rows, initialize the entire actual extent, then import.
    /// Zero rows returns Mem(ZeroSize); above 2^32 returns RowCountOutOfRange.
    pub fn open(gpu: &'gpu Gpu, rows: u64) -> Result<Self, BenchError> {
        let (logical, request) = layout(rows, import_memory::page_size()?)?;
        let plan = Block::plan(request)?;
        import_memory::capacity(plan.allocation_size(), gpu.max_buffer_length())?;
        Self::initialize(gpu, rows, logical, plan.open()?, plan.allocation_size())
    }

    fn initialize(
        gpu: &'gpu Gpu,
        rows: u64,
        logical: usize,
        block: Block,
        planned: usize,
    ) -> Result<Self, BenchError> {
        if block.size() != planned || block.size() < logical {
            return Err(
                import_memory::error("table backing does not match its planned extent").into(),
            );
        }
        import_memory::capacity(block.size(), gpu.max_buffer_length())?;
        import_memory::validate(&block)?;
        // SAFETY: Fresh exclusively owned full writable extent; no Rust view or
        // native import exists. Initialize all padding before either is created.
        unsafe { block.address().write_bytes(0, block.size()) };
        // SAFETY: Initialized page-aligned single-region storage stays owned until
        // after Buffer drops. Private import cannot escape; exclusive builds and
        // synchronous probes exclude conflicting access and retain it until wait.
        let buffer = unsafe { gpu.wrap(&block)? };
        Ok(Self {
            buffer,
            block,
            device: gpu,
            rows,
            logical,
            failed: false,
        })
    }

    pub fn row_count(&self) -> u64 {
        self.rows
    }

    pub(crate) fn available(&self) -> Result<(), BenchError> {
        if self.failed {
            Err(BenchError::Unavailable)
        } else {
            Ok(())
        }
    }

    /// Logical bytes, initially zero; a live view excludes build and GPU access.
    /// ```compile_fail,E0502
    /// fn overlap(table: &mut rtable_bench::RTable<'_>) {
    ///     let bytes = table.bytes().unwrap();
    ///     table.build_parallel(0, 1).unwrap();
    ///     std::hint::black_box(bytes);
    /// }
    /// ```
    pub fn bytes(&self) -> Result<&[u8], BenchError> {
        self.available()?;
        // SAFETY: Initialized prefix is within retained actual extent; shared
        // borrow excludes mutable CPU build, dispatch and owner destruction.
        Ok(unsafe { std::slice::from_raw_parts(self.block.address(), self.logical) })
    }

    /// Write Blake2b256(LE(row:u32)||LE(height:u32)) into each logical row.
    /// Threads must be positive and are clamped to row count. Every started worker
    /// finishes before return; spawn failure or panic permanently closes access.
    pub fn build_parallel(&mut self, height: u32, threads: usize) -> Result<(), BenchError> {
        self.build_with(
            threads,
            &|start, bytes| fill_rows(start, bytes, height),
            |_| Ok(()),
        )
    }

    fn build_with(
        &mut self,
        threads: usize,
        fill: &(impl Fn(usize, &mut [u8]) + Sync),
        before_spawn: impl Fn(usize) -> std::io::Result<()>,
    ) -> Result<(), BenchError> {
        self.available()?;
        if threads == 0 {
            return Err(BenchError::InvalidThreadCount);
        }
        self.failed = true;
        // SAFETY: Exclusive owner, initialized logical prefix, no active native
        // work. Disjoint scoped chunks cannot escape before scope joins them.
        let bytes = unsafe { std::slice::from_raw_parts_mut(self.block.address(), self.logical) };
        workers(bytes, threads, fill, before_spawn)?;
        self.failed = false;
        Ok(())
    }

    /// XOR the first little-endian u64 of each selected logical row.
    pub fn cpu_checksum(&self, seed: u64, count: u32) -> Result<u64, BenchError> {
        let bytes = self.bytes()?;
        let mut result = 0;
        for i in 0..u64::from(count) {
            let offset = lcg_index(seed, i, self.rows) as usize * ROW_BYTES;
            let mut word = [0; 8];
            word.copy_from_slice(&bytes[offset..offset + 8]);
            result ^= u64::from_le_bytes(word);
        }
        Ok(result)
    }

    pub(crate) fn same_device(&self, gpu: &Gpu) -> Result<(), GpuError> {
        if self.device.as_raw() != gpu.as_raw() {
            return Err(GpuError::CommandBufferError(
                "table belongs to a different Metal device".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn validate(&self) -> Result<(), GpuError> {
        import_memory::capacity(self.block.size(), self.device.max_buffer_length())?;
        import_memory::validate(&self.block)
    }
    pub(crate) fn bind(&self, encoder: &Encoder) {
        encoder.bind_buffer(&self.buffer, 0, 0);
    }
    pub(crate) fn close(&mut self) {
        self.failed = true;
    }
    pub(crate) fn reopen(&mut self) {
        self.failed = false;
    }
}

fn lcg_index(seed: u64, i: u64, rows: u64) -> u64 {
    let mut x = seed.wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (x ^ (x >> 31)) % rows
}

#[cfg(test)]
mod tests;
