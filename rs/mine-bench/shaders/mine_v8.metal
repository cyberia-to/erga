//
// V8 — dual R-table for channel parallelism test (A3).
//
// Two identical R-table copies live in two separate IOSurface blocks.
// Even-gid threads read from R_a, odd-gid threads read from R_b.
//
// Hypothesis: if Apple's LPDDR5X 8-channel memory subsystem places the
// two IOSurface allocations on different physical channels, splitting
// the access stream halves per-channel contention and may close some
// of the gap to the 78 MH/s theoretical ceiling.
//
// Risk: the OS may place both on the same channel, in which case V8
// performs identically to V1. Easy to verify empirically.
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
    ulong v1 = IV1, v2 = IV2, v3 = IV3, v4 = IV4, v5 = IV5, v6 = IV6, v7 = IV7;
    ulong v8 = IV0, v9 = IV1, vA = IV2, vB = IV3;
    ulong vC = IV4 ^ (ulong)t_bytes, vD = IV5;
    ulong vE = IV6 ^ 0xFFFFFFFFFFFFFFFFUL, vF = IV7;

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

kernel void mine_kernel_v8(
    device   const ulong4*     R_a        [[buffer(0)]],
    device   const ulong4*     R_b        [[buffer(3)]],   // second R copy
    device   atomic_uint*      acc        [[buffer(1)]],
    constant Params&           p          [[buffer(2)]],
    uint                       gid        [[thread_position_in_grid]]
) {
    if (gid >= p.count) return;
    ulong nonce = p.nonce_base + (ulong)gid;

    // Each thread picks one R copy based on gid parity. The two copies
    // hold identical data — split is purely to distribute memory
    // requests across potentially-different channels.
    device const ulong4* R = (gid & 1u) ? R_b : R_a;

    ulong m_blk[16];
    m_blk[0] = ((ulong)p.m[ 0]) | ((ulong)p.m[ 1] <<  8) | ((ulong)p.m[ 2] << 16) | ((ulong)p.m[ 3] << 24)
             | ((ulong)p.m[ 4] << 32) | ((ulong)p.m[ 5] << 40) | ((ulong)p.m[ 6] << 48) | ((ulong)p.m[ 7] << 56);
    m_blk[1] = ((ulong)p.m[ 8]) | ((ulong)p.m[ 9] <<  8) | ((ulong)p.m[10] << 16) | ((ulong)p.m[11] << 24)
             | ((ulong)p.m[12] << 32) | ((ulong)p.m[13] << 40) | ((ulong)p.m[14] << 48) | ((ulong)p.m[15] << 56);
    m_blk[2] = ((ulong)p.m[16]) | ((ulong)p.m[17] <<  8) | ((ulong)p.m[18] << 16) | ((ulong)p.m[19] << 24)
             | ((ulong)p.m[20] << 32) | ((ulong)p.m[21] << 40) | ((ulong)p.m[22] << 48) | ((ulong)p.m[23] << 56);
    m_blk[3] = ((ulong)p.m[24]) | ((ulong)p.m[25] <<  8) | ((ulong)p.m[26] << 16) | ((ulong)p.m[27] << 24)
             | ((ulong)p.m[28] << 32) | ((ulong)p.m[29] << 40) | ((ulong)p.m[30] << 48) | ((ulong)p.m[31] << 56);
    m_blk[4] = nonce;
    m_blk[5]=0; m_blk[6]=0; m_blk[7]=0; m_blk[8]=0; m_blk[9]=0;
    m_blk[10]=0; m_blk[11]=0; m_blk[12]=0; m_blk[13]=0; m_blk[14]=0; m_blk[15]=0;

    ulong seed_h[4];
    blake2b256_block(m_blk, 40u, seed_h);

    ulong eb0 = seed_h[0], eb1 = seed_h[1], eb2 = seed_h[2], eb3 = seed_h[3];
    ulong eb4 = seed_h[0] & 0xFFFFFFUL;

    #define SBYTE(k) (\
        ((k) <  8) ? ((eb0 >> ((k)      * 8)) & 0xFFUL) :  \
        ((k) < 16) ? ((eb1 >> (((k)-8)  * 8)) & 0xFFUL) :  \
        ((k) < 24) ? ((eb2 >> (((k)-16) * 8)) & 0xFFUL) :  \
        ((k) < 32) ? ((eb3 >> (((k)-24) * 8)) & 0xFFUL) :  \
                     ((eb4 >> (((k)-32) * 8)) & 0xFFUL) )

    #define LOAD_AND_ADD(i) do {                                   \
        uint be = (uint)((SBYTE(i  ) << 24) |                      \
                         (SBYTE(i+1) << 16) |                      \
                         (SBYTE(i+2) <<  8) |                      \
                          SBYTE(i+3));                             \
        ulong idx = (ulong)be % p.n;                               \
        add256_v4(sum, R[idx]);                                    \
    } while (0)

    ulong sum[4] = {0,0,0,0};
    LOAD_AND_ADD( 0); LOAD_AND_ADD( 1); LOAD_AND_ADD( 2); LOAD_AND_ADD( 3);
    LOAD_AND_ADD( 4); LOAD_AND_ADD( 5); LOAD_AND_ADD( 6); LOAD_AND_ADD( 7);
    LOAD_AND_ADD( 8); LOAD_AND_ADD( 9); LOAD_AND_ADD(10); LOAD_AND_ADD(11);
    LOAD_AND_ADD(12); LOAD_AND_ADD(13); LOAD_AND_ADD(14); LOAD_AND_ADD(15);
    LOAD_AND_ADD(16); LOAD_AND_ADD(17); LOAD_AND_ADD(18); LOAD_AND_ADD(19);
    LOAD_AND_ADD(20); LOAD_AND_ADD(21); LOAD_AND_ADD(22); LOAD_AND_ADD(23);
    LOAD_AND_ADD(24); LOAD_AND_ADD(25); LOAD_AND_ADD(26); LOAD_AND_ADD(27);
    LOAD_AND_ADD(28); LOAD_AND_ADD(29); LOAD_AND_ADD(30); LOAD_AND_ADD(31);

    ulong sum_blk[16] = {sum[0],sum[1],sum[2],sum[3], 0,0,0,0, 0,0,0,0, 0,0,0,0};
    ulong d[4];
    blake2b256_block(sum_blk, 32u, d);

    atomic_fetch_xor_explicit(&acc[0], (uint)(d[0] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[1], (uint)(d[0] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[2], (uint)(d[1] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[3], (uint)(d[1] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[4], (uint)(d[2] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[5], (uint)(d[2] >> 32),          memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[6], (uint)(d[3] & 0xFFFFFFFFUL), memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[7], (uint)(d[3] >> 32),          memory_order_relaxed);
}
