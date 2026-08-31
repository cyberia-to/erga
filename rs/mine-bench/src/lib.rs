//! Phase 3 — integrated Autolykos v2 mining: combine the R-table from
//! Phase 2 with the Blake2b256 from Phase 1 into the full mining loop,
//! and measure actual MH/s on M4 Max.
//!
//! Per-nonce work (matches Autolykos v2 spec structure):
//!
//!   seed_hash    = Blake2b256(m || LE(nonce))         // 32 + 8 → 32 bytes
//!   extended[35] = seed_hash || seed_hash[0..3]
//!   for i in 0..32: idx[i] = BE_u32(extended[i..i+4]) mod N
//!   sum_256      = sum of R[idx[0]..idx[31]] mod 2^256 (32 random reads)
//!   d            = Blake2b256(sum_256)                  // 32 → 32 bytes
//!
//! For this benchmark the row size is 32 bytes (vs Autolykos spec 31)
//! and the simplified R-table builder from Phase 2 is reused. These
//! differences do not change the access pattern or the per-nonce work
//! count — only the bit-exact protocol output. Phase 5 swaps in the
//! protocol-exact byte layout.

use aruminium::{Buffer, Dispatch, Gpu, GpuError, Pipeline, Queue};
use blake_bench::reference::blake2b256;
use rtable_bench::{RTable, ROW_BYTES};

const SHADER_MINE_V1: &str = include_str!("../shaders/mine.metal");
const SHADER_MINE_V2: &str = include_str!("../shaders/mine_v2.metal");
const SHADER_MINE_V3: &str = include_str!("../shaders/mine_v3.metal");
const SHADER_MINE_V4: &str = include_str!("../shaders/mine_v4.metal");
const SHADER_MINE_V5: &str = include_str!("../shaders/mine_v5.metal");
const SHADER_MINE_V6: &str = include_str!("../shaders/mine_v6.metal");
const SHADER_MINE_V7: &str = include_str!("../shaders/mine_v7.metal");
const SHADER_MINE_V8: &str = include_str!("../shaders/mine_v8.metal");
pub const SHADER_MINE_V9: &str = include_str!("../shaders/mine_v9.metal");

pub mod texture_ffi;

#[derive(Copy, Clone, Debug)]
pub enum MineVariant {
    V1Single,
    V2DualUlong4,
    V3SingleUlong4,
    V4Batch4,
    V5SeqK4,
    V6NoBlake,
    V7NoLoads,
    V8DualTable,
}

impl MineVariant {
    pub fn shader_src(self) -> &'static str {
        match self {
            MineVariant::V1Single => SHADER_MINE_V1,
            MineVariant::V2DualUlong4 => SHADER_MINE_V2,
            MineVariant::V3SingleUlong4 => SHADER_MINE_V3,
            MineVariant::V4Batch4 => SHADER_MINE_V4,
            MineVariant::V5SeqK4 => SHADER_MINE_V5,
            MineVariant::V6NoBlake => SHADER_MINE_V6,
            MineVariant::V7NoLoads => SHADER_MINE_V7,
            MineVariant::V8DualTable => SHADER_MINE_V8,
        }
    }
    pub fn function_name(self) -> &'static str {
        match self {
            MineVariant::V1Single => "mine_kernel",
            MineVariant::V2DualUlong4 => "mine_kernel_v2",
            MineVariant::V3SingleUlong4 => "mine_kernel_v3",
            MineVariant::V4Batch4 => "mine_kernel_v4",
            MineVariant::V5SeqK4 => "mine_kernel_v5",
            MineVariant::V6NoBlake => "mine_kernel_v6",
            MineVariant::V7NoLoads => "mine_kernel_v7",
            MineVariant::V8DualTable => "mine_kernel_v8",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            MineVariant::V1Single => "V1 single-nonce baseline",
            MineVariant::V2DualUlong4 => "V2 dual-nonce + ulong4 loads",
            MineVariant::V3SingleUlong4 => "V3 single-nonce + ulong4 loads",
            MineVariant::V4Batch4 => "V4 single-nonce + 4-up batched loads",
            MineVariant::V5SeqK4 => "V5 sequential 4-nonce per thread",
            MineVariant::V6NoBlake => "V6 DIAG: no Blake2b (mem+sum only)",
            MineVariant::V7NoLoads => "V7 DIAG: no R loads (compute only)",
            MineVariant::V8DualTable => "V8 dual R-table (channel parallel)",
        }
    }
    pub fn nonces_per_thread(self) -> u32 {
        match self {
            MineVariant::V1Single => 1,
            MineVariant::V2DualUlong4 => 2,
            MineVariant::V3SingleUlong4 => 1,
            MineVariant::V4Batch4 => 1,
            MineVariant::V5SeqK4 => 4,
            MineVariant::V6NoBlake => 1,
            MineVariant::V7NoLoads => 1,
            MineVariant::V8DualTable => 1,
        }
    }
    /// Whether this variant produces real Autolykos d hashes (matches CPU
    /// reference) or is diagnostic-only.
    pub fn is_real(self) -> bool {
        !matches!(self, MineVariant::V6NoBlake | MineVariant::V7NoLoads)
    }
    /// Whether this variant needs a second R-table buffer at index 3.
    pub fn needs_dual_table(self) -> bool {
        matches!(self, MineVariant::V8DualTable)
    }
}

/// CPU reference: compute the d-hash for a single nonce.
///
/// Optimization: precompute all 32 indexes upfront, then prefetch each
/// `R[idx[i+PREFETCH_AHEAD]]` cache line while processing index i. This
/// keeps several random DRAM reads in flight per thread, hiding more
/// memory latency than the naive serialized version. Apple's ARM cores
/// have a deep ROB (~600 entries) but no guarantee they speculate
/// random-access loads aggressively; explicit `prfm` removes the
/// uncertainty.
pub fn mine_one(m: &[u8; 32], nonce: u64, r_bytes: &[u8], n: u64) -> [u8; 32] {
    let mut input = [0u8; 40];
    input[..32].copy_from_slice(m);
    input[32..40].copy_from_slice(&nonce.to_le_bytes());
    let seed_hash: [u8; 32] = blake2b256(&input);

    let mut ext = [0u8; 35];
    ext[..32].copy_from_slice(&seed_hash);
    ext[32] = seed_hash[0];
    ext[33] = seed_hash[1];
    ext[34] = seed_hash[2];

    // Precompute all 32 row offsets upfront.
    let mut offsets = [0usize; 32];
    for i in 0..32 {
        let be = u32::from_be_bytes([ext[i], ext[i + 1], ext[i + 2], ext[i + 3]]);
        let idx = (be as u64) % n;
        offsets[i] = (idx as usize) * ROW_BYTES;
    }

    // Issue prefetches for the first few rows BEFORE entering the loop.
    const PREFETCH_AHEAD: usize = 4;
    for i in 0..PREFETCH_AHEAD.min(32) {
        prefetch_read(unsafe { r_bytes.as_ptr().add(offsets[i]) });
    }

    let mut sum: [u64; 4] = [0; 4];
    for i in 0..32 {
        // Prefetch row i+PREFETCH_AHEAD (if in range) before consuming row i.
        if i + PREFETCH_AHEAD < 32 {
            prefetch_read(unsafe { r_bytes.as_ptr().add(offsets[i + PREFETCH_AHEAD]) });
        }
        let off = offsets[i];
        let row = &r_bytes[off..off + ROW_BYTES];
        let r0 = u64::from_le_bytes(row[0..8].try_into().unwrap());
        let r1 = u64::from_le_bytes(row[8..16].try_into().unwrap());
        let r2 = u64::from_le_bytes(row[16..24].try_into().unwrap());
        let r3 = u64::from_le_bytes(row[24..32].try_into().unwrap());
        add_256(&mut sum, [r0, r1, r2, r3]);
    }

    let mut sum_bytes = [0u8; 32];
    sum_bytes[0..8].copy_from_slice(&sum[0].to_le_bytes());
    sum_bytes[8..16].copy_from_slice(&sum[1].to_le_bytes());
    sum_bytes[16..24].copy_from_slice(&sum[2].to_le_bytes());
    sum_bytes[24..32].copy_from_slice(&sum[3].to_le_bytes());
    blake2b256(&sum_bytes)
}

#[inline(always)]
fn prefetch_read(ptr: *const u8) {
    // ARM64 `prfm pldl1keep, [addr]` — prefetch for read, keep in L1.
    // No-op if the data is already cached, or if prefetcher is disabled.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        std::arch::asm!(
            "prfm pldl1keep, [{0}]",
            in(reg) ptr,
            options(nostack, readonly, preserves_flags)
        );
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let _ = ptr;
    }
}

#[inline]
fn add_256(sum: &mut [u64; 4], r: [u64; 4]) {
    let mut carry: u128 = 0;
    for i in 0..4 {
        let s = sum[i] as u128 + r[i] as u128 + carry;
        sum[i] = s as u64;
        carry = s >> 64;
    }
}

/// GPU mining dispatcher.
pub struct GpuMiner {
    pub gpu: Gpu,
    _queue: Queue,
    dispatch: Dispatch,
    pipeline: Pipeline,
    pub variant: MineVariant,
}

impl GpuMiner {
    pub fn open(gpu: Gpu, variant: MineVariant) -> Result<Self, GpuError> {
        let queue = gpu.new_command_queue()?;
        let dispatch = Dispatch::new(&queue);
        let lib = gpu.compile(variant.shader_src())?;
        let func = lib.function(variant.function_name())?;
        let pipeline = gpu.pipeline(&func)?;
        Ok(Self {
            gpu,
            _queue: queue,
            dispatch,
            pipeline,
            variant,
        })
    }

    pub fn open_v1(gpu: Gpu) -> Result<Self, GpuError> {
        Self::open(gpu, MineVariant::V1Single)
    }

    /// Dispatch `count` nonce attempts starting at `nonce_base`.
    pub fn run(
        &self,
        r_table: &RTable,
        acc_buf: &Buffer,
        m: &[u8; 32],
        nonce_base: u64,
        count: u32,
        tg_width: usize,
    ) {
        self.run_dual(r_table, None, acc_buf, m, nonce_base, count, tg_width);
    }

    /// Dispatch with optional second R-table at buffer index 3 (for V8).
    pub fn run_dual(
        &self,
        r_table: &RTable,
        r_table_b: Option<&RTable>,
        acc_buf: &Buffer,
        m: &[u8; 32],
        nonce_base: u64,
        count: u32,
        tg_width: usize,
    ) {
        #[repr(C)]
        struct Params {
            m: [u8; 32],
            n: u64,
            nonce_base: u64,
            count: u32,
            _pad: u32,
        }
        let p = Params {
            m: *m,
            n: r_table.n,
            nonce_base,
            count,
            _pad: 0,
        };
        let p_bytes: [u8; 56] = unsafe { std::mem::transmute(p) };
        let nonces_per_thread = self.variant.nonces_per_thread() as usize;
        let threads = (count as usize).div_ceil(nonces_per_thread);
        let grid = threads.div_ceil(tg_width) * tg_width;
        unsafe {
            match (self.variant.needs_dual_table(), r_table_b) {
                (true, Some(rb)) => {
                    self.dispatch.dispatch_with_bytes(
                        &self.pipeline,
                        &[
                            (&r_table.buffer, 0, 0),
                            (acc_buf, 0, 1),
                            (&rb.buffer, 0, 3),
                        ],
                        &p_bytes,
                        2,
                        (grid, 1, 1),
                        (tg_width, 1, 1),
                    );
                }
                _ => {
                    self.dispatch.dispatch_with_bytes(
                        &self.pipeline,
                        &[(&r_table.buffer, 0, 0), (acc_buf, 0, 1)],
                        &p_bytes,
                        2,
                        (grid, 1, 1),
                        (tg_width, 1, 1),
                    );
                }
            }
        }
    }
}

/// CPU-side reference accumulator: XOR full 32-byte d hashes from
/// `count` consecutive nonces starting at `nonce_base`. Reads the R
/// table directly from the IOSurface-pinned `block.as_bytes()`.
pub fn cpu_accumulate(
    m: &[u8; 32],
    nonce_base: u64,
    count: u32,
    r_table: &RTable,
) -> [u8; 32] {
    let r_bytes = r_table.block.as_bytes();
    let n = r_table.n;
    let mut acc = [0u8; 32];
    for i in 0..count as u64 {
        let d = mine_one(m, nonce_base + i, r_bytes, n);
        for j in 0..32 {
            acc[j] ^= d[j];
        }
    }
    acc
}

/// V9-specific dispatcher: uses the Encoder pattern (slightly more
/// overhead than Dispatch) so we can bind a texture at slot 0. Raw
/// `setTexture:atIndex:` FFI is used because aruminium's Encoder
/// doesn't yet expose `bind_texture`.
pub struct GpuMinerV9 {
    pub gpu: Gpu,
    queue: Queue,
    pipeline: Pipeline,
    sel_set_texture: *mut std::ffi::c_void,
}

impl GpuMinerV9 {
    pub fn open(gpu: Gpu) -> Result<Self, GpuError> {
        let queue = gpu.new_command_queue()?;
        let lib = gpu.compile(SHADER_MINE_V9)?;
        let func = lib.function("mine_kernel_v9")?;
        let pipeline = gpu.pipeline(&func)?;
        let cname = std::ffi::CString::new("setTexture:atIndex:").unwrap();
        let sel_set_texture = unsafe {
            extern "C" {
                fn sel_registerName(n: *const std::ffi::c_char) -> *mut std::ffi::c_void;
            }
            sel_registerName(cname.as_ptr())
        };
        Ok(Self {
            gpu,
            queue,
            pipeline,
            sel_set_texture,
        })
    }

    pub fn run(
        &self,
        texture: &aruminium::Texture,
        acc_buf: &Buffer,
        m: &[u8; 32],
        n: u64,
        nonce_base: u64,
        count: u32,
        tg_width: usize,
    ) {
        #[repr(C)]
        struct Params {
            m: [u8; 32],
            n: u64,
            nonce_base: u64,
            count: u32,
            _pad: u32,
        }
        let p = Params {
            m: *m,
            n,
            nonce_base,
            count,
            _pad: 0,
        };
        let p_bytes: [u8; 56] = unsafe { std::mem::transmute(p) };
        let threads = count as usize;
        let grid = threads.div_ceil(tg_width) * tg_width;

        let cmd = self.queue.commands().expect("cmd buf");
        let enc = cmd.encoder().expect("encoder");
        enc.bind(&self.pipeline);

        // Raw setTexture:atIndex:0 — aruminium has no wrapper for this.
        type SetTexFn = unsafe extern "C" fn(
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
            *mut std::ffi::c_void,
            u64,
        );
        extern "C" {
            fn objc_msgSend();
        }
        let set_tex: SetTexFn =
            unsafe { std::mem::transmute(objc_msgSend as *const std::ffi::c_void) };
        unsafe {
            set_tex(
                enc.as_raw(),
                self.sel_set_texture,
                texture.as_raw() as *mut std::ffi::c_void,
                0,
            );
        }

        enc.bind_buffer(acc_buf, 0, 1);
        enc.push(&p_bytes, 2);
        enc.launch((grid, 1, 1), (tg_width, 1, 1));
        enc.finish();
        cmd.submit();
        cmd.wait();
    }
}

pub fn read_acc32(buf: &Buffer) -> [u8; 32] {
    buf.read(|b| {
        let mut out = [0u8; 32];
        out.copy_from_slice(&b[..32]);
        out
    })
}

pub fn zero_acc32(buf: &Buffer) {
    buf.write(|b| {
        for x in &mut b[..32] {
            *x = 0;
        }
    });
}

/// CPU mining session — runs for `duration_secs`, returns total nonces mined.
/// Spawns `threads` worker threads, each reading R directly from the
/// IOSurface-pinned block (zero-copy CPU access path). Uses a stop-time
/// flag rather than a fixed nonce count so wall time is the bound.
pub fn cpu_mine_for(
    table: &RTable,
    m: &[u8; 32],
    nonce_base: u64,
    duration_secs: f64,
    threads: usize,
) -> u64 {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::Duration;

    let r_bytes: &[u8] = table.block.as_bytes();
    let n = table.n;
    let total = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let total_ref = &total;
    let stop_ref = &stop;

    std::thread::scope(|s| {
        for tid in 0..threads {
            s.spawn(move || {
                let mut local: u64 = 0;
                let mut nonce = nonce_base.wrapping_add(tid as u64);
                let stride = threads as u64;
                let mut next_check = 1024u64;
                loop {
                    let _d = mine_one(m, nonce, r_bytes, n);
                    nonce = nonce.wrapping_add(stride);
                    local += 1;
                    if local == next_check {
                        if stop_ref.load(Ordering::Relaxed) {
                            break;
                        }
                        next_check += 1024;
                    }
                }
                total_ref.fetch_add(local, Ordering::Relaxed);
            });
        }
        std::thread::sleep(Duration::from_secs_f64(duration_secs));
        stop.store(true, Ordering::Relaxed);
    });

    total.load(Ordering::Relaxed)
}

/// GPU mining session — runs V1 kernel back-to-back for `duration_secs`,
/// returns total nonces mined. Uses a per-dispatch batch size and counts
/// completed dispatches.
pub fn gpu_mine_for(
    miner: &GpuMiner,
    table: &RTable,
    acc_buf: &Buffer,
    m: &[u8; 32],
    nonce_base: u64,
    duration_secs: f64,
    batch: u32,
    tg_width: usize,
) -> u64 {
    use std::time::{Duration, Instant};
    let start = Instant::now();
    let deadline = start + Duration::from_secs_f64(duration_secs);
    let mut total: u64 = 0;
    let mut next_nonce = nonce_base;
    while Instant::now() < deadline {
        miner.run(table, acc_buf, m, next_nonce, batch, tg_width);
        total = total.wrapping_add(batch as u64);
        next_nonce = next_nonce.wrapping_add(batch as u64);
    }
    total
}
