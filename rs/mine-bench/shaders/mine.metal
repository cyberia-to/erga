//
// Phase 3 — Autolykos v2 mining kernel.
//
// Per-thread (one nonce):
//   seed_hash    = Blake2b256(m[32] || LE(nonce))         // 40 → 32
//   extended[35] = seed_hash || seed_hash[0..3]
//   for i in 0..32: idx[i] = BE_u32(extended[i..i+4]) mod N
//   sum_256      = Σ R[idx[i]] mod 2^256                  // 32 random reads
//   d            = Blake2b256(sum_256)                    // 32 → 32
//   XOR-accumulate d into the 256-bit atomic accumulator
//
// The accumulator XOR is meaningless for actual mining (which would be
// a target compare + nonce report) but is a faithful end-to-end load
// and gives us a byte-exact value to compare against the CPU reference.
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

// Compress one 128-byte block. `m` is the 16-ulong block (caller must
// have zero-padded the unused suffix). `t_bytes` is the total message
// length so far (32 or 40 for our two call sites). Writes the first
// 4 ulongs of the Blake2b state into `h_out`.
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

// 256-bit (4 × ulong limbs, little-endian) addition with carry, modulo 2^256.
static inline void add256(thread ulong* sum, ulong r0, ulong r1, ulong r2, ulong r3) {
    ulong t = sum[0] + r0;
    ulong c = (t < sum[0]) ? 1UL : 0UL;
    sum[0] = t;

    t = sum[1] + r1;
    ulong c1 = (t < sum[1]) ? 1UL : 0UL;
    ulong t2 = t + c;
    ulong c2 = (t2 < t) ? 1UL : 0UL;
    sum[1] = t2;
    c = c1 + c2;

    t = sum[2] + r2;
    c1 = (t < sum[2]) ? 1UL : 0UL;
    t2 = t + c;
    c2 = (t2 < t) ? 1UL : 0UL;
    sum[2] = t2;
    c = c1 + c2;

    t = sum[3] + r3;
    t2 = t + c;
    sum[3] = t2; // carry out discarded (mod 2^256)
}

kernel void mine_kernel(
    device   const ulong*      R          [[buffer(0)]],
    device   atomic_uint*      acc        [[buffer(1)]],   // 8 × u32 = 32 bytes
    constant Params&           p          [[buffer(2)]],
    uint                       gid        [[thread_position_in_grid]]
) {
    if (gid >= p.count) return;
    ulong nonce = p.nonce_base + (ulong)gid;

    // Build 40-byte Blake2b input: m[32] || LE(nonce, 8)
    // Pack into 16 ulongs (128-byte block); bytes 40..128 are zero.
    ulong m_blk[16];
    // Bytes 0..32 of the block are p.m[0..32]. Read as 4 LE ulongs.
    m_blk[0] = ((ulong)p.m[ 0]) | ((ulong)p.m[ 1] <<  8) | ((ulong)p.m[ 2] << 16) | ((ulong)p.m[ 3] << 24)
             | ((ulong)p.m[ 4] << 32) | ((ulong)p.m[ 5] << 40) | ((ulong)p.m[ 6] << 48) | ((ulong)p.m[ 7] << 56);
    m_blk[1] = ((ulong)p.m[ 8]) | ((ulong)p.m[ 9] <<  8) | ((ulong)p.m[10] << 16) | ((ulong)p.m[11] << 24)
             | ((ulong)p.m[12] << 32) | ((ulong)p.m[13] << 40) | ((ulong)p.m[14] << 48) | ((ulong)p.m[15] << 56);
    m_blk[2] = ((ulong)p.m[16]) | ((ulong)p.m[17] <<  8) | ((ulong)p.m[18] << 16) | ((ulong)p.m[19] << 24)
             | ((ulong)p.m[20] << 32) | ((ulong)p.m[21] << 40) | ((ulong)p.m[22] << 48) | ((ulong)p.m[23] << 56);
    m_blk[3] = ((ulong)p.m[24]) | ((ulong)p.m[25] <<  8) | ((ulong)p.m[26] << 16) | ((ulong)p.m[27] << 24)
             | ((ulong)p.m[28] << 32) | ((ulong)p.m[29] << 40) | ((ulong)p.m[30] << 48) | ((ulong)p.m[31] << 56);
    m_blk[4] = nonce;
    m_blk[5]  = 0; m_blk[6]  = 0; m_blk[7]  = 0;
    m_blk[8]  = 0; m_blk[9]  = 0; m_blk[10] = 0; m_blk[11] = 0;
    m_blk[12] = 0; m_blk[13] = 0; m_blk[14] = 0; m_blk[15] = 0;

    ulong seed_h[4];
    blake2b256_block(m_blk, 40u, seed_h);

    // Convert seed_h (4 ulongs, LE) to seed_bytes[32] for byte indexing.
    // Then form 35-byte extended buffer and read 32 big-endian u32s.
    // We can read bytes from seed_h via shifts.
    // Helper: byte k of seed_h is (seed_h[k/8] >> ((k%8)*8)) & 0xFF.
    // For genIndexes we don't need the array form — compute indexes
    // directly from the seed_h words.

    // Pack the 32 seed bytes + 3 wraparound bytes into a 5-ulong buffer
    // shifted so byte k is at byte position k of the buffer.
    ulong eb0 = seed_h[0];
    ulong eb1 = seed_h[1];
    ulong eb2 = seed_h[2];
    ulong eb3 = seed_h[3];
    // eb4 has only the low 3 bytes valid (extended[32..35]).
    ulong eb4 = seed_h[0] & 0xFFFFFFUL;

    // Sum accumulator (little-endian limbs).
    ulong sum[4] = {0UL, 0UL, 0UL, 0UL};

    // For each i in 0..32, read 4 bytes [i..i+4] as big-endian.
    // The 4-byte window spans at most two adjacent ulongs.
    // We extract via combined shifts.
    // Define seed_byte(k):
    //   k <  8 → (eb0 >> (k*8)) & 0xFF
    //   k < 16 → (eb1 >> ((k-8)*8)) & 0xFF
    //   k < 24 → (eb2 >> ((k-16)*8)) & 0xFF
    //   k < 32 → (eb3 >> ((k-24)*8)) & 0xFF
    //   k < 35 → (eb4 >> ((k-32)*8)) & 0xFF
    // Unrolled, with idx mod N done inline.
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
        ulong off = idx * 4UL;                                     \
        ulong r0 = R[off + 0];                                     \
        ulong r1 = R[off + 1];                                     \
        ulong r2 = R[off + 2];                                     \
        ulong r3 = R[off + 3];                                     \
        add256(sum, r0, r1, r2, r3);                               \
    } while (0)

    LOAD_AND_ADD( 0); LOAD_AND_ADD( 1); LOAD_AND_ADD( 2); LOAD_AND_ADD( 3);
    LOAD_AND_ADD( 4); LOAD_AND_ADD( 5); LOAD_AND_ADD( 6); LOAD_AND_ADD( 7);
    LOAD_AND_ADD( 8); LOAD_AND_ADD( 9); LOAD_AND_ADD(10); LOAD_AND_ADD(11);
    LOAD_AND_ADD(12); LOAD_AND_ADD(13); LOAD_AND_ADD(14); LOAD_AND_ADD(15);
    LOAD_AND_ADD(16); LOAD_AND_ADD(17); LOAD_AND_ADD(18); LOAD_AND_ADD(19);
    LOAD_AND_ADD(20); LOAD_AND_ADD(21); LOAD_AND_ADD(22); LOAD_AND_ADD(23);
    LOAD_AND_ADD(24); LOAD_AND_ADD(25); LOAD_AND_ADD(26); LOAD_AND_ADD(27);
    LOAD_AND_ADD(28); LOAD_AND_ADD(29); LOAD_AND_ADD(30); LOAD_AND_ADD(31);

    // Second Blake2b256 over the 32-byte sum.
    ulong sum_blk[16];
    sum_blk[0] = sum[0];
    sum_blk[1] = sum[1];
    sum_blk[2] = sum[2];
    sum_blk[3] = sum[3];
    sum_blk[4]  = 0; sum_blk[5]  = 0; sum_blk[6]  = 0; sum_blk[7]  = 0;
    sum_blk[8]  = 0; sum_blk[9]  = 0; sum_blk[10] = 0; sum_blk[11] = 0;
    sum_blk[12] = 0; sum_blk[13] = 0; sum_blk[14] = 0; sum_blk[15] = 0;
    ulong d[4];
    blake2b256_block(sum_blk, 32u, d);

    // XOR-accumulate d into the 256-bit accumulator (8 × atomic_uint).
    uint d0_lo = (uint)(d[0] & 0xFFFFFFFFUL);
    uint d0_hi = (uint)(d[0] >> 32);
    uint d1_lo = (uint)(d[1] & 0xFFFFFFFFUL);
    uint d1_hi = (uint)(d[1] >> 32);
    uint d2_lo = (uint)(d[2] & 0xFFFFFFFFUL);
    uint d2_hi = (uint)(d[2] >> 32);
    uint d3_lo = (uint)(d[3] & 0xFFFFFFFFUL);
    uint d3_hi = (uint)(d[3] >> 32);
    atomic_fetch_xor_explicit(&acc[0], d0_lo, memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[1], d0_hi, memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[2], d1_lo, memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[3], d1_hi, memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[4], d2_lo, memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[5], d2_hi, memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[6], d3_lo, memory_order_relaxed);
    atomic_fetch_xor_explicit(&acc[7], d3_hi, memory_order_relaxed);
}
