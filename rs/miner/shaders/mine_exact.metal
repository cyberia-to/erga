// Protocol-exact Autolykos v2 mining kernel.
//
// Per nonce, byte-for-byte with crates/autolykos (chain-verified):
//   h1   = Blake2b256(msg[32] || nonce_be[8])
//   i    = BE(h1[24..32]) mod N
//   f    = R[i]                                   (31-byte element)
//   seed = Blake2b256(f_be31 || msg || nonce_be)
//   idx  = genIndexes(seed, N)                    (32 indexes)
//   sum  = Σ R[idx[j]]                            (256-bit)
//   hit  = Blake2b256(be32(sum))
// then hit is compared big-endian against the target.
//
// R rows are stored as 4 little-endian u64 limbs of the element value
// (limb0 = low 64 bits). The diff kernel emits the 32 big-endian hit
// bytes so a CPU differential test can gate this against the reference.

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

struct Params { ulong n; ulong nonce_base; uint count; uint _pad; };

static inline ulong rotr64(ulong x, uint n) { return (x >> n) | (x << (64 - n)); }
static inline ulong swap64(ulong x) {
    return ((x & 0x00000000000000FFUL) << 56) | ((x & 0x000000000000FF00UL) << 40)
         | ((x & 0x0000000000FF0000UL) << 24) | ((x & 0x00000000FF000000UL) << 8)
         | ((x & 0x000000FF00000000UL) >> 8)  | ((x & 0x0000FF0000000000UL) >> 24)
         | ((x & 0x00FF000000000000UL) >> 40) | ((x & 0xFF00000000000000UL) >> 56);
}

#define G(va, vb, vc, vd, x, y) do {         \
    va = va + vb + (x); vd = rotr64(vd ^ va, 32); \
    vc = vc + vd;       vb = rotr64(vb ^ vc, 24); \
    va = va + vb + (y); vd = rotr64(vd ^ va, 16); \
    vc = vc + vd;       vb = rotr64(vb ^ vc, 63); \
} while (0)

#define R12(m, s0,s1,s2,s3,s4,s5,s6,s7,s8,s9,sa,sb,sc,sd,se,sf, \
            v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF) do { \
    G(v0,v4,v8,vC, m[s0],m[s1]); G(v1,v5,v9,vD, m[s2],m[s3]); \
    G(v2,v6,vA,vE, m[s4],m[s5]); G(v3,v7,vB,vF, m[s6],m[s7]); \
    G(v0,v5,vA,vF, m[s8],m[s9]); G(v1,v6,vB,vC, m[sa],m[sb]); \
    G(v2,v7,v8,vD, m[sc],m[sd]); G(v3,v4,v9,vE, m[se],m[sf]); \
} while (0)

// One-block Blake2b-256 (message ≤128 B, `t` = real byte length). Writes the
// 4 state words (little-endian digest limbs) to h_out.
static inline void blake2b_block(thread ulong* m, uint t, thread ulong* h_out) {
    ulong v0=IV0^0x01010020UL, v1=IV1, v2=IV2, v3=IV3, v4=IV4, v5=IV5, v6=IV6, v7=IV7;
    ulong v8=IV0, v9=IV1, vA=IV2, vB=IV3, vC=IV4^(ulong)t, vD=IV5, vE=IV6^0xFFFFFFFFFFFFFFFFUL, vF=IV7;
    R12(m, 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 14,10,4,8,9,15,13,6,1,12,0,2,11,7,5,3, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 11,8,12,0,5,2,15,13,10,14,3,6,7,1,9,4, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 7,9,3,1,13,12,11,14,2,6,5,10,4,0,15,8, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 9,0,5,7,2,4,10,15,14,1,11,12,6,8,3,13, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 2,12,6,10,0,11,8,3,4,13,7,5,15,14,1,9, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 12,5,1,15,14,13,4,10,0,7,6,3,9,2,8,11, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 13,11,7,14,12,1,3,9,5,0,15,4,8,6,2,10, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 6,15,14,9,11,3,0,8,12,2,13,7,1,4,10,5, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 10,2,8,4,7,6,1,5,15,11,9,14,3,12,13,0, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 14,10,4,8,9,15,13,6,1,12,0,2,11,7,5,3, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    h_out[0] = (IV0^0x01010020UL) ^ v0 ^ v8;
    h_out[1] = IV1 ^ v1 ^ v9;
    h_out[2] = IV2 ^ v2 ^ vA;
    h_out[3] = IV3 ^ v3 ^ vB;
}

static inline void add256(thread ulong* s, ulong r0, ulong r1, ulong r2, ulong r3) {
    ulong t = s[0]+r0; ulong c = (t<s[0])?1UL:0UL; s[0]=t;
    t = s[1]+r1; ulong c1=(t<s[1])?1UL:0UL; ulong t2=t+c; ulong c2=(t2<t)?1UL:0UL; s[1]=t2; c=c1+c2;
    t = s[2]+r2; c1=(t<s[2])?1UL:0UL; t2=t+c; c2=(t2<t)?1UL:0UL; s[2]=t2; c=c1+c2;
    t = s[3]+r3; t2=t+c; s[3]=t2;
}

// Streaming Blake2b-256 compress: update 8-word state with one 128-byte
// block. `t` = bytes hashed through this block; `fin` = last block.
static inline void blake2b_compress(thread ulong* hs, thread ulong* m, ulong t, bool fin) {
    ulong v0=hs[0],v1=hs[1],v2=hs[2],v3=hs[3],v4=hs[4],v5=hs[5],v6=hs[6],v7=hs[7];
    ulong v8=IV0,v9=IV1,vA=IV2,vB=IV3,vC=IV4^t,vD=IV5,vE=IV6^(fin?0xFFFFFFFFFFFFFFFFUL:0UL),vF=IV7;
    R12(m, 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 14,10,4,8,9,15,13,6,1,12,0,2,11,7,5,3, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 11,8,12,0,5,2,15,13,10,14,3,6,7,1,9,4, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 7,9,3,1,13,12,11,14,2,6,5,10,4,0,15,8, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 9,0,5,7,2,4,10,15,14,1,11,12,6,8,3,13, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 2,12,6,10,0,11,8,3,4,13,7,5,15,14,1,9, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 12,5,1,15,14,13,4,10,0,7,6,3,9,2,8,11, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 13,11,7,14,12,1,3,9,5,0,15,4,8,6,2,10, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 6,15,14,9,11,3,0,8,12,2,13,7,1,4,10,5, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 10,2,8,4,7,6,1,5,15,11,9,14,3,12,13,0, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    R12(m, 14,10,4,8,9,15,13,6,1,12,0,2,11,7,5,3, v0,v1,v2,v3,v4,v5,v6,v7,v8,v9,vA,vB,vC,vD,vE,vF);
    hs[0]^=v0^v8; hs[1]^=v1^v9; hs[2]^=v2^vA; hs[3]^=v3^vB;
    hs[4]^=v4^vC; hs[5]^=v5^vD; hs[6]^=v6^vE; hs[7]^=v7^vF;
}

// `base` is the first row of this dispatch: the build runs in pieces so
// the window can show how far along it is.
struct BuildParams { uint n; uint height; uint base; };

// Build R: each thread computes genElement(idx,h,M) = Blake2b256(idx_be4 ||
// h_be4 || M)[1..32] and writes it as 4 little-endian limbs.
kernel void build_kernel(
    device ulong*         R   [[buffer(0)]],
    constant BuildParams& bp  [[buffer(1)]],
    uint gid [[thread_position_in_grid]])
{
    uint idx = bp.base + gid; if (idx >= bp.n) return;
    // The message is idx(4) || height(4) || M(8192), where M is the constant
    // pad: 1024 big-endian u64s counting 0..1023. Every 8-byte word of that
    // pad is therefore *computable* — the byte-swap of its index — so this
    // kernel never reads M at all. The previous version assembled each word
    // byte by byte out of `constant` memory, which cost ~8 KB of loads per
    // element: 1.86 TB across a full table, and the memory system, not
    // Blake2b, was the wall.
    ulong hs[8] = { IV0^0x01010020UL, IV1, IV2, IV3, IV4, IV5, IV6, IV7 };
    const uint T = 8200u; // 4 + 4 + 8192
    const ulong pre = swap64(((ulong)idx << 32) | (ulong)bp.height);
    for (uint b=0; b<65u; b++) {
        ulong mm[16];
        for (uint w=0; w<16; w++) {
            uint k0 = b*128u + w*8u;                 // byte offset of this word
            // The pad index is at most 1023 within the message, so only two
            // of its eight big-endian bytes are ever non-zero; placing them
            // directly is cheaper than a general swap64. Measured at ~1%:
            // the Metal compiler was already folding most of it. Kept because
            // it states the intent, not because it bought much.
            uint q = b*16u + w - 1u;
            mm[w] = (k0 == 0u)     ? pre             // idx || height
                  : (k0 + 8u <= T) ? (((ulong)(q & 0xffu) << 56) | ((ulong)(q >> 8) << 48))
                                   : 0UL;           // past the message: zero pad
        }
        ulong t = ((b+1u)*128u < T) ? (ulong)((b+1u)*128u) : (ulong)T;
        blake2b_compress(hs, mm, t, b==64u);
    }
    // digest byte j = hs[j/8] >> ((j%8)*8); value = be32 where be32[0]=0,
    // be32[1..32]=digest[1..32]. limbs: o[0]=BE(be32[24..32]) … o[3]=BE(be32[0..8]).
    device ulong* o = R + (ulong)idx*4;
    for (uint L=0; L<4; L++) {
        uint base = (3u-L)*8u;
        ulong x=0;
        for (uint i=0;i<8;i++) {
            uint j = base+i;
            uchar be = (j==0u) ? (uchar)0 : (uchar)(hs[j/8] >> ((j%8)*8));
            x = (x<<8) | (ulong)be;
        }
        o[L] = x;
    }
}

static inline void hit_words(
    device const ulong* R, constant uchar* msg, ulong n, ulong nonce, thread ulong* d)
{
    // block1 = msg[32] || nonce_be[8]
    ulong b1[16];
    for (uint w=0; w<4; w++) {
        ulong x=0; for (uint b=0;b<8;b++) x |= ((ulong)msg[8*w+b]) << (8*b); b1[w]=x;
    }
    b1[4] = swap64(nonce);            // nonce as big-endian bytes, read LE
    for (uint w=5; w<16; w++) b1[w]=0;
    ulong h1[4]; blake2b_block(b1, 40u, h1);

    ulong i = swap64(h1[3]) % n;      // BE(h1[24..32]) mod N
    device const ulong* fr = R + i*4; // R[i]
    ulong f0=fr[0], f1=fr[1], f2=fr[2], f3=fr[3];

    // seed = Blake2b256(f_be31 || msg || nonce_be)  (71 bytes)
    uchar sbuf[72];
    uchar fbe[32];
    for (uint k=0;k<8;k++) fbe[k]    = (uchar)(f3 >> (56 - 8*k));
    for (uint k=0;k<8;k++) fbe[8+k]  = (uchar)(f2 >> (56 - 8*k));
    for (uint k=0;k<8;k++) fbe[16+k] = (uchar)(f1 >> (56 - 8*k));
    for (uint k=0;k<8;k++) fbe[24+k] = (uchar)(f0 >> (56 - 8*k));
    for (uint k=0;k<31;k++) sbuf[k]     = fbe[1+k];     // drop leading byte
    for (uint k=0;k<32;k++) sbuf[31+k]  = msg[k];
    for (uint k=0;k<8;k++)  sbuf[63+k]  = (uchar)(nonce >> (56 - 8*k));
    sbuf[71]=0;
    ulong sblk[16];
    for (uint w=0; w<16; w++) {
        ulong x=0; for (uint b=0;b<8;b++) { uint p=8*w+b; x |= ((ulong)(p<72?sbuf[p]:0)) << (8*b); } sblk[w]=x;
    }
    ulong seed[4]; blake2b_block(sblk, 71u, seed);

    // genIndexes + sum
    // seed_byte(k) for k<32; extended wraps: k in 32..35 → seed_byte(k-32)
    #define SB(k) ((uchar)(seed[((k)%32)/8] >> (((( k)%32)%8)*8)))
    ulong sum[4] = {0,0,0,0};
    for (uint j=0;j<32;j++) {
        uint be = ((uint)SB(j)<<24) | ((uint)SB(j+1)<<16) | ((uint)SB(j+2)<<8) | (uint)SB(j+3);
        ulong idx = (ulong)be % n;
        device const ulong* rr = R + idx*4;
        add256(sum, rr[0], rr[1], rr[2], rr[3]);
    }
    #undef SB

    // hit = Blake2b256(be32(sum))
    ulong hb[16];
    hb[0]=swap64(sum[3]); hb[1]=swap64(sum[2]); hb[2]=swap64(sum[1]); hb[3]=swap64(sum[0]);
    for (uint w=4; w<16; w++) hb[w]=0;
    blake2b_block(hb, 32u, d);
}

// Emit the 32 big-endian hit bytes for each nonce (differential test).
kernel void diff_kernel(
    device const ulong*   R    [[buffer(0)]],
    device uchar*         out  [[buffer(1)]],
    constant uchar*       msg  [[buffer(2)]],
    constant Params&      p    [[buffer(3)]],
    uint gid [[thread_position_in_grid]])
{
    if (gid >= p.count) return;
    ulong nonce = p.nonce_base + (ulong)gid;
    ulong d[4]; hit_words(R, msg, p.n, nonce, d);
    device uchar* o = out + gid*32;
    for (uint k=0;k<32;k++) o[k] = (uchar)(d[k/8] >> ((k%8)*8)); // big-endian hit
}

// Mine: if hit < target (both big-endian 32 B) record the winning nonce.
kernel void scan_kernel(
    device const ulong*   R      [[buffer(0)]],
    device atomic_uint*   found  [[buffer(1)]], // [0]=flag, [1]=nonce_lo, [2]=nonce_hi
    constant uchar*       msg    [[buffer(2)]],
    constant Params&      p      [[buffer(3)]],
    constant uchar*       target [[buffer(4)]], // 32 big-endian bytes
    uint gid [[thread_position_in_grid]])
{
    if (gid >= p.count) return;
    ulong nonce = p.nonce_base + (ulong)gid;
    ulong d[4]; hit_words(R, msg, p.n, nonce, d);
    // big-endian lexicographic compare hit < target
    for (uint k=0;k<32;k++) {
        uchar hk = (uchar)(d[k/8] >> ((k%8)*8));
        uchar tk = target[k];
        if (hk < tk) { break; }
        if (hk > tk) { return; } // hit >= target
    }
    if (atomic_fetch_or_explicit(&found[0], 1u, memory_order_relaxed) == 0u) {
        atomic_store_explicit(&found[1], (uint)(nonce & 0xFFFFFFFFUL), memory_order_relaxed);
        atomic_store_explicit(&found[2], (uint)(nonce >> 32), memory_order_relaxed);
    }
}
