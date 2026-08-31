//
// V1: baseline Blake2b-256 kernel — one hash per thread, ulong (64-bit) state.
// Compiler emulates 64-bit ops as 32-bit limb pairs on Apple GPU SIMT.
//
// Input:  N × 32-byte messages (single-block Autolykos mining case)
// Output: N × 32-byte Blake2b-256 digests
//

#include <metal_stdlib>
using namespace metal;

// Blake2b initial vector (RFC 7693)
constant ulong IV[8] = {
    0x6a09e667f3bcc908UL, 0xbb67ae8584caa73bUL,
    0x3c6ef372fe94f82bUL, 0xa54ff53a5f1d36f1UL,
    0x510e527fade682d1UL, 0x9b05688c2b3e6c1fUL,
    0x1f83d9abfb41bd6bUL, 0x5be0cd19137e2179UL
};

// Sigma permutation: 12 rounds × 16 message word indices
constant uchar SIGMA[12][16] = {
    { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15},
    {14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3},
    {11, 8,12, 0, 5, 2,15,13,10,14, 3, 6, 7, 1, 9, 4},
    { 7, 9, 3, 1,13,12,11,14, 2, 6, 5,10, 4, 0,15, 8},
    { 9, 0, 5, 7, 2, 4,10,15,14, 1,11,12, 6, 8, 3,13},
    { 2,12, 6,10, 0,11, 8, 3, 4,13, 7, 5,15,14, 1, 9},
    {12, 5, 1,15,14,13, 4,10, 0, 7, 6, 3, 9, 2, 8,11},
    {13,11, 7,14,12, 1, 3, 9, 5, 0,15, 4, 8, 6, 2,10},
    { 6,15,14, 9,11, 3, 0, 8,12, 2,13, 7, 1, 4,10, 5},
    {10, 2, 8, 4, 7, 6, 1, 5,15,11, 9,14, 3,12,13, 0},
    { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15},
    {14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3},
};

static inline ulong rotr64(ulong x, uint n) {
    return (x >> n) | (x << (64 - n));
}

#define G(a, b, c, d, x, y) do {                                  \
    v[a] = v[a] + v[b] + (x);                                     \
    v[d] = rotr64(v[d] ^ v[a], 32);                               \
    v[c] = v[c] + v[d];                                           \
    v[b] = rotr64(v[b] ^ v[c], 24);                               \
    v[a] = v[a] + v[b] + (y);                                     \
    v[d] = rotr64(v[d] ^ v[a], 16);                               \
    v[c] = v[c] + v[d];                                           \
    v[b] = rotr64(v[b] ^ v[c], 63);                               \
} while (0)

kernel void blake2b256_v1(
    device   const uchar* inputs   [[buffer(0)]],   // N × 32 bytes
    device         uchar* outputs  [[buffer(1)]],   // N × 32 bytes
    constant       uint&  count    [[buffer(2)]],
    uint                  gid      [[thread_position_in_grid]]
) {
    if (gid >= count) return;

    // Load 32-byte message, pad with zeros to 128-byte block (16 ulongs).
    device const ulong* in_u64 = (device const ulong*)(inputs + (ulong)gid * 32);
    ulong m[16];
    m[0] = in_u64[0];
    m[1] = in_u64[1];
    m[2] = in_u64[2];
    m[3] = in_u64[3];
    for (uint i = 4; i < 16; i++) m[i] = 0;

    // Initial state h = IV ^ params; for Blake2b-256: digest=32, fanout=1, depth=1
    // → first param word = 0x01010000 | 32 = 0x01010020
    ulong h0 = IV[0] ^ 0x01010020UL;
    ulong h1 = IV[1];
    ulong h2 = IV[2];
    ulong h3 = IV[3];
    ulong h4 = IV[4];
    ulong h5 = IV[5];
    ulong h6 = IV[6];
    ulong h7 = IV[7];

    // Compression state v[0..16]
    ulong v[16];
    v[0]  = h0;          v[1]  = h1;
    v[2]  = h2;          v[3]  = h3;
    v[4]  = h4;          v[5]  = h5;
    v[6]  = h6;          v[7]  = h7;
    v[8]  = IV[0];       v[9]  = IV[1];
    v[10] = IV[2];       v[11] = IV[3];
    v[12] = IV[4] ^ 32UL;                       // t low = 32 bytes processed
    v[13] = IV[5];                              // t high = 0
    v[14] = IV[6] ^ 0xFFFFFFFFFFFFFFFFUL;       // f0 = last block
    v[15] = IV[7];

    // 12 rounds (loop form for V1; V2 unrolls these)
    for (uint r = 0; r < 12; r++) {
        constant uchar* s = SIGMA[r];
        G(0, 4,  8, 12, m[s[ 0]], m[s[ 1]]);
        G(1, 5,  9, 13, m[s[ 2]], m[s[ 3]]);
        G(2, 6, 10, 14, m[s[ 4]], m[s[ 5]]);
        G(3, 7, 11, 15, m[s[ 6]], m[s[ 7]]);
        G(0, 5, 10, 15, m[s[ 8]], m[s[ 9]]);
        G(1, 6, 11, 12, m[s[10]], m[s[11]]);
        G(2, 7,  8, 13, m[s[12]], m[s[13]]);
        G(3, 4,  9, 14, m[s[14]], m[s[15]]);
    }

    // Finalize: h[i] ^= v[i] ^ v[i+8]; take first 32 bytes
    h0 ^= v[0] ^ v[8];
    h1 ^= v[1] ^ v[9];
    h2 ^= v[2] ^ v[10];
    h3 ^= v[3] ^ v[11];

    device ulong* out = (device ulong*)(outputs + (ulong)gid * 32);
    out[0] = h0;
    out[1] = h1;
    out[2] = h2;
    out[3] = h3;
}
