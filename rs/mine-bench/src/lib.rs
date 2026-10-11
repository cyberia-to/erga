//! Initialized borrowed tables and synchronous simplified mining benchmarks.
//! The 32-byte rows differ from the Autolykos protocol layout.
//! Private output and texture stay retained through checked GPU completion.

mod owner;
mod reference;
mod texture_ffi;

use aruminium::GpuError;
pub use owner::GpuMiner;
pub use reference::{cpu_accumulate, mine_one};
use std::{error::Error, fmt};

const SHADER_MINE_V1: &str = include_str!("../shaders/mine.metal");
const SHADER_MINE_V2: &str = include_str!("../shaders/mine_v2.metal");
const SHADER_MINE_V3: &str = include_str!("../shaders/mine_v3.metal");
const SHADER_MINE_V4: &str = include_str!("../shaders/mine_v4.metal");
const SHADER_MINE_V5: &str = include_str!("../shaders/mine_v5.metal");
const SHADER_MINE_V6: &str = include_str!("../shaders/mine_v6.metal");
const SHADER_MINE_V7: &str = include_str!("../shaders/mine_v7.metal");
const SHADER_MINE_V8: &str = include_str!("../shaders/mine_v8.metal");
const SHADER_MINE_V9: &str = include_str!("../shaders/mine_v9.metal");

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MineVariant {
    V1Single,
    V2DualUlong4,
    V3SingleUlong4,
    V4Batch4,
    V5SeqK4,
    V6NoBlake,
    V7NoLoads,
    V8DualTable,
    V9Texture,
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
            MineVariant::V9Texture => SHADER_MINE_V9,
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
            MineVariant::V9Texture => "mine_kernel_v9",
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
            MineVariant::V9Texture => "V9 texture table copy",
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
            MineVariant::V9Texture => 1,
        }
    }
    /// Whether this matches the simplified benchmark's CPU reference. V8
    /// also requires equal table contents; this is not protocol conformance.
    pub fn is_real(self) -> bool {
        !matches!(self, MineVariant::V6NoBlake | MineVariant::V7NoLoads)
    }
    /// Whether this variant needs a second R-table buffer at index 3.
    pub fn needs_dual_table(self) -> bool {
        matches!(self, MineVariant::V8DualTable)
    }
}

/// Input, worker or synchronous native-completion failure.
#[derive(Debug)]
pub enum BenchError {
    Table(rtable_bench::BenchError),
    Gpu(GpuError),
    InvalidInput(&'static str),
    WorkerSpawn(std::io::Error),
    WorkerPanicked,
    CounterOverflow,
    Unavailable,
}
impl fmt::Display for BenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table(e) => e.fmt(f),
            Self::Gpu(e) => e.fmt(f),
            Self::InvalidInput(field) => write!(f, "invalid mining input: {field}"),
            Self::WorkerSpawn(e) => write!(f, "mining worker spawn failed: {e}"),
            Self::WorkerPanicked => f.write_str("mining worker panicked"),
            Self::CounterOverflow => f.write_str("mining count overflow"),
            Self::Unavailable => f.write_str("miner unavailable after failed completion"),
        }
    }
}
impl Error for BenchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Table(e) => Some(e),
            Self::Gpu(e) => Some(e),
            Self::WorkerSpawn(e) => Some(e),
            _ => None,
        }
    }
}
impl From<GpuError> for BenchError {
    fn from(error: GpuError) -> Self {
        Self::Gpu(error)
    }
}
impl From<rtable_bench::BenchError> for BenchError {
    fn from(error: rtable_bench::BenchError) -> Self {
        Self::Table(error)
    }
}
