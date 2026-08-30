//! GPU driver: exact table build (limb layout) and kernel dispatch.

use aruminium::{Dispatch, Gpu};

const SHADER: &str = include_str!("../shaders/mine_exact.metal");

/// Build R for `n` rows in the kernel's layout: each row is 4 little-endian
/// u64 limbs of the 256-bit element value (limb0 = low 64 bits).
pub fn build_table_limbs(n: u32, h: &[u8], m: &[u8]) -> Vec<u64> {
    let mut rows = vec![0u64; n as usize * 4];
    for idx in 0..n {
        let e = autolykos::table::gen_element(idx, h, m); // 31 big-endian bytes
        let mut be32 = [0u8; 32];
        be32[1..].copy_from_slice(&e);
        let o = idx as usize * 4;
        rows[o] = u64::from_be_bytes(be32[24..32].try_into().unwrap());
        rows[o + 1] = u64::from_be_bytes(be32[16..24].try_into().unwrap());
        rows[o + 2] = u64::from_be_bytes(be32[8..16].try_into().unwrap());
        rows[o + 3] = u64::from_be_bytes(be32[0..8].try_into().unwrap());
    }
    rows
}

/// Parallel table build for a real epoch (large N). Fills `rows` in place
/// across threads — each row is independent.
#[allow(dead_code)] // CPU fallback / reference build
pub fn build_table_limbs_parallel(n: u32, h: &[u8], m: &[u8], threads: usize) -> Vec<u64> {
    let mut rows = vec![0u64; n as usize * 4];
    let chunk = (n as usize).div_ceil(threads);
    std::thread::scope(|s| {
        for (t, part) in rows.chunks_mut(chunk * 4).enumerate() {
            let h = h.to_vec();
            let m = m.to_vec();
            let base = (t * chunk) as u32;
            s.spawn(move || {
                for (j, row) in part.chunks_mut(4).enumerate() {
                    let idx = base + j as u32;
                    if idx >= n {
                        break;
                    }
                    let e = autolykos::table::gen_element(idx, &h, &m);
                    let mut be32 = [0u8; 32];
                    be32[1..].copy_from_slice(&e);
                    row[0] = u64::from_be_bytes(be32[24..32].try_into().unwrap());
                    row[1] = u64::from_be_bytes(be32[16..24].try_into().unwrap());
                    row[2] = u64::from_be_bytes(be32[8..16].try_into().unwrap());
                    row[3] = u64::from_be_bytes(be32[0..8].try_into().unwrap());
                }
            });
        }
    });
    rows
}

#[repr(C)]
struct Params {
    n: u64,
    nonce_base: u64,
    count: u32,
    _pad: u32,
}

fn params_bytes(n: u64, nonce_base: u64, count: u32) -> [u8; 24] {
    let p = Params { n, nonce_base, count, _pad: 0 };
    unsafe { std::mem::transmute(p) }
}

/// Holds the epoch table on the GPU and scans nonce ranges for shares.
pub struct ScanMiner {
    gpu: Gpu,
    dispatch: Dispatch,
    pipe: aruminium::Pipeline,
    r_buf: aruminium::Buffer,
    _queue: aruminium::Queue,
    pub n: u32,
}

impl ScanMiner {
    #[allow(dead_code)] // CPU-built path; live mining uses new_gpu_built
    pub fn new(gpu: Gpu, rows: &[u64], n: u32) -> Result<ScanMiner, String> {
        let queue = gpu.new_command_queue().map_err(|e| format!("{e:?}"))?;
        let dispatch = Dispatch::new(&queue);
        let lib = gpu.compile(SHADER).map_err(|e| format!("{e:?}"))?;
        let func = lib.function("scan_kernel").map_err(|e| format!("{e:?}"))?;
        let pipe = gpu.pipeline(&func).map_err(|e| format!("{e:?}"))?;
        let r_bytes: &[u8] =
            unsafe { std::slice::from_raw_parts(rows.as_ptr() as *const u8, rows.len() * 8) };
        let r_buf = gpu.buffer_with_data(r_bytes).map_err(|e| format!("R buffer {e:?}"))?;
        Ok(ScanMiner { gpu, dispatch, pipe, r_buf, _queue: queue, n })
    }

    /// Build R on the GPU (fast enough to rebuild every Ergo block) and keep
    /// it for scanning. `m` is the 8 KiB pad. Verifies a few rows against the
    /// CPU reference before trusting the buffer.
    pub fn new_gpu_built(gpu: Gpu, n: u32, height: u32, m: &[u8]) -> Result<ScanMiner, String> {
        let queue = gpu.new_command_queue().map_err(|e| format!("{e:?}"))?;
        let dispatch = Dispatch::new(&queue);
        let lib = gpu.compile(SHADER).map_err(|e| format!("{e:?}"))?;
        let scan = gpu.pipeline(&lib.function("scan_kernel").map_err(|e| format!("{e:?}"))?)
            .map_err(|e| format!("{e:?}"))?;
        let build = gpu.pipeline(&lib.function("build_kernel").map_err(|e| format!("{e:?}"))?)
            .map_err(|e| format!("{e:?}"))?;

        let r_buf = gpu.buffer(n as usize * 32).map_err(|e| format!("R buffer {e:?}"))?;
        #[repr(C)]
        struct BuildP { n: u32, height: u32 }
        let bp: [u8; 8] = unsafe { std::mem::transmute(BuildP { n, height }) };
        let tg = 64usize;
        let grid = (n as usize).div_ceil(tg) * tg;
        unsafe {
            dispatch.dispatch_with_bytes(
                &build,
                &[(&r_buf, 0, 0)],
                &bp,
                1,
                (grid, 1, 1),
                (tg, 1, 1),
            );
        }
        // verify a handful of rows against the CPU reference
        let h = height.to_be_bytes();
        let got = r_buf.as_bytes();
        for &idx in &[0u32, 1, 7, n / 2, n - 1] {
            let e = autolykos::table::gen_element(idx, &h, m);
            let mut be32 = [0u8; 32];
            be32[1..].copy_from_slice(&e);
            let want = [
                u64::from_be_bytes(be32[24..32].try_into().unwrap()),
                u64::from_be_bytes(be32[16..24].try_into().unwrap()),
                u64::from_be_bytes(be32[8..16].try_into().unwrap()),
                u64::from_be_bytes(be32[0..8].try_into().unwrap()),
            ];
            let o = idx as usize * 32;
            for l in 0..4 {
                let g = u64::from_le_bytes(got[o + l * 8..o + l * 8 + 8].try_into().unwrap());
                if g != want[l] {
                    return Err(format!("GPU-built row {idx} limb {l} mismatch: gpu {g:x} cpu {:x}", want[l]));
                }
            }
        }
        Ok(ScanMiner { gpu, dispatch, pipe: scan, r_buf, _queue: queue, n })
    }

    /// Scan `count` nonces from `nonce_base` for hit < target. Returns the
    /// first winning full nonce, if any.
    pub fn scan(&self, msg: &[u8; 32], target: &[u8; 32], nonce_base: u64, count: u32) -> Option<u64> {
        let found = self.gpu.buffer(12).expect("found buf");
        found.write(|b| b.iter_mut().for_each(|x| *x = 0));
        let msg_buf = self.gpu.buffer_with_data(msg).expect("msg");
        let tgt_buf = self.gpu.buffer_with_data(target).expect("target");
        let p = params_bytes(self.n as u64, nonce_base, count);
        let tg = 64usize;
        let grid = (count as usize).div_ceil(tg) * tg;
        unsafe {
            self.dispatch.dispatch_with_bytes(
                &self.pipe,
                &[(&self.r_buf, 0, 0), (&found, 0, 1), (&msg_buf, 0, 2), (&tgt_buf, 0, 4)],
                &p,
                3,
                (grid, 1, 1),
                (tg, 1, 1),
            );
        }
        let b = found.as_bytes();
        let flag = u32::from_le_bytes(b[0..4].try_into().unwrap());
        if flag == 0 {
            return None;
        }
        let lo = u32::from_le_bytes(b[4..8].try_into().unwrap()) as u64;
        let hi = u32::from_le_bytes(b[8..12].try_into().unwrap()) as u64;
        Some(lo | (hi << 32))
    }
}

/// Run `diff_kernel`: return `count * 32` big-endian hit bytes.
pub fn gpu_hits(gpu: &Gpu, rows: &[u64], n: u32, msg: &[u8; 32], nonce_base: u64, count: u32) -> Vec<u8> {
    let queue = gpu.new_command_queue().expect("queue");
    let dispatch = Dispatch::new(&queue);
    let lib = gpu.compile(SHADER).expect("compile");
    let func = lib.function("diff_kernel").expect("diff_kernel");
    let pipe = gpu.pipeline(&func).expect("pipeline");

    let r_bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(rows.as_ptr() as *const u8, rows.len() * 8) };
    let r_buf = gpu.buffer_with_data(r_bytes).expect("R buffer");
    let out_buf = gpu.buffer(count as usize * 32).expect("out buffer");
    let msg_buf = gpu.buffer_with_data(msg).expect("msg buffer");
    let p = params_bytes(n as u64, nonce_base, count);

    let tg = 64usize;
    let grid = (count as usize).div_ceil(tg) * tg;
    unsafe {
        dispatch.dispatch_with_bytes(
            &pipe,
            &[(&r_buf, 0, 0), (&out_buf, 0, 1), (&msg_buf, 0, 2)],
            &p,
            3,
            (grid, 1, 1),
            (tg, 1, 1),
        );
    }
    out_buf.as_bytes()[..count as usize * 32].to_vec()
}
