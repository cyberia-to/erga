//! Initialized R-tables and synchronous GPU probes for macOS Apple Silicon.
//!
//! CPU and GPU access must address the same physical pages. Private owners retain
//! initialized backing through completion and exclude conflicting CPU/device
//! access. Row generation is the simplified eight-byte benchmark, not Autolykos.
//! Thread runtime/control allocations are separate from the native table payload.

mod import_memory;
mod probe;
mod table;

use aruminium::GpuError;
use std::{error::Error, fmt};
use unimem::MemError;

pub use probe::Probe;
pub use table::RTable;

pub const ROW_BYTES: usize = 32;

/// Construction, worker or synchronous completion failure.
#[derive(Debug)]
pub enum BenchError {
    Gpu(GpuError),
    Mem(MemError),
    /// More than 2^32 rows; zero instead returns `Mem(MemError::ZeroSize)`.
    RowCountOutOfRange {
        rows: u64,
    },
    InvalidThreadCount,
    WorkerSpawn(std::io::Error),
    WorkerPanicked,
    Unavailable,
}

impl fmt::Display for BenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gpu(error) => error.fmt(f),
            Self::Mem(error) => error.fmt(f),
            Self::RowCountOutOfRange { rows } => write!(f, "row count {rows} exceeds 2^32"),
            Self::InvalidThreadCount => f.write_str("worker count must be positive"),
            Self::WorkerSpawn(error) => write!(f, "table worker spawn failed: {error}"),
            Self::WorkerPanicked => f.write_str("table worker panicked"),
            Self::Unavailable => {
                f.write_str("owner unavailable after partial build or failed completion")
            }
        }
    }
}
impl Error for BenchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Gpu(error) => Some(error),
            Self::Mem(error) => Some(error),
            Self::WorkerSpawn(error) => Some(error),
            _ => None,
        }
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
