//
// Phase 2 probe kernel — random reads from the IOSurface-backed R table.
//
// Each thread:
//   1. Computes a pseudorandom row index via the same splitmix64-based
//      LCG used on the CPU side (must match lcg_index in lib.rs).
//   2. Reads the first u64 of that 32-byte row.
//   3. XORs it into a global accumulator (split into two atomic_uints
//      because device-memory 64-bit atomics are not portable on Apple
//      GPU families).
//
// The checksum reported back to the CPU must equal the CPU-side
// `cpu_checksum(seed, count)` value computed by independent code. A
// matching value verifies the IOSurface block is shared correctly:
// the GPU is reading the same bytes the CPU wrote.
//

#include <metal_stdlib>
using namespace metal;

struct Params {
    ulong n;
    ulong seed;
    uint  count;
    uint  _pad;
};

// Must match lib.rs::lcg_index exactly.
static inline ulong lcg_index(ulong seed, ulong i, ulong n) {
    ulong x = seed + i * 0x9E3779B97F4A7C15UL;
    x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9UL;
    x = (x ^ (x >> 27)) * 0x94D049BB133111EBUL;
    x ^= x >> 31;
    return x % n;
}

kernel void rtable_probe(
    device   const ulong*       table  [[buffer(0)]],   // R[0..N*4] as ulongs (4 per row)
    device   atomic_uint*       acc    [[buffer(1)]],   // 2 × u32 = 8 bytes, treated as u64
    constant Params&            p      [[buffer(2)]],
    uint                        gid    [[thread_position_in_grid]]
) {
    if (gid >= p.count) return;
    ulong idx = lcg_index(p.seed, (ulong)gid, p.n);
    // First ulong of the 32-byte row (4 ulongs per row).
    ulong row0 = table[idx * 4UL];
    uint lo = (uint)(row0 & 0xFFFFFFFFUL);
    uint hi = (uint)(row0 >> 32);
    atomic_fetch_xor_explicit(&acc[0], lo, memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[1], hi, memory_order_relaxed);
}
