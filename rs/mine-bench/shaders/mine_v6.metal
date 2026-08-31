//
// V6 DIAGNOSTIC — V1 with both Blake2b calls stripped.
//
// Synthesize the seed-hash bytes from gid via a cheap hash mix (splitmix
// style), do the 32 random reads + 256-bit sum, then XOR the sum (NOT
// hashed) into the accumulator. Compares to V1 to measure how much of
// V1's time is Blake2b compute vs random-read memory.
//
// Correctness: this kernel does NOT compute the real Autolykos d; its
// XOR accumulator will differ from V1's. The host treats this as a
// diagnostic-only kernel and does not verify against CPU reference.
//

#include <metal_stdlib>
using namespace metal;

struct Params {
    uchar  m[32];
    ulong  n;
    ulong  nonce_base;
    uint   count;
    uint   _pad;
};

static inline void add256_v4(thread ulong* sum, ulong4 r) {
    ulong t = sum[0] + r.x;
    ulong c = (t < sum[0]) ? 1UL : 0UL;
    sum[0] = t;
    t = sum[1] + r.y;
    ulong c1 = (t < sum[1]) ? 1UL : 0UL;
    ulong t2 = t + c;
    ulong c2 = (t2 < t) ? 1UL : 0UL;
    sum[1] = t2;
    c = c1 + c2;
    t = sum[2] + r.z;
    c1 = (t < sum[2]) ? 1UL : 0UL;
    t2 = t + c;
    c2 = (t2 < t) ? 1UL : 0UL;
    sum[2] = t2;
    c = c1 + c2;
    t = sum[3] + r.w;
    t2 = t + c;
    sum[3] = t2;
}

kernel void mine_kernel_v6(
    device   const ulong4*     R          [[buffer(0)]],
    device   atomic_uint*      acc        [[buffer(1)]],
    constant Params&           p          [[buffer(2)]],
    uint                       gid        [[thread_position_in_grid]]
) {
    if (gid >= p.count) return;
    ulong nonce = p.nonce_base + (ulong)gid;

    // Synthesize "seed hash" bytes from nonce via splitmix64.
    // No Blake2b — this is the diagnostic strip.
    ulong x = nonce + 0x9E3779B97F4A7C15UL;
    x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9UL;
    x = (x ^ (x >> 27)) * 0x94D049BB133111EBUL;
    x ^= x >> 31;

    ulong eb0 = x;
    ulong eb1 = x * 0x9E3779B97F4A7C15UL;
    ulong eb2 = eb1 * 0xBF58476D1CE4E5B9UL;
    ulong eb3 = eb2 * 0x94D049BB133111EBUL;
    ulong eb4 = eb0 & 0xFFFFFFUL;

    #define SBYTE6(k) (\
        ((k) <  8) ? ((eb0 >> ((k)      * 8)) & 0xFFUL) :  \
        ((k) < 16) ? ((eb1 >> (((k)-8)  * 8)) & 0xFFUL) :  \
        ((k) < 24) ? ((eb2 >> (((k)-16) * 8)) & 0xFFUL) :  \
        ((k) < 32) ? ((eb3 >> (((k)-24) * 8)) & 0xFFUL) :  \
                     ((eb4 >> (((k)-32) * 8)) & 0xFFUL) )

    #define IDX6(i) ((ulong)((uint)((SBYTE6(i  ) << 24) |  \
                                   (SBYTE6(i+1) << 16) |  \
                                   (SBYTE6(i+2) <<  8) |  \
                                    SBYTE6(i+3))) % p.n)

    #define LOAD6(i) add256_v4(sum, R[IDX6(i)])

    ulong sum[4] = {0,0,0,0};
    LOAD6( 0); LOAD6( 1); LOAD6( 2); LOAD6( 3);
    LOAD6( 4); LOAD6( 5); LOAD6( 6); LOAD6( 7);
    LOAD6( 8); LOAD6( 9); LOAD6(10); LOAD6(11);
    LOAD6(12); LOAD6(13); LOAD6(14); LOAD6(15);
    LOAD6(16); LOAD6(17); LOAD6(18); LOAD6(19);
    LOAD6(20); LOAD6(21); LOAD6(22); LOAD6(23);
    LOAD6(24); LOAD6(25); LOAD6(26); LOAD6(27);
    LOAD6(28); LOAD6(29); LOAD6(30); LOAD6(31);

    // No second Blake2b — XOR the sum directly into the accumulator.
    atomic_fetch_xor_explicit(&acc[0], (uint)(sum[0] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[1], (uint)(sum[0] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[2], (uint)(sum[1] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[3], (uint)(sum[1] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[4], (uint)(sum[2] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[5], (uint)(sum[2] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[6], (uint)(sum[3] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[7], (uint)(sum[3] >> 32),          memory_order_relaxed);
}
