//
// V2: fully unrolled rounds, SIGMA inlined as compile-time message indices.
// Compared to V1, removes:
//  - 12-iteration loop overhead
//  - SIGMA[r][k] array indexing (replaced by direct m[<literal>] accesses)
//
// One hash per thread, ulong (64-bit) state.
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

static inline ulong rotr64(ulong x, uint n) {
    return (x >> n) | (x << (64 - n));
}

#define G(a, b, c, d, x, y) do {                                  \
    v##a = v##a + v##b + (x);                                     \
    v##d = rotr64(v##d ^ v##a, 32);                               \
    v##c = v##c + v##d;                                           \
    v##b = rotr64(v##b ^ v##c, 24);                               \
    v##a = v##a + v##b + (y);                                     \
    v##d = rotr64(v##d ^ v##a, 16);                               \
    v##c = v##c + v##d;                                           \
    v##b = rotr64(v##b ^ v##c, 63);                               \
} while (0)

#define ROUND(s0,s1,s2,s3,s4,s5,s6,s7,s8,s9,sa,sb,sc,sd,se,sf)    \
    G(0, 4,  8, 12, m##s0, m##s1);                                \
    G(1, 5,  9, 13, m##s2, m##s3);                                \
    G(2, 6, 10, 14, m##s4, m##s5);                                \
    G(3, 7, 11, 15, m##s6, m##s7);                                \
    G(0, 5, 10, 15, m##s8, m##s9);                                \
    G(1, 6, 11, 12, m##sa, m##sb);                                \
    G(2, 7,  8, 13, m##sc, m##sd);                                \
    G(3, 4,  9, 14, m##se, m##sf)

kernel void blake2b256_v2(
    device   const uchar* inputs   [[buffer(0)]],
    device         uchar* outputs  [[buffer(1)]],
    constant       uint&  count    [[buffer(2)]],
    uint                  gid      [[thread_position_in_grid]]
) {
    if (gid >= count) return;

    // 32-byte input padded with zeros to 128-byte block.
    device const ulong* in_u64 = (device const ulong*)(inputs + (ulong)gid * 32);
    ulong m0  = in_u64[0];
    ulong m1  = in_u64[1];
    ulong m2  = in_u64[2];
    ulong m3  = in_u64[3];
    const ulong m4  = 0UL, m5  = 0UL, m6  = 0UL, m7  = 0UL;
    const ulong m8  = 0UL, m9  = 0UL, m10 = 0UL, m11 = 0UL;
    const ulong m12 = 0UL, m13 = 0UL, m14 = 0UL, m15 = 0UL;

    // Blake2b-256 initial chaining value: h = IV with first word XORed
    // by params = digest_length=32, fanout=1, depth=1 → 0x01010020.
    ulong v0  = IV0 ^ 0x01010020UL;
    ulong v1  = IV1;
    ulong v2  = IV2;
    ulong v3  = IV3;
    ulong v4  = IV4;
    ulong v5  = IV5;
    ulong v6  = IV6;
    ulong v7  = IV7;
    ulong v8  = IV0;
    ulong v9  = IV1;
    ulong v10 = IV2;
    ulong v11 = IV3;
    ulong v12 = IV4 ^ 32UL;
    ulong v13 = IV5;
    ulong v14 = IV6 ^ 0xFFFFFFFFFFFFFFFFUL;
    ulong v15 = IV7;

    // 12 explicit rounds; SIGMA indices baked into token-pasted m-word names.
    ROUND( 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15);
    ROUND(14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3);
    ROUND(11, 8,12, 0, 5, 2,15,13,10,14, 3, 6, 7, 1, 9, 4);
    ROUND( 7, 9, 3, 1,13,12,11,14, 2, 6, 5,10, 4, 0,15, 8);
    ROUND( 9, 0, 5, 7, 2, 4,10,15,14, 1,11,12, 6, 8, 3,13);
    ROUND( 2,12, 6,10, 0,11, 8, 3, 4,13, 7, 5,15,14, 1, 9);
    ROUND(12, 5, 1,15,14,13, 4,10, 0, 7, 6, 3, 9, 2, 8,11);
    ROUND(13,11, 7,14,12, 1, 3, 9, 5, 0,15, 4, 8, 6, 2,10);
    ROUND( 6,15,14, 9,11, 3, 0, 8,12, 2,13, 7, 1, 4,10, 5);
    ROUND(10, 2, 8, 4, 7, 6, 1, 5,15,11, 9,14, 3,12,13, 0);
    ROUND( 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15);
    ROUND(14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3);

    // Finalize: h[i] ^= v[i] ^ v[i+8]; first 32 bytes only.
    ulong h0 = (IV0 ^ 0x01010020UL) ^ v0 ^ v8;
    ulong h1 =  IV1                  ^ v1 ^ v9;
    ulong h2 =  IV2                  ^ v2 ^ v10;
    ulong h3 =  IV3                  ^ v3 ^ v11;

    device ulong* out = (device ulong*)(outputs + (ulong)gid * 32);
    out[0] = h0;
    out[1] = h1;
    out[2] = h2;
    out[3] = h3;
}
