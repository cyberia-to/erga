//
// V4: V3 (dual hash per thread) + per-rotation primitives.
//
// Blake2b uses only rotr64 by {16, 24, 32, 63}. We give each one its own
// inline function so the Metal compiler can recognize patterns:
//   - rotr64 by 32: high/low half-word swap (mov + mov)
//   - rotr64 by 16: byte shuffle (potentially via bit-extract)
//   - rotr64 by 63: equivalent to rotl by 1 — single shift+or
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

// rotr64 by 32 is high/low half-swap.
static inline ulong rotr32(ulong x) {
    return (x >> 32) | (x << 32);
}

// rotr64 by 24
static inline ulong rotr24(ulong x) {
    return (x >> 24) | (x << 40);
}

// rotr64 by 16: byte shuffle pattern.
static inline ulong rotr16(ulong x) {
    return (x >> 16) | (x << 48);
}

// rotr64 by 63 = rotl by 1.
static inline ulong rotl1(ulong x) {
    return (x << 1) | (x >> 63);
}

#define G2(suf, a, b, c, d, x, y) do {                            \
    v##a##suf = v##a##suf + v##b##suf + (x);                      \
    v##d##suf = rotr32(v##d##suf ^ v##a##suf);                    \
    v##c##suf = v##c##suf + v##d##suf;                            \
    v##b##suf = rotr24(v##b##suf ^ v##c##suf);                    \
    v##a##suf = v##a##suf + v##b##suf + (y);                      \
    v##d##suf = rotr16(v##d##suf ^ v##a##suf);                    \
    v##c##suf = v##c##suf + v##d##suf;                            \
    v##b##suf = rotl1 (v##b##suf ^ v##c##suf);                    \
} while (0)

#define ROUND2(s0,s1,s2,s3,s4,s5,s6,s7,s8,s9,sa,sb,sc,sd,se,sf)   \
    G2(A, 0, 4,  8, 12, mA##s0, mA##s1);                          \
    G2(B, 0, 4,  8, 12, mB##s0, mB##s1);                          \
    G2(A, 1, 5,  9, 13, mA##s2, mA##s3);                          \
    G2(B, 1, 5,  9, 13, mB##s2, mB##s3);                          \
    G2(A, 2, 6, 10, 14, mA##s4, mA##s5);                          \
    G2(B, 2, 6, 10, 14, mB##s4, mB##s5);                          \
    G2(A, 3, 7, 11, 15, mA##s6, mA##s7);                          \
    G2(B, 3, 7, 11, 15, mB##s6, mB##s7);                          \
    G2(A, 0, 5, 10, 15, mA##s8, mA##s9);                          \
    G2(B, 0, 5, 10, 15, mB##s8, mB##s9);                          \
    G2(A, 1, 6, 11, 12, mA##sa, mA##sb);                          \
    G2(B, 1, 6, 11, 12, mB##sa, mB##sb);                          \
    G2(A, 2, 7,  8, 13, mA##sc, mA##sd);                          \
    G2(B, 2, 7,  8, 13, mB##sc, mB##sd);                          \
    G2(A, 3, 4,  9, 14, mA##se, mA##sf);                          \
    G2(B, 3, 4,  9, 14, mB##se, mB##sf)

kernel void blake2b256_v4(
    device   const uchar* inputs   [[buffer(0)]],
    device         uchar* outputs  [[buffer(1)]],
    constant       uint&  count    [[buffer(2)]],
    uint                  gid      [[thread_position_in_grid]]
) {
    uint idxA = gid * 2u;
    uint idxB = idxA + 1u;
    if (idxA >= count) return;

    device const ulong* inA = (device const ulong*)(inputs + (ulong)idxA * 32);
    ulong mA0 = inA[0], mA1 = inA[1], mA2 = inA[2], mA3 = inA[3];
    const ulong mA4=0, mA5=0, mA6=0, mA7=0, mA8=0, mA9=0;
    const ulong mA10=0, mA11=0, mA12=0, mA13=0, mA14=0, mA15=0;

    bool has_b = idxB < count;
    device const ulong* inB = (device const ulong*)(inputs + (ulong)(has_b ? idxB : idxA) * 32);
    ulong mB0 = inB[0], mB1 = inB[1], mB2 = inB[2], mB3 = inB[3];
    const ulong mB4=0, mB5=0, mB6=0, mB7=0, mB8=0, mB9=0;
    const ulong mB10=0, mB11=0, mB12=0, mB13=0, mB14=0, mB15=0;

    ulong v0A=IV0^0x01010020UL, v1A=IV1, v2A=IV2, v3A=IV3;
    ulong v4A=IV4, v5A=IV5, v6A=IV6, v7A=IV7;
    ulong v8A=IV0, v9A=IV1, v10A=IV2, v11A=IV3;
    ulong v12A=IV4^32UL, v13A=IV5, v14A=IV6^0xFFFFFFFFFFFFFFFFUL, v15A=IV7;

    ulong v0B=IV0^0x01010020UL, v1B=IV1, v2B=IV2, v3B=IV3;
    ulong v4B=IV4, v5B=IV5, v6B=IV6, v7B=IV7;
    ulong v8B=IV0, v9B=IV1, v10B=IV2, v11B=IV3;
    ulong v12B=IV4^32UL, v13B=IV5, v14B=IV6^0xFFFFFFFFFFFFFFFFUL, v15B=IV7;

    ROUND2( 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15);
    ROUND2(14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3);
    ROUND2(11, 8,12, 0, 5, 2,15,13,10,14, 3, 6, 7, 1, 9, 4);
    ROUND2( 7, 9, 3, 1,13,12,11,14, 2, 6, 5,10, 4, 0,15, 8);
    ROUND2( 9, 0, 5, 7, 2, 4,10,15,14, 1,11,12, 6, 8, 3,13);
    ROUND2( 2,12, 6,10, 0,11, 8, 3, 4,13, 7, 5,15,14, 1, 9);
    ROUND2(12, 5, 1,15,14,13, 4,10, 0, 7, 6, 3, 9, 2, 8,11);
    ROUND2(13,11, 7,14,12, 1, 3, 9, 5, 0,15, 4, 8, 6, 2,10);
    ROUND2( 6,15,14, 9,11, 3, 0, 8,12, 2,13, 7, 1, 4,10, 5);
    ROUND2(10, 2, 8, 4, 7, 6, 1, 5,15,11, 9,14, 3,12,13, 0);
    ROUND2( 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,10,11,12,13,14,15);
    ROUND2(14,10, 4, 8, 9,15,13, 6, 1,12, 0, 2,11, 7, 5, 3);

    device ulong* outA = (device ulong*)(outputs + (ulong)idxA * 32);
    outA[0] = (IV0 ^ 0x01010020UL) ^ v0A ^ v8A;
    outA[1] =  IV1                  ^ v1A ^ v9A;
    outA[2] =  IV2                  ^ v2A ^ v10A;
    outA[3] =  IV3                  ^ v3A ^ v11A;

    if (has_b) {
        device ulong* outB = (device ulong*)(outputs + (ulong)idxB * 32);
        outB[0] = (IV0 ^ 0x01010020UL) ^ v0B ^ v8B;
        outB[1] =  IV1                  ^ v1B ^ v9B;
        outB[2] =  IV2                  ^ v2B ^ v10B;
        outB[3] =  IV3                  ^ v3B ^ v11B;
    }
}
