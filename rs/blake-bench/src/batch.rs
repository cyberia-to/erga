use crate::{import_memory, BenchError};
use aruminium::{Buffer, Commands, Encoder, Gpu, GpuError};
use unimem::{Block, MemError};

// Rust drops fields in declaration order, independently of struct-literal order.
// Private fields prevent detaching the import from its retained backing owner.
struct Storage {
    buffer: Buffer,
    block: Block,
}

impl Storage {
    fn initialize(
        gpu: &Gpu,
        block: Block,
        logical: usize,
        planned: usize,
    ) -> Result<Self, BenchError> {
        if block.size() != planned || block.size() < logical {
            return Err(
                import_memory::error("batch backing does not match its planned extent").into(),
            );
        }
        import_memory::capacity(block.size(), gpu.max_buffer_length())?;
        import_memory::validate(&block)?;
        // SAFETY: Fresh, unpublished, exclusively owned writable actual extent.
        // No references or imports exist yet. Initialize padding before any view.
        unsafe { block.address().write_bytes(0, block.size()) };
        // SAFETY: The checked, initialized, page-aligned single-region backing is
        // retained after this Buffer drops. No handle/reference escapes; Batch
        // borrows exclude conflicting CPU access until dispatch waits/checks.
        let buffer = unsafe { gpu.wrap(&block)? };
        Ok(Self { buffer, block })
    }
}

/// Initialized input/output records retained through synchronous GPU dispatch.
///
/// Views cover logical records, excluding native padding. No native alias is
/// exported. The factory device outlives the batch. Retained Buffer owners make
/// this type neither Send nor Sync; no thread-safety assertion is added here.
///
/// ```compile_fail,E0277
/// fn send<T: Send>() {}
/// send::<blake_bench::Batch<'static>>();
/// ```
/// ```compile_fail,E0277
/// fn sync<T: Sync>() {}
/// sync::<blake_bench::Batch<'static>>();
/// ```
/// ```compile_fail,E0505
/// let gpu = blake_bench::GpuBlake2b::open_v1().unwrap();
/// let batch = gpu.alloc_batch(1).unwrap();
/// drop(gpu);
/// std::hint::black_box(batch);
/// ```
pub struct Batch<'a> {
    input: Storage,
    output: Storage,
    device: &'a Gpu,
    count: u32,
    pub(crate) failed: bool,
}

fn layout(count: u32, page: usize) -> Result<(usize, usize), MemError> {
    if count == 0 {
        return Err(MemError::ZeroSize);
    }
    let logical = (count as usize)
        .checked_mul(32)
        .ok_or(MemError::SizeOverflow)?;
    let request = logical
        .checked_add(page.checked_sub(1).ok_or(MemError::SizeOverflow)?)
        .and_then(|size| size.checked_div(page))
        .and_then(|pages| pages.checked_mul(page))
        .filter(|&size| size <= isize::MAX as usize)
        .ok_or(MemError::SizeOverflow)?;
    Ok((logical, request))
}

impl<'a> Batch<'a> {
    pub(crate) fn allocate(gpu: &'a Gpu, count: u32) -> Result<Self, BenchError> {
        if count == 0 {
            return Err(MemError::ZeroSize.into());
        }
        let (logical, request) = layout(count, import_memory::page_size()?)?;
        let plan = Block::plan(request)?;
        import_memory::capacity(plan.allocation_size(), gpu.max_buffer_length())?;
        let input = Storage::initialize(gpu, plan.open()?, logical, plan.allocation_size())?;
        let output = Storage::initialize(gpu, plan.open()?, logical, plan.allocation_size())?;
        Ok(Self {
            input,
            output,
            device: gpu,
            count,
            failed: false,
        })
    }

    /// Logical record count, also inspectable after a failed completion.
    pub fn count(&self) -> u32 {
        self.count
    }

    fn available(&self) -> Result<(), GpuError> {
        if self.failed {
            return Err(GpuError::CommandBufferError(
                "batch unavailable after failed GPU completion".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn validate(&self, gpu: &Gpu) -> Result<(), GpuError> {
        self.available()?;
        if self.device.as_raw() != gpu.as_raw() {
            return Err(GpuError::CommandBufferError(
                "batch imports belong to a different Metal device".into(),
            ));
        }
        for storage in [&self.input, &self.output] {
            import_memory::capacity(storage.block.size(), gpu.max_buffer_length())?;
            import_memory::validate(&storage.block)?;
        }
        Ok(())
    }

    pub(crate) fn bind(&self, encoder: &Encoder) {
        encoder.bind_buffer(&self.input.buffer, 0, 0);
        encoder.bind_buffer(&self.output.buffer, 0, 1);
        encoder.push(&self.count.to_le_bytes(), 2);
    }

    /// Initialized logical inputs. A live view prevents writes and dispatch.
    /// ```compile_fail,E0502
    /// fn overlap(gpu: &blake_bench::GpuBlake2b, batch: &mut blake_bench::Batch<'_>) {
    ///     let inputs = batch.inputs().unwrap();
    ///     gpu.dispatch(batch, 64).unwrap();
    ///     std::hint::black_box(inputs);
    /// }
    /// ```
    pub fn inputs(&self) -> Result<&[[u8; 32]], GpuError> {
        self.available()?;
        // SAFETY: The initialized logical prefix fits retained storage; arrays
        // have alignment1. Shared borrow excludes CPU/device writes and drop.
        Ok(unsafe {
            std::slice::from_raw_parts(self.input.block.address().cast(), self.count as usize)
        })
    }

    /// Exclusive logical inputs. End this borrow before synchronous dispatch.
    /// ```compile_fail,E0499
    /// fn overlap(gpu: &blake_bench::GpuBlake2b, batch: &mut blake_bench::Batch<'_>) {
    ///     let inputs = batch.inputs_mut().unwrap();
    ///     gpu.dispatch(batch, 64).unwrap();
    ///     inputs[0][0] = 1;
    /// }
    /// ```
    pub fn inputs_mut(&mut self) -> Result<&mut [[u8; 32]], GpuError> {
        self.available()?;
        // SAFETY: Same initialized in-bounds prefix as inputs(), exclusively
        // borrowed; private imports cannot submit work or escape independently.
        Ok(unsafe {
            std::slice::from_raw_parts_mut(self.input.block.address().cast(), self.count as usize)
        })
    }

    /// Logical output bytes, initially zero. Only successful dispatch establishes
    /// their hash relation to then-current inputs. Changing inputs does not hash.
    /// After failed completion, drop/recreate the batch; all views remain closed.
    /// ```compile_fail,E0502
    /// fn overlap(gpu: &blake_bench::GpuBlake2b, batch: &mut blake_bench::Batch<'_>) {
    ///     let outputs = batch.outputs().unwrap();
    ///     gpu.dispatch(batch, 64).unwrap();
    ///     std::hint::black_box(outputs);
    /// }
    /// ```
    pub fn outputs(&self) -> Result<&[[u8; 32]], GpuError> {
        self.available()?;
        // SAFETY: Initialized logical output retained by this owner. No GPU work
        // is active: dispatch is exclusive and waits/checks before reopening views.
        Ok(unsafe {
            std::slice::from_raw_parts(self.output.block.address().cast(), self.count as usize)
        })
    }

    // Production calls only after wait; tests inject explicit completion facts.
    pub(crate) fn completed(&mut self, status: u64, error: Option<String>) -> Result<(), GpuError> {
        if status == Commands::STATUS_COMPLETED && error.is_none() {
            self.failed = false;
            return Ok(());
        }
        Err(GpuError::CommandBufferError(match error {
            Some(error) => format!("GPU completion status {status}: {error}"),
            None => format!("GPU completion status {status} without native error detail"),
        }))
    }
}

#[cfg(test)]
mod tests;
