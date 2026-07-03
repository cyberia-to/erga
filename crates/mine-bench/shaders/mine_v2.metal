//
// V2 mining kernel — 2 nonces per thread + ulong4 vector loads.
//
// The integrated mining loop is memory-bound: each nonce issues 32 random
// reads from R, and the next iteration's index depends on the seed hash
// of THIS nonce (no cross-nonce data dependency). Processing two nonces
// per thread doubles the in-flight memory requests per thread, giving
// the GPU memory subsystem more outstanding loads to schedule. This is
// the classic trick to push memory utilization beyond the single-thread
// outstanding-load ceiling.
//
// `ulong4` loads force a single 32-byte SIMD load per row instead of
// four sequential 8-byte loads (the compiler may already do this; making
// it explicit removes the dependence on that optimization choice).
//
// Per thread compared to V1:
//   - 2× sum accumulator (8 ulongs vs 4)
//   - 2× seed hash buffer (8 ulongs vs 4)
//   - 2× Blake2b state during the two compression passes
// Total per-thread state ~60-65 ulongs (= 120-130 32-bit registers).
// Apple GPU per-thread budget is 128 32-bit registers — tight but workable.
//

#include <metal_stdlib>
using namespace metal;

constant ulong IV0 = 0x6a09e667f3bcc908UL;
constant ulong IV1 = 0xbb67ae8584caa73bUL;
constant ulong IV2 = 0x3c6ef372fe94f82bUL;
constant ulong IV3 = 0xa54ff53a5f1d36f1UL;
constant ulong IV4 = 0x510e527fade682d1UL;
constant ulong IV5 = 0x9b05688c2b3e6c1fUL;
constant ulong IV6 = 0x1f83d9abfb41bd6bUL;
constant ulong IV7 = 0x5be0cd19137e2179UL;

struct Params {
    uchar  m[32];
    ulong  n;
    ulong  nonce_base;
    uint   count;
    uint   _pad;
};

static inline ulong rotr64(ulong x, uint n) {
    return (x >> n) | (x << (64 - n));
}

#define G(va, vb, vc, vd, x, y) do {                              \
    va = va + vb + (x);                                           \
    vd = rotr64(vd ^ va, 32);                                     \
    vc = vc + vd;                                                 \
    vb = rotr64(vb ^ vc, 24);                                     \
    va = va + vb + (y);                                           \
    vd = rotr64(vd ^ va, 16);                                     \
    vc = vc + vd;                                                 \
    vb = rotr64(vb ^ vc, 63);                                     \
} while (0)

#define R12(m,                                                    \
            s0,s1,s2,s3,s4,s5,s6,s7,s8,s9,sa,sb,sc,sd,se,sf,      \
            v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF) do { \
    G(v0, v4, v8,  vC, m[s0], m[s1]);                             \
    G(v1, v5, v9,  vD, m[s2], m[s3]);                             \
    G(v2, v6, vA,  vE, m[s4], m[s5]);                             \
    G(v3, v7, vB,  vF, m[s6], m[s7]);                             \
    G(v0, v5, vA,  vF, m[s8], m[s9]);                             \
    G(v1, v6, vB,  vC, m[sa], m[sb]);                             \
    G(v2, v7, v8,  vD, m[sc], m[sd]);                             \
    G(v3, v4, v9,  vE, m[se], m[sf]);                             \
} while (0)

static inline void blake2b256_block(thread ulong* m, uint t_bytes, thread ulong* h_out) {
    ulong v0 = IV0 ^ 0x01010020UL;
    ulong v1 = IV1;
    ulong v2 = IV2;
    ulong v3 = IV3;
    ulong v4 = IV4;
    ulong v5 = IV5;
    ulong v6 = IV6;
    ulong v7 = IV7;
    ulong v8 = IV0;
    ulong v9 = IV1;
    ulong vA = IV2;
    ulong vB = IV3;
    ulong vC = IV4 ^ (ulong)t_bytes;
    ulong vD = IV5;
    ulong vE = IV6 ^ 0xFFFFFFFFFFFFFFFFUL;
    ulong vF = IV7;

    R12(m,  0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 11, 8,12, 0, 5, 2,15,13,10,14, 3, 6, 7, 1, 9, 4, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m,  7, 9, 3, 1,13,12,11,14, 2, 6, 5,10, 4, 0,15, 8, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m,  9, 0, 5, 7, 2, 4,10,15,14, 1,11,12, 6, 8, 3,13, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m,  2,12, 6,10, 0,11, 8, 3, 4,13, 7, 5,15,14, 1, 9, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 12, 5, 1,15,14,13, 4,10, 0, 7, 6, 3, 9, 2, 8,11, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 13,11, 7,14,12, 1, 3, 9, 5, 0,15, 4, 8, 6, 2,10, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m,  6,15,14, 9,11, 3, 0, 8,12, 2,13, 7, 1, 4,10, 5, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 10, 2, 8, 4, 7, 6, 1, 5,15,11, 9,14, 3,12,13, 0, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m,  0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);

    h_out[0] = (IV0 ^ 0x01010020UL) ^ v0 ^ v8;
    h_out[1] =  IV1                  ^ v1 ^ v9;
    h_out[2] =  IV2                  ^ v2 ^ vA;
    h_out[3] =  IV3                  ^ v3 ^ vB;
}

// 256-bit add with carry, modulo 2^256. `r` is a ulong4 of the row.
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

// Build the seed-hash bytes packing (5 ulongs: 4 hash words + low 3 bytes
// of word 0 in the 5th slot) so byte index lookup is uniform.
#define SBYTE(eb0,eb1,eb2,eb3,eb4,k) (\
    ((k) <  8) ? ((eb0 >> ((k)      * 8)) & 0xFFUL) :  \
    ((k) < 16) ? ((eb1 >> (((k)-8)  * 8)) & 0xFFUL) :  \
    ((k) < 24) ? ((eb2 >> (((k)-16) * 8)) & 0xFFUL) :  \
    ((k) < 32) ? ((eb3 >> (((k)-24) * 8)) & 0xFFUL) :  \
                 ((eb4 >> (((k)-32) * 8)) & 0xFFUL) )

kernel void mine_kernel_v2(
    device   const ulong4*     R          [[buffer(0)]],   // ulong4 per row
    device   atomic_uint*      acc        [[buffer(1)]],
    constant Params&           p          [[buffer(2)]],
    uint                       gid        [[thread_position_in_grid]]
) {
    uint nonce_idx_A = gid * 2u;
    uint nonce_idx_B = nonce_idx_A + 1u;
    if (nonce_idx_A >= p.count) return;
    bool has_b = (nonce_idx_B < p.count);

    ulong nonce_A = p.nonce_base + (ulong)nonce_idx_A;
    ulong nonce_B = p.nonce_base + (ulong)nonce_idx_B;

    // Pack m[32] into ulongs once (shared between nonces A and B).
    ulong m0 = ((ulong)p.m[ 0])       | ((ulong)p.m[ 1] <<  8) | ((ulong)p.m[ 2] << 16) | ((ulong)p.m[ 3] << 24)
             | ((ulong)p.m[ 4] << 32) | ((ulong)p.m[ 5] << 40) | ((ulong)p.m[ 6] << 48) | ((ulong)p.m[ 7] << 56);
    ulong m1 = ((ulong)p.m[ 8])       | ((ulong)p.m[ 9] <<  8) | ((ulong)p.m[10] << 16) | ((ulong)p.m[11] << 24)
             | ((ulong)p.m[12] << 32) | ((ulong)p.m[13] << 40) | ((ulong)p.m[14] << 48) | ((ulong)p.m[15] << 56);
    ulong m2 = ((ulong)p.m[16])       | ((ulong)p.m[17] <<  8) | ((ulong)p.m[18] << 16) | ((ulong)p.m[19] << 24)
             | ((ulong)p.m[20] << 32) | ((ulong)p.m[21] << 40) | ((ulong)p.m[22] << 48) | ((ulong)p.m[23] << 56);
    ulong m3 = ((ulong)p.m[24])       | ((ulong)p.m[25] <<  8) | ((ulong)p.m[26] << 16) | ((ulong)p.m[27] << 24)
             | ((ulong)p.m[28] << 32) | ((ulong)p.m[29] << 40) | ((ulong)p.m[30] << 48) | ((ulong)p.m[31] << 56);

    // First Blake2b(m || LE(nonce)) for both nonces.
    ulong m_blk_A[16] = {m0,m1,m2,m3, nonce_A,0,0,0,0,0,0,0,0,0,0,0};
    ulong m_blk_B[16] = {m0,m1,m2,m3, nonce_B,0,0,0,0,0,0,0,0,0,0,0};
    ulong sh_A[4], sh_B[4];
    blake2b256_block(m_blk_A, 40u, sh_A);
    // Even if nonce B is past end, run anyway — its result is just XORed
    // into a guarded path. Branching here would hurt SIMD divergence;
    // doing the work and skipping the atomic XOR is cheaper.
    blake2b256_block(m_blk_B, 40u, sh_B);

    // Build the extended-byte words for both seed hashes.
    ulong A_eb0 = sh_A[0], A_eb1 = sh_A[1], A_eb2 = sh_A[2], A_eb3 = sh_A[3];
    ulong A_eb4 = sh_A[0] & 0xFFFFFFUL;
    ulong B_eb0 = sh_B[0], B_eb1 = sh_B[1], B_eb2 = sh_B[2], B_eb3 = sh_B[3];
    ulong B_eb4 = sh_B[0] & 0xFFFFFFUL;

    ulong sum_A[4] = {0,0,0,0};
    ulong sum_B[4] = {0,0,0,0};

    // Each iteration issues 2 random loads (one per nonce). The Apple
    // GPU schedules these as outstanding loads in parallel, doubling
    // memory-level parallelism per thread vs V1.
    #define STEP(i) do {                                                    \
        uint be_A = (uint)((SBYTE(A_eb0,A_eb1,A_eb2,A_eb3,A_eb4,i)   << 24) \
                         | (SBYTE(A_eb0,A_eb1,A_eb2,A_eb3,A_eb4,i+1) << 16) \
                         | (SBYTE(A_eb0,A_eb1,A_eb2,A_eb3,A_eb4,i+2) <<  8) \
                         |  SBYTE(A_eb0,A_eb1,A_eb2,A_eb3,A_eb4,i+3));      \
        uint be_B = (uint)((SBYTE(B_eb0,B_eb1,B_eb2,B_eb3,B_eb4,i)   << 24) \
                         | (SBYTE(B_eb0,B_eb1,B_eb2,B_eb3,B_eb4,i+1) << 16) \
                         | (SBYTE(B_eb0,B_eb1,B_eb2,B_eb3,B_eb4,i+2) <<  8) \
                         |  SBYTE(B_eb0,B_eb1,B_eb2,B_eb3,B_eb4,i+3));      \
        ulong idx_A = (ulong)be_A % p.n;                                    \
        ulong idx_B = (ulong)be_B % p.n;                                    \
        ulong4 row_A = R[idx_A];                                            \
        ulong4 row_B = R[idx_B];                                            \
        add256_v4(sum_A, row_A);                                            \
        add256_v4(sum_B, row_B);                                            \
    } while (0)

    STEP( 0); STEP( 1); STEP( 2); STEP( 3);
    STEP( 4); STEP( 5); STEP( 6); STEP( 7);
    STEP( 8); STEP( 9); STEP(10); STEP(11);
    STEP(12); STEP(13); STEP(14); STEP(15);
    STEP(16); STEP(17); STEP(18); STEP(19);
    STEP(20); STEP(21); STEP(22); STEP(23);
    STEP(24); STEP(25); STEP(26); STEP(27);
    STEP(28); STEP(29); STEP(30); STEP(31);

    // Second Blake2b for both
    ulong sum_blk_A[16] = {sum_A[0],sum_A[1],sum_A[2],sum_A[3], 0,0,0,0, 0,0,0,0, 0,0,0,0};
    ulong sum_blk_B[16] = {sum_B[0],sum_B[1],sum_B[2],sum_B[3], 0,0,0,0, 0,0,0,0, 0,0,0,0};
    ulong d_A[4], d_B[4];
    blake2b256_block(sum_blk_A, 32u, d_A);
    blake2b256_block(sum_blk_B, 32u, d_B);

    // XOR-accumulate. Always do nonce A; nonce B only if in range.
    atomic_fetch_xor_explicit(&acc[0], (uint)(d_A[0] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[1], (uint)(d_A[0] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[2], (uint)(d_A[1] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[3], (uint)(d_A[1] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[4], (uint)(d_A[2] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[5], (uint)(d_A[2] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[6], (uint)(d_A[3] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[7], (uint)(d_A[3] >> 32),          memory_order_relaxed);

    if (has_b) {
        atomic_fetch_xor_explicit(&acc[0], (uint)(d_B[0] & 0xFFFFFFFFUL), memory_order_relaxed);
        atomic_fetch_xor_explicit(&acc[1], (uint)(d_B[0] >> 32),          memory_order_relaxed);
        atomic_fetch_xor_explicit(&acc[2], (uint)(d_B[1] & 0xFFFFFFFFUL), memory_order_relaxed);
        atomic_fetch_xor_explicit(&acc[3], (uint)(d_B[1] >> 32),          memory_order_relaxed);
        atomic_fetch_xor_explicit(&acc[4], (uint)(d_B[2] & 0xFFFFFFFFUL), memory_order_relaxed);
        atomic_fetch_xor_explicit(&acc[5], (uint)(d_B[2] >> 32),          memory_order_relaxed);
        atomic_fetch_xor_explicit(&acc[6], (uint)(d_B[3] & 0xFFFFFFFFUL), memory_order_relaxed);
        atomic_fetch_xor_explicit(&acc[7], (uint)(d_B[3] >> 32),          memory_order_relaxed);
    }
}
