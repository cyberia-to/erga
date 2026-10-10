use crate::{BenchError, RTable};
use aruminium::{Buffer, Commands, Gpu, GpuError, Pipeline, Queue};

const SHADER: &str = include_str!("../shaders/probe.metal");
const WIDTH: usize = 64;

/// Private accumulator and synchronous random-read probe; no native alias escapes.
/// Failed completion permanently closes both the probe and its table.
///
/// ```compile_fail,E0277
/// fn send<T: Send>() {}
/// send::<rtable_bench::Probe<'static>>();
/// ```
/// ```compile_fail,E0277
/// fn sync<T: Sync>() {}
/// sync::<rtable_bench::Probe<'static>>();
/// ```
/// ```compile_fail,E0505
/// let gpu = aruminium::Gpu::open().unwrap();
/// let probe = rtable_bench::Probe::open(&gpu).unwrap();
/// drop(gpu);
/// std::hint::black_box(probe);
/// ```
/// ```compile_fail,E0616
/// fn raw(probe: &rtable_bench::Probe<'_>) { std::hint::black_box(&probe.accumulator); }
/// ```
pub struct Probe<'gpu> {
    accumulator: Buffer,
    queue: Queue,
    pipeline: Pipeline,
    device: &'gpu Gpu,
    failed: bool,
}

fn grid(count: u32, pipeline: usize, device: (usize, usize, usize)) -> Result<usize, GpuError> {
    if WIDTH > pipeline || WIDTH > device.0 || device.1 == 0 || device.2 == 0 {
        return Err(GpuError::CommandBufferError(
            "probe width exceeds pipeline/device limits".into(),
        ));
    }
    usize::try_from(count)
        .map_err(|_| GpuError::CommandBufferError("probe count exceeds native grid domain".into()))
}

fn parameters(rows: u64, seed: u64, count: u32) -> [u8; 24] {
    let mut bytes = [0; 24];
    bytes[..8].copy_from_slice(&rows.to_le_bytes());
    bytes[8..16].copy_from_slice(&seed.to_le_bytes());
    bytes[16..20].copy_from_slice(&count.to_le_bytes());
    bytes
}

impl<'gpu> Probe<'gpu> {
    pub fn open(gpu: &'gpu Gpu) -> Result<Self, GpuError> {
        let queue = gpu.new_command_queue()?;
        let library = gpu.compile(SHADER)?;
        let function = library.function("rtable_probe")?;
        let pipeline = gpu.pipeline(&function)?;
        let accumulator = gpu.buffer_with_data(&[0; 8])?;
        let pointer = accumulator.contents_ptr();
        if accumulator.size() != 8
            || !accumulator.is_shared()
            || pointer.is_null()
            || !(pointer as usize).is_multiple_of(4)
        {
            return Err(GpuError::BufferCreationFailed(
                "probe accumulator must be eight shared atomic-u32-aligned bytes".into(),
            ));
        }
        Ok(Self {
            accumulator,
            queue,
            pipeline,
            device: gpu,
            failed: false,
        })
    }

    /// Return the copied checksum only after successful native completion.
    /// Pre-submit rejection preserves usable input. Count zero submits nothing,
    /// but still checks availability and device identity.
    /// ```compile_fail,E0502
    /// fn overlap(probe: &mut rtable_bench::Probe<'_>, table: &mut rtable_bench::RTable<'_>) {
    ///     let bytes = table.bytes().unwrap();
    ///     probe.run(table, 0, 1).unwrap();
    ///     std::hint::black_box(bytes);
    /// }
    /// ```
    pub fn run(
        &mut self,
        table: &mut RTable<'_>,
        seed: u64,
        count: u32,
    ) -> Result<u64, BenchError> {
        if self.failed {
            return Err(BenchError::Unavailable);
        }
        table.available()?;
        table.same_device(self.device)?;
        if count == 0 {
            return Ok(0);
        }
        table.validate()?;
        let maximum = self.device.max_threads_per_threadgroup();
        let threads = grid(
            count,
            self.pipeline.max_total_threads_per_threadgroup(),
            (maximum.width, maximum.height, maximum.depth),
        )?;
        let params = parameters(table.row_count(), seed, count);
        // The encoder drops before command, both after wait/status. Their bound
        // imports remain private and owned through the exclusive owner borrows.
        let command = self.queue.commands()?;
        let encoder = command.encoder()?;
        // Private initialized accumulator, no active command or exported alias.
        self.accumulator
            .write(|bytes| bytes.copy_from_slice(&[0; 8]));
        encoder.bind(&self.pipeline);
        table.bind(&encoder);
        encoder.bind_buffer(&self.accumulator, 0, 1);
        encoder.push(&params, 2);
        // Apple M-series supports nonuniform grids. Exactly count threads keeps
        // uint gid in range, with no padded surplus lanes or rounded overflow.
        encoder.launch((threads, 1, 1), (WIDTH, 1, 1));
        encoder.finish();
        self.failed = true;
        table.close();
        command.submit();
        command.wait();
        self.completed(table, command.status(), command.error())?;
        let mut bytes = [0; 8];
        self.accumulator.read(|value| bytes.copy_from_slice(value));
        Ok(u64::from_le_bytes(bytes))
    }

    // Production calls only after wait; private tests inject completion facts.
    fn completed(
        &mut self,
        table: &mut RTable<'_>,
        status: u64,
        error: Option<String>,
    ) -> Result<(), BenchError> {
        if status == Commands::STATUS_COMPLETED && error.is_none() {
            self.failed = false;
            table.reopen();
            return Ok(());
        }
        Err(GpuError::CommandBufferError(match error {
            Some(error) => format!("GPU completion status {status}: {error}"),
            None => format!("GPU completion status {status} without native error detail"),
        })
        .into())
    }
}

#[cfg(test)]
mod tests;
