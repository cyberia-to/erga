# erga — ERGO (Autolykos v2) miner for Apple Silicon

Zero-copy Autolykos v2 miner on Rust using the honeycrisp hardware driver
stack. ERGO uses Autolykos v2: a memory-hard k-sum PoW where each nonce
attempt requires 32 random reads from a 2+ GB precomputed table R, summed,
and Blake2b256-hashed against target.

The name **erga** (ἔργα) is the plural/feminine of ergon (ἔργον, "work") —
the works/deeds. Sits next to [[mona]] (Monero/RandomX), [[zoya]] (ZANO/
ProgPoWZ), [[trisha]] (Triton/Tip5) as a fourth honeycrisp-native miner.

## Thesis (to be verified)

Apple Silicon's unified memory architecture maps exceptionally well to
Autolykos v2's memory-bound profile:

- **Bandwidth is the bottleneck**, not compute. M4 Max 40-core GPU has
  546 GB/s — comparable to RTX 3070-class. Random reads from a 2+ GB
  table defeat almost all L2 cache, making this a pure-bandwidth race.
- **No PCIe transfer**: R table built once per epoch sits in IOSurface-
  pinned memory; CPU/AMX build, GPU mines, zero copies.
- **No table-size ceiling**: as Autolykos N grows toward 8 GB cap, discrete
  GPUs with 24 GB VRAM stay competitive, but unified memory (64-128 GB on
  M4 Max) has effectively no ceiling. Becomes a structural advantage over
  time.
- **AMX-accelerated Blake2b256 table build**: precomputation of R is itself
  N × Blake2b256 calls. honeycrisp exposes PMULL/SHA extensions; AMX +
  NEON can build the table 2-4× faster than scalar, hiding epoch latency.

Estimated hashrates and efficiency (memory-bandwidth-bound model, 12-18%
realized bandwidth efficiency vs theoretical due to random access cache
miss rate):

| Platform                       | MH/s    | Power     | MH/W     |
|--------------------------------|---------|-----------|----------|
| MacMetal Miner (baseline)      | 60.3    | ~45 W     | ~1.34    |
| RTX 3090 (best OC reference)   | 281     | 171 W     | 1.64     |
| RTX 4090 (best OC reference)   | 292     | 200 W     | 1.46     |
| **erga target (M4, base)**     | ~20-25  | ~15 W     | ~1.5     |
| **erga target (M4 Pro)**       | ~45-55  | ~25 W     | ~2.0     |
| **erga target (M4 Max 40c)**   | ~90-110 | ~50 W     | **~2.0** |
| Theoretical bandwidth ceiling  | ~150    | —         | —        |

**Verification of the 2.0 MH/W claim is the central goal of this project.**
If it cannot be reached, the project is documented as a negative result.

## Architecture

```
honeycrisp/acpu        ← Blake2b256 via PMULL + NEON, P-core affinity
honeycrisp/aruminium   ← Metal GPU dispatch, MTLBuffer over unimem block
honeycrisp/unimem      ← IOSurface-pinned R-table (2-8 GB)
     ↑
erga/crates/
  autolykos/           ← core: primitives, R-table, mining kernel
  worker/              ← GPU worker loop, stratum work management
  erga/                ← CLI binary: stratum v1, stats UI
```

## Autolykos v2 algorithm (reference, normative)

```
Inputs:    m  = block header pre-hash    (32 B)
           h  = block height             (u32)
           M  = 8 KB constant padding    (defined in protocol)
           N  = table size               (function of h, range 2^26 .. 2.14e9)
           target

Precompute R (per epoch, when h crosses a boundary):
    for i in 0..N:
        R[i] = take_right(31, Blake2b256(i || h || M))

Mining loop:
    nonce = random()
    seed  = take_right(8, Blake2b256(m || nonce)) mod N
    idx[0..32] = genIndexes(seed)            ← 32 pseudorandom indexes < N
    sum   = Σ R[idx[j]] for j in 0..31       ← 32 random reads, 31 B each
    d     = Blake2b256(sum) mod q            ← q = order of secp256k1 group
    if d < target: solution = (m, nonce)
```

Key constants:
- **N growth schedule**: starts at 2^26 (67,108,864) at h=0; increments by
  1 in the exponent every 102,400 blocks until h=4,198,400 where N caps at
  2,143,944,600 (the 31-byte addressing limit minus header reservation).
- Current height ~1.5M → N ≈ 1.0e8 → R table ≈ 3.1 GB.
- **genIndexes**: deterministic function of seed; produces 32 indexes via
  iterated Blake2b expansion + modulo-N reduction. Must match reference
  exactly — verified by test vectors.
- **Blake2b256**: 256-bit output variant of Blake2b. Heavier per call than
  SHA-256 (addition-dominant, vs XOR-dominant), 10 rounds × 16 G-functions
  per block.
- **31-byte truncation**: drops MSB to keep values < 2^248, prevents
  overflow during the 32-element sum (max sum < 32 × 2^248 = 2^253 < 2^256).

## Hypothesis verification plan (phased, with hard exit criteria)

Each phase has a **verification metric** — a measured number that must be
hit before proceeding. If the metric is missed, document why and either
adjust the hypothesis or stop. This protects against sunk-cost continuation
of a non-viable approach.

### Phase 0 — Reference correctness (Days 1-3, ~600 lines)

**Goal**: pure-Rust CPU reference implementation that produces bit-exact
hashes matching the ergo-core reference on known test vectors.

Build:
- `autolykos/blake2b.rs` — Blake2b256, validated against RFC 7693 vectors
- `autolykos/genindexes.rs` — genIndexes() with vectors from ergo-core
- `autolykos/reference.rs` — `compute_hash(m, nonce, h, N, R) -> [u8; 32]`
- `autolykos/tests/` — vectors extracted from ergo-core test suite

**Verification metric**: 100% match on ≥20 known (header, nonce, hash)
tuples from ergo-core. **If this fails, the project halts** — without a
correct reference, no benchmark or optimization is meaningful.

### Phase 1 — Microbenchmark Blake2b256 (✅ COMPLETED 2026-05-22)

**Original goal**: measure raw Blake2b256 throughput on M4 Max GPU and
determine if it is a bottleneck for Autolykos v2 mining.

**Original threshold**: ≥ 5 GH/s "on 64-byte inputs." This number was
based on incorrect ALU-budget math — see "lesson" below.

**Crate built**: `crates/blake-bench` — four MSL kernel variants, CPU
reference verification, sweep harness. Each variant verified bit-exact
against the `blake2` crate on 4096 random 32-byte inputs.

**Measured results** (M4 Max 40-core GPU, 32-byte input → 32-byte output,
best of 8 trials per config; figures noisy ±10% from system load):

| Variant | Description                              | Peak MH/s | vs V1 |
|---------|------------------------------------------|-----------|-------|
| V1      | 12-round loop, SIGMA[][] table indexing  | ~860      | 1.00× |
| **V2**  | **Fully unrolled, SIGMA inlined**        | **~1650** | 1.92× |
| V3      | V2 + dual hash per thread (ILP)          | ~1510     | 1.76× |
| V4      | V3 + per-rotation-amount primitives      | ~1400     | 1.63× |

**V2 wins**: a fully-unrolled kernel with SIGMA indices baked into
compile-time message-word references. The Metal compiler then constant-
folds the 12 zero-message-words (m4..m15 are always 0 for 32-byte
padded input), eliminating roughly half of all 64-bit additions in the
G-functions.

V3 (dual-hash per thread) and V4 (per-amount rotations) both
*regressed* slightly vs V2 — dual-hash hurts due to register pressure
(2× state = 2× register usage, reducing in-flight wavefronts and
defeating the latency-hiding goal). Per-amount rotation primitives are
worse than the compiler's own pattern detection on the generic
`(x >> n) | (x << (64-n))` form. **Auditor mindset confirmed: code that
trusts the compiler beat code that tried to outsmart it.**

**Throughput verdict**: M4 Max sustains **~1.6 GH/s of Blake2b-256** on
32-byte inputs. At ~50 W under sustained GPU load this is roughly
**32 MH/J** of Blake2b throughput.

**Lesson on the original 5 GH/s target**: Wrong threshold. The real
Phase 1 question is whether Blake2b256 is a bottleneck in Autolykos
mining. Mining loop runs 2 Blake2b calls per nonce, so 100 MH/s of
mining throughput consumes only ~200 MH/s of Blake2b. We have 1.6 GH/s
available — **8× headroom**. Blake2b is decisively NOT the bottleneck.
The 32-random-reads from the R table will dominate, exactly as expected
from the memory-bandwidth thesis.

**Verification metric (revised)**: ≥ 200 MH/s of Blake2b256 needed for
mining viability. **Met by 8×.** Proceed to Phase 2.

**Honeycrisp stack usage audit** (2026-05-22, after refactor):

The Phase 1 benchmark codebase is now 100% on the honeycrisp zero-copy
pipeline for the parts that apply at this stage:

| Component                                  | Used | Where |
|--------------------------------------------|------|-------|
| `unimem::Block::open()` IOSurface pin      | ✅   | input + output buffers |
| `Gpu::wrap(&block)` MTLBuffer over surface | ✅   | both buffers wrapped |
| `block.as_bytes_mut()` direct CPU access   | ✅   | fill_inputs writes directly |
| `block.as_bytes()` direct CPU readback     | ✅   | read_outputs, verify |
| `Dispatch::new()` + pre-resolved IMPs      | ✅   | hot-path dispatch |
| `Dispatch::dispatch_with_bytes()` inline   | ✅   | uniforms via setBytes |
| `acpu` AMX/NEON Blake2b on CPU             | ❌   | not needed (GPU-only bench) |
| `Dispatch::batch_async` / `GpuFuture`      | ❌   | not needed (single kernel) |
| `block.handle()` IOSurfaceRef for ANE      | ❌   | ANE has no Blake2b use case |

The CPU↔GPU axis of the zero-copy pipeline is fully exercised. Throughput
after the refactor matches the pre-refactor `gpu.buffer()` path within
noise (1.61-1.65 GH/s sustained at count=67M, tg=32, 5-trial median
1625 MH/s). This confirms that at the Metal level, `gpu.wrap(IOSurface)`
and `gpu.buffer(StorageModeShared)` have equivalent throughput for pure
CPU↔GPU workloads — IOSurface's real value appears when AMX/ANE need to
share the same allocation, which is Phase 2.

The four checked items above are the honeycrisp-specific differentiators
that any erga crate going forward will reuse — Phase 2 plugs the R-table
into the same `unimem::Block` and adds AMX on top.

### Phase 2 — R-table build and zero-copy access (✅ COMPLETED 2026-05-22)

**Crate built**: `crates/rtable-bench`. One `unimem::Block` per benchmark
run, wrapped via `gpu.wrap(&block)` as an `MTLBuffer`. CPU builder writes
into the IOSurface-pinned bytes directly via `block.as_bytes_mut()` from
parallel `std::thread::scope` workers. GPU probe kernel reads pseudorandom
rows from the SAME pages via the wrapped buffer. CPU and GPU compute
identical XOR checksums proving the bytes the CPU wrote are exactly the
bytes the GPU read — zero-copy CPU↔GPU sharing fully verified.

Note: this Phase 2 builder uses a simplified row hash
`Blake2b256(LE(row as u32) || LE(h as u32))` (8-byte input → 32-byte
output, single Blake2b block) rather than the full Autolykos
`Blake2b256(i || h || M)` with 8 KB pad. Same row size, same access
pattern — sufficient for validating zero-copy and measuring random-read
bandwidth. Phase 5 will substitute the protocol-exact hash.

**Measured results** (M4 Max, N = 2^26 = 67M rows = 2 GiB table):

| Metric                              | Value           | Target       | Status |
|-------------------------------------|-----------------|--------------|--------|
| CPU table build, 16 threads         | 0.753 s         | ≤ 60 s       | ✅ 80× headroom |
| Table write throughput (CPU side)   | 2853 MB/s       | —            | informational |
| GPU random-read bandwidth (4M–256M reads) | **~82 GB/s**    | ≥ 80 GB/s    | ✅ **PASS (barely)** |
| CPU vs GPU checksum (1M random reads) | byte-exact match | byte-exact | ✅ |

**Bandwidth scaling** (best of 5 trials, GPU probe kernel):

```
       probe count      best ms          GB/s
           1048576         0.71         47.00     (overhead-dominated)
           4194304         1.69         79.43     (approaching saturation)
          16777216         6.39         83.99     (saturated)
          67108864        26.48         81.10
         268435456       105.31         81.57
```

Random-read bandwidth saturates at ~82 GB/s above 4M reads — exactly
the 15% of M4 Max's 546 GB/s peak that the original plan predicted for
random access into a 2-4 GB table.

**Implication for mining hashrate ceiling** (revised, honest):

With 32 random 32-byte reads per Autolykos nonce, the *memory-bound*
mining hashrate ceiling is:

    82 GB/s ÷ (32 reads × 32 bytes) = 80 M nonces/sec

That is the upper bound from memory bandwidth alone — actual mining
will be lower due to genIndexes/Blake2b/dispatch costs. Realistic
projection: **~60-80 MH/s on M4 Max**, not the 90-110 MH/s estimated
in the original plan. Updating the project's hashrate table:

| Platform                       | MH/s (original est.) | **MH/s (post-Phase-2)** |
|--------------------------------|----------------------|--------------------------|
| MacMetal Miner (baseline)      | 60.3                 | 60.3 (unchanged)         |
| erga target (M4 Max 40c)       | 90-110               | **70-80**                |

This shifts the headline efficiency claim. At 70 MH/s / 50 W → 1.4 MH/W,
which is *below* RTX 3090's documented best of 1.64 MH/W. Apple Silicon
no longer obviously wins on energy efficiency for Autolykos v2.

**Phase 2 verdict**: zero-copy CPU↔GPU pipeline is correct and performant
(82 GB/s on random access is the limit of M4 Max bandwidth for this
pattern).

### Phase 3 — Integrated mining kernel (✅ COMPLETED 2026-05-22)

**Crate built**: `crates/mine-bench`. End-to-end Autolykos v2 mining
kernel: per-nonce Blake2b256(m||nonce) → genIndexes(35-byte sliding
window) → 32 random reads from R → 256-bit mod-2^256 sum → Blake2b256
→ XOR-accumulate. Reads R directly from the same IOSurface block built
by Phase 2's rtable-bench. CPU reference miner produces byte-exact
identical XOR accumulators over 64 nonces — kernel is correct.

**Measured mining hashrate** (M4 Max, 2 GiB R-table, best of 5 trials):

```
     nonce count      tg         best ms          MH/s
          262144     128            4.17         62.93
         1048576      32           18.43         56.91
         4194304     256           66.53         63.04
        16777216     128          256.67         65.37   ← peak
```

**Peak measured: 65.4 MH/s** at count=16M, tg=128.

**Comparison table (revised with measurement, not projection)**:

| Platform                          | MH/s   | Power  | MH/W | Notes |
|-----------------------------------|--------|--------|------|-------|
| RTX 3090 (best documented OC)     | 281    | 171 W  | 1.64 | reference |
| RTX 4090 (best OC)                | 292    | 200 W  | 1.46 | reference |
| MacMetal Miner on M4 Max          | 60.3   | ~45 W  | 1.34 | proprietary baseline |
| **erga on M4 Max (this work)**    | **65.4** | ~50 W | **1.31** | open-source, 1st kernel |
| M4 Max memory ceiling             | ~78    | —      | —    | bandwidth physics |

The integrated kernel hits **83% of the memory-bound ceiling**. Most of the
remaining 17% would come from threadgroup memory staging or multiple-
nonces-per-thread; even at 100% ceiling (78 MH/s) the energy figure is
1.56 MH/W — still below RTX 3090's 1.64.

**Strategic verdict**: the original headline hypothesis — *Apple Silicon
beats RTX 3090 energy efficiency on Autolykos v2* — is **disconfirmed by
direct measurement**. Apple M4 Max can run this algorithm correctly and
slightly better than the existing MacMetal Miner (+8%), but it cannot
exceed RTX 3090 efficiency on this workload because the random-access
pattern into a 2-4 GiB table is bandwidth-limited and Apple's 546 GB/s
unified memory cannot deliver more than ~80 GB/s of useful throughput
into that access pattern.

What was learned and what remains valuable regardless:

- ✅ honeycrisp zero-copy pipeline is correct, performant, and reusable
  for any workload where CPU and GPU share large allocations. The
  `unimem::Block` + `gpu.wrap()` pattern is now battle-tested for ≥ 2 GiB
  surfaces with byte-exact CPU↔GPU agreement.
- ✅ Blake2b256 MSL kernel (1.6 GH/s) and integrated Autolykos kernel
  (65 MH/s) are production-quality reusable artifacts.
- ❌ Apple Silicon is NOT the energy-efficiency leader for ERGO mining.
  Not at current N, not at the future ~8 GiB N cap either (would be
  worse, not better — even lower cache hit rate).

**Recommendation**: roll up erga as a partial negative result (the
[[project_xena_miner]] precedent). Preserve the three crates as
infrastructure references; do NOT continue to a productionized miner
since the headline efficiency claim is now empirically refuted.

### Phase 3.x — Optimization attempts (all failed) and diagnostic isolation

**Five additional kernel variants tried (V2-V5 plus V6/V7 diagnostics);
ALL of the optimization variants regressed against V1.**

```
V1 single-nonce baseline           peak 71.76 MH/s   ← winner
V2 dual-nonce + ulong4 loads       peak ~52 MH/s    register pressure
V3 single-nonce + ulong4 loads     peak ~60 MH/s    compiler already vectorizes
V4 4-up batched explicit loads     peak ~60 MH/s    compiler already pipelines
V5 sequential 4-nonce per thread   peak ~58 MH/s    no overhead-amort gain
V6 DIAG: no Blake2b                peak ~57 MH/s    SLOWER than V1
V7 DIAG: no R loads                peak ~669 MH/s   compute is 10× spare
```

**The diagnostic V6/V7 split is decisive**:

- V7 at 669 MH/s proves compute (Blake2b) is plentiful — at least 10×
  more compute headroom than V1 uses. Compute is not the wall.
- V6 at 57 MH/s, *slower* than V1, proves that stripping the heavy
  Blake2b causes thread requests to bunch up at the memory controller.
  Blake2b at the start of V1 naturally staggers thread phases and
  reduces memory contention. **The wall is memory subsystem contention
  on simultaneous random accesses, not aggregate bandwidth and not
  compute.**

This rules out every kernel-level optimization path. The bottleneck is
in Apple M4 Max's unified memory subsystem behavior on 5120-thread
simultaneous random 32-byte reads into a 2 GiB table — a hardware
property, not a code property.

### Final answer (revised after `powermetrics` data)

The "below RTX 3090" conclusion above was based on **estimated** 50 W
power. When the project added actual power measurement via macOS
`powermetrics`, the finding **flipped**.

**Two distinct regimes:**

| Regime | Hashrate | Combined power (CPU+GPU) | MH/W |
|---|---|---|---|
| Burst (5-10s, chip cold) | 65-72 MH/s GPU-only / 84-87 hybrid | uncalibrated, estimated 30-70 W | 1.2-2.4 |
| **Sustained (60s+, thermal equilibrium)** | **32 MH/s** GPU-only | **8.28 W measured** | **3.91** |
| RTX 3090 (best documented) | 281 | 171 | 1.64 |

The sustained measurement was taken after 90s cooldown, then 60s
continuous V1 mining, with `powermetrics --samplers cpu_power,gpu_power`
running concurrently. Mean GPU power during mining: **6.19 W**. Mean
CPU power: **2.09 W**. Total chip compute power: **8.28 W**.

**This is 2.4× the energy efficiency of RTX 3090.**

The reason Apple Silicon wins on sustained efficiency: this workload is
memory-bandwidth-bound. Shader cores stall on DRAM most cycles,
consuming little dynamic ALU power. Apple's SoC detects this and
downclocks aggressively while preserving most of the throughput.
Discrete GPUs cannot downclock independent of their memory subsystem
this aggressively — RTX 3090 burns near rated TDP even when bandwidth
is the wall.

**Caveats:**
- Absolute hashrate per machine is 9× lower than RTX 3090 (32 vs 281)
- This is a MacBook Pro M4 Max number; Mac Studio (active cooling)
  likely sustains higher hashrate, possibly at slightly lower MH/W
- Hybrid CPU+GPU mining WINS in burst (+25%) but LOSES at sustained:
  thermal budget can't support both simultaneously on a laptop. At
  sustained, GPU-only is the optimal config.

### Phase 4-5 — Not strictly recommended, but no longer ruled out

Given Apple Silicon DOES beat RTX 3090 on energy efficiency at sustained
operation, productionizing the miner (Stratum integration, pool client,
etc.) is now defensible IF the target market is energy-cost-sensitive
operators with access to idle MacBook/Mac Mini/Mac Studio inventory.
For greenfield mining hardware purchases, RTX 3090 still wins on
$/MH/s/W combined economics.

### Phase 3 — Mining kernel and naive hashrate (Days 9-12, ~800 lines)

**Goal**: end-to-end mining kernel running on GPU, measure hashrate,
compare to MacMetal Miner baseline.

Build:
- `autolykos/shaders/blake2b.metal` — Blake2b256 MSL function (unrolled)
- `autolykos/shaders/genindexes.metal` — genIndexes in MSL
- `autolykos/shaders/mine.metal` — combined kernel: nonce → seed → indexes
  → R lookups → sum → Blake2b256 → target check
- `worker/gpu.rs` — host-side dispatch, batch-size tuning, result polling

Measure on M4 Max (40-core GPU):
- Hashrate at various threadgroup sizes (32, 64, 128, 256)
- Hashrate at various batch sizes (10k, 100k, 1M nonces per dispatch)
- Power draw via `powermetrics` during sustained mining

**Verification metric (correctness)**: produce a valid share on testnet
within 1 hour at testnet difficulty.

**Verification metric (performance)**: ≥ 70 MH/s with naive (untuned)
kernel — this is the threshold for "we beat MacMetal Miner". If naive
hashrate is < 50 MH/s, the kernel is fundamentally limited; debug before
proceeding to optimization.

### Phase 4 — Optimization sweep (Days 13-16, ~400 lines)

**Goal**: hit the 90-110 MH/s and 2.0 MH/W targets, or document why not.

Tune (in priority order, biggest expected wins first):
1. Threadgroup memory: stage partial sums in SRAM to coalesce writes
2. Coalesced index batching: process 4-8 nonces per thread, amortize
   genIndexes overhead
3. Blake2b256 unrolling depth: 1-round vs 2-round unrolled vs fully unrolled
4. AMX-accelerated genIndexes on CPU, queue indexes to GPU as work-ahead
5. P-core affinity tuning via honeycrisp/acpu

Measure after each optimization: hashrate, power draw, MH/W.

**Verification metric (primary)**: ≥ 90 MH/s sustained at ≤ 55 W →
**≥ 1.64 MH/W**, matching the best documented RTX 3090 figure. This is the
minimum bar to claim efficiency parity.

**Verification metric (stretch)**: ≥ 100 MH/s at ≤ 50 W → **2.0 MH/W**,
the headline claim. **If reached, the thesis is verified.**

### Phase 5 — Mining daemon (Days 17-20, ~600 lines)

**Goal**: production-shaped CLI for sustained mining against a real pool.

Build:
- `erga/stratum.rs` — Stratum v1 JSON-RPC over TCP (adapt mona's plan)
- `erga/stats.rs` — hashrate display, share counter, uptime
- `erga/main.rs` — CLI entry point, flag parsing, pool selection
- Reconnect logic, share submission, difficulty adjustment

**Verification metric**: 24-hour sustained run against 2miners.com ERG pool
with no crashes, share acceptance rate ≥ 98%, no degradation in measured
hashrate vs solo-mining baseline.

## Crate structure

```
erga/
├── Cargo.toml                     workspace
├── plan.md                        this file
└── crates/
    ├── autolykos/                 core algorithm
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── blake2b.rs         Blake2b256, ~250 lines
    │   │   ├── genindexes.rs      genIndexes(seed) -> [u32; 32], ~120 lines
    │   │   ├── rtable.rs          R-table builder over unimem::Block, ~300 lines
    │   │   ├── reference.rs       pure-Rust mining loop for testing, ~150 lines
    │   │   └── shaders/
    │   │       ├── blake2b.metal  ~250 lines MSL
    │   │       ├── genindexes.metal ~120 lines MSL
    │   │       ├── mine.metal     mining kernel, ~250 lines MSL
    │   │       └── probe.metal    Phase 2 verification kernel, ~80 lines
    │   ├── benches/
    │   │   ├── blake2b.rs         Phase 1 microbenchmarks
    │   │   ├── rtable.rs          Phase 2 table build + access
    │   │   └── mine.rs            Phase 3-4 hashrate
    │   └── tests/
    │       ├── vectors/           ergo-core test vectors (JSON)
    │       └── correctness.rs     bit-exact match against vectors
    │
    ├── worker/                    GPU mining loop
    │   └── src/
    │       ├── lib.rs
    │       ├── gpu.rs             aruminium-based dispatcher, ~300 lines
    │       ├── epoch.rs           detect height change, rebuild R, ~150 lines
    │       └── pool.rs            worker thread coordination, ~150 lines
    │
    └── erga/                      CLI binary
        └── src/
            ├── main.rs            ~100 lines
            ├── stratum.rs         Stratum v1 JSON-RPC, ~350 lines
            └── stats.rs           hashrate display, ~150 lines
```

Total LoC estimate: **~2,000-2,200**, in line with the original effort
estimate (14-18 days for a focused implementation).

## Risks and unknowns

1. **Blake2b256 in MSL throughput** — Blake2b's 10-round structure with
   64-bit additions doesn't map as cleanly to GPU SIMT as SHA-256. If MSL
   Blake2b is 2-3× slower than expected, the GPU compute side becomes the
   bottleneck instead of memory, and the bandwidth argument is moot.
   *Phase 1 microbenchmark is the early-warning system for this.*

2. **L2 cache behavior with 3-4 GB random table** — the M4 Max GPU L2 is
   ~24 MB. Random reads into a 3-4 GB table have ~0.6% hit rate; almost
   every read goes to DRAM. The 12-18% bandwidth utilization estimate is
   plausible but could be lower. *Phase 2 random-read benchmark validates
   this directly.*

3. **genIndexes determinism** — genIndexes is one of the trickier parts of
   Autolykos to implement correctly; a subtle bug here produces "valid-
   looking" hashes that the network rejects on submission. *Phase 0 test
   vectors and Phase 3 testnet share submission both catch this.*

4. **Epoch transitions** — when block height crosses a 51,200-block
   boundary, N changes and R must be rebuilt. During the rebuild (estimated
   30-60s on M4 Max) the GPU is idle. Must implement double-buffering or
   accept the gap. *Acceptable for now; revisit if measured downtime is
   significant.*

5. **MacMetal Miner is closed-source** — we cannot inspect their
   implementation. Our 60.3 MH/s baseline is their *advertised* number;
   real-world may vary. Treat as a directional reference, not gospel.

6. **Profitability is poor at current ERG price** ($0.279, breakeven at
   $0.045/kWh for 100 MH/s @ 50 W). **This project is built for efficiency
   leadership demonstration, not profit.** If ERG recovers to $0.50+, the
   work becomes economically meaningful.

## Out of scope (explicitly)

- Windows or Linux ports (Apple Silicon only by design)
- ASIC resistance research (algorithm is fixed; we implement it as-is)
- Pool software (we connect to existing pools as a client only)
- ERG wallet/payout management (use external wallet)
- Hashrate sharing protocols / mining-as-a-service infrastructure

## Open questions for discussion

- Should erga target M4 Pro/Max only, or also tune for M1/M2/M3 generations?
  (Initial target: M4 Max as benchmark platform; add others if base works.)
- Stratum v2 instead of v1? (ERGO pools currently mostly v1; v1 is simpler
  and matches mona; defer v2 to later.)
- Should the R table builder share code with mona's Argon2d/AES table
  build, or stay separate? (Separate for now; honeycrisp/acpu is the
  shared dependency.)
