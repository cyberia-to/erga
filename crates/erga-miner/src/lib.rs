//! GPU-exact Autolykos v2 miner, as a library. `engine::run` drives the
//! whole pool-mining loop through a shared `Progress` any front-end reads.
pub mod cli;
pub mod engine;
pub mod gpu;
