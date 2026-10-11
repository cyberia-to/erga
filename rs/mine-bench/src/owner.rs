use crate::{texture_ffi, BenchError, MineVariant};
use aruminium::{Buffer, Commands, Gpu, GpuError, Pipeline, Queue, Texture};
use rtable_bench::RTable;

/// Immutable table/device borrows, private output and synchronous GPU completion.
/// Inputs are read-only in every shader; failed completion closes this executor
/// and its output, while input tables remain initialized. No native alias escapes.
///
/// ```compile_fail,E0277
/// fn send<T: Send>() {}
/// send::<mine_bench::GpuMiner<'static, 'static>>();
/// ```
/// ```compile_fail,E0277
/// fn sync<T: Sync>() {}
/// sync::<mine_bench::GpuMiner<'static, 'static>>();
/// ```
/// ```compile_fail,E0505
/// let gpu = aruminium::Gpu::open().unwrap();
/// let table = rtable_bench::RTable::open(&gpu, 1).unwrap();
/// let miner = mine_bench::GpuMiner::open(&gpu, mine_bench::MineVariant::V1Single, &table, None).unwrap();
/// drop(table);
/// std::hint::black_box(miner);
/// ```
/// ```compile_fail,E0505
/// let gpu = aruminium::Gpu::open().unwrap();
/// let table = rtable_bench::RTable::open(&gpu, 1).unwrap();
/// let miner = mine_bench::GpuMiner::open(&gpu, mine_bench::MineVariant::V1Single, &table, None).unwrap();
/// drop(gpu);
/// std::hint::black_box(miner);
/// ```
/// ```compile_fail,E0502
/// let gpu = aruminium::Gpu::open().unwrap();
/// let mut table = rtable_bench::RTable::open(&gpu, 1).unwrap();
/// let miner = mine_bench::GpuMiner::open(&gpu, mine_bench::MineVariant::V1Single, &table, None).unwrap();
/// table.build_parallel(0, 1).unwrap();
/// std::hint::black_box(miner);
/// ```
/// ```compile_fail,E0616
/// fn native(miner: &mine_bench::GpuMiner<'_, '_>) { std::hint::black_box(&miner.output); }
/// ```
pub struct GpuMiner<'table, 'gpu> {
    output: Buffer,
    texture: Option<Texture>,
    queue: Queue,
    pipeline: Pipeline,
    gpu: &'gpu Gpu,
    table: &'table RTable<'gpu>,
    second: Option<&'table RTable<'gpu>>,
    variant: MineVariant,
    failed: bool,
}

fn table_buffer<'a>(table: &'a RTable<'_>, gpu: &Gpu) -> Result<&'a Buffer, BenchError> {
    let bytes = table.bytes()?;
    if !(bytes.as_ptr() as usize).is_multiple_of(32) {
        return Err(BenchError::InvalidInput(
            "table requires 32-byte shader alignment",
        ));
    }
    // SAFETY: This private helper feeds only the const table bindings below.
    // Owner borrows exclude builds/drop; shader indices remain inside logical
    // rows; no returned Buffer is written, stored or exported. Each command is
    // waited synchronously before these retained borrows can end. Concurrent
    // immutable CPU reads are allowed; the import checks device/actual mapping.
    Ok(unsafe { table.read_only_buffer(gpu)? })
}

fn grid(
    count: u32,
    k: u32,
    width: usize,
    pipeline: usize,
    device: (usize, usize, usize),
) -> Result<usize, BenchError> {
    if width == 0 || width > pipeline || width > device.0 || device.1 == 0 || device.2 == 0 {
        return Err(BenchError::InvalidInput(
            "threadgroup width exceeds pipeline/device limits",
        ));
    }
    // k is fixed by the immutable variant (1, 2 or 4). No padded surplus lanes.
    usize::try_from(count / k + u32::from(!count.is_multiple_of(k)))
        .map_err(|_| BenchError::InvalidInput("count exceeds native grid domain"))
}

fn parameters(m: &[u8; 32], rows: u64, nonce: u64, count: u32) -> [u8; 56] {
    let mut bytes = [0; 56];
    bytes[..32].copy_from_slice(m);
    bytes[32..40].copy_from_slice(&rows.to_le_bytes());
    bytes[40..48].copy_from_slice(&nonce.to_le_bytes());
    bytes[48..52].copy_from_slice(&count.to_le_bytes());
    bytes
}

impl<'table, 'gpu> GpuMiner<'table, 'gpu> {
    /// V8 requires exactly one equal-length second table from the same device.
    /// V9 copies initialized logical bytes to its private texture once at open.
    pub fn open(
        gpu: &'gpu Gpu,
        variant: MineVariant,
        table: &'table RTable<'gpu>,
        second: Option<&'table RTable<'gpu>>,
    ) -> Result<Self, BenchError> {
        if variant.needs_dual_table() != second.is_some() {
            return Err(BenchError::InvalidInput(
                "second table is required only for V8",
            ));
        }
        table_buffer(table, gpu)?;
        if let Some(second) = second {
            if second.row_count() != table.row_count() {
                return Err(BenchError::InvalidInput("second table row count differs"));
            }
            table_buffer(second, gpu)?;
        }
        let texture = if variant == MineVariant::V9Texture {
            Some(texture_ffi::create(gpu, table.row_count(), table.bytes()?)?)
        } else {
            None
        };
        let queue = gpu.new_command_queue()?;
        let library = gpu.compile(variant.shader_src())?;
        let function = library.function(variant.function_name())?;
        let pipeline = gpu.pipeline(&function)?;
        let output = gpu.buffer_with_data(&[0; 32])?;
        let pointer = output.contents_ptr();
        if output.size() != 32
            || !output.is_shared()
            || pointer.is_null()
            || !(pointer as usize).is_multiple_of(4)
        {
            return Err(GpuError::BufferCreationFailed(
                "mining output requires 32 shared atomic-u32-aligned bytes".into(),
            )
            .into());
        }
        Ok(Self {
            output,
            texture,
            queue,
            pipeline,
            gpu,
            table,
            second,
            variant,
            failed: false,
        })
    }

    pub fn variant(&self) -> MineVariant {
        self.variant
    }

    /// Copy all digest bytes after successful completion. Count zero still
    /// validates availability, inputs and width, and submits no GPU work.
    pub fn run(
        &mut self,
        m: &[u8; 32],
        nonce_base: u64,
        count: u32,
        width: usize,
    ) -> Result<[u8; 32], BenchError> {
        if self.failed {
            return Err(BenchError::Unavailable);
        }
        let maximum = self.gpu.max_threads_per_threadgroup();
        let threads = grid(
            count,
            self.variant.nonces_per_thread(),
            width,
            self.pipeline.max_total_threads_per_threadgroup(),
            (maximum.width, maximum.height, maximum.depth),
        )?;
        let table = table_buffer(self.table, self.gpu)?;
        let second = self.second.map(|t| table_buffer(t, self.gpu)).transpose()?;
        if count == 0 {
            return Ok([0; 32]);
        }
        let params = parameters(m, self.table.row_count(), nonce_base, count);
        let command = self.queue.commands()?;
        let encoder = command.encoder()?;
        self.output.write(|bytes| bytes.copy_from_slice(&[0; 32]));
        encoder.bind(&self.pipeline);
        // V1–V7: table0/output1/params2. V8 also uses tableB3.
        // V9 substitutes texture0, retaining output1/params2.
        if let Some(texture) = &self.texture {
            texture_ffi::bind(&encoder, texture);
        } else {
            encoder.bind_buffer(table, 0, 0);
        }
        if let Some(second) = second {
            encoder.bind_buffer(second, 0, 3);
        }
        encoder.bind_buffer(&self.output, 0, 1);
        encoder.push(&params, 2);
        encoder.launch((threads, 1, 1), (width, 1, 1));
        encoder.finish();
        self.failed = true;
        command.submit();
        command.wait();
        self.completed(command.status(), command.error())?;
        let mut result = [0; 32];
        self.output.read(|bytes| result.copy_from_slice(bytes));
        Ok(result)
    }

    // Production calls only after wait; tests inject completion facts privately.
    fn completed(&mut self, status: u64, error: Option<String>) -> Result<(), BenchError> {
        if status == Commands::STATUS_COMPLETED && error.is_none() {
            self.failed = false;
            return Ok(());
        }
        Err(GpuError::CommandBufferError(match error {
            Some(detail) => format!("GPU completion status {status}: {detail}"),
            None => format!("GPU completion status {status} without native error detail"),
        })
        .into())
    }
}

#[cfg(test)]
mod tests;
