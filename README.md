# erga

Autolykos v2 (ERGO) mining research on Apple Silicon — zero-copy, and
honest about what it found.

**erga** (ἔργα, "the works") asks one question: does Apple's unified
memory make M-series chips competitive at ERGO's memory-hard proof of
work? The answer surprised us twice.

Status: **research artifact, experimental.** The kernels are real and
measured; a pool-ready miner is not built yet (see
[what this is not](#what-this-is-not-yet)).

## the app — one button

`erga.app` is a menu-free desktop miner: open it, press the crystal,
watch the hashrate climb. It runs the same honeycrisp zero-copy kernel
the study measured, shows live MH/s, the device, the pinned table size,
and — read-only, via the public explorer — the confirmed balance of any
address you paste.

Download the `.dmg` from [Releases](https://github.com/cyberia-to/erga/releases).
It is not yet notarized, so the first launch needs one line to clear the
quarantine flag Gatekeeper sets on downloaded apps:

```
xattr -dr com.apple.quarantine /Applications/erga.app
```

then open it normally. v0.1 is a **local mining benchmark** — it proves
the efficiency on *your* Mac. Pool connection and share submission are
the next build (see [what this is not](#what-this-is-not-yet)).

Build it yourself:

```
git clone https://github.com/cyberia-to/honeycrisp ../honeycrisp
nu packaging/bundle.nu        # → packaging/dist/erga.app + erga-<ver>.dmg
# or just run it:
cargo run --release -p erga-app
cargo run --release -p erga-app -- --smoke   # headless: prints live MH/s
```

## the result

Measured on a MacBook Pro M4 Max (40-core GPU), `powermetrics` sampling
CPU+GPU power concurrently with mining:

| regime | hashrate | chip power (measured) | MH/W |
|---|---|---|---|
| burst (5–10 s, cold chip) | 65–72 MH/s GPU-only, 84–87 hybrid | est. 30–70 W | 1.2–2.4 |
| **sustained (60 s+, thermal equilibrium)** | **32 MH/s** GPU-only | **8.28 W** | **3.91** |
| RTX 3090, best documented OC | 281 MH/s | 171 W | 1.64 |
| RTX 4090, best documented OC | 292 MH/s | 200 W | 1.46 |
| MacMetal Miner on M4 Max (proprietary baseline) | 60.3 MH/s | ~45 W | 1.34 |

**Sustained, an M4 Max delivers 2.4× the energy efficiency of an
RTX 3090** on this workload — 6.19 W mean GPU + 2.09 W mean CPU while
holding 32 MH/s. The absolute rate per machine is ~9× lower than a
3090; the win is per watt, not per box.

The measurement protocol: 90 s cooldown, then 60 s of continuous V1
mining with `powermetrics --samplers cpu_power,gpu_power` running
alongside. Laptop chassis — a Mac Studio should sustain more (and
possibly score slightly lower MH/W doing it).

## the story, honestly

1. **Hypothesis**: unified memory + no PCIe + no VRAM ceiling should
   make Apple Silicon the efficiency leader for a bandwidth-bound PoW.
2. **First verdict — disconfirmed.** The integrated kernel peaked at
   65–72 MH/s, 83% of the memory-bound ceiling (~78 MH/s at the ~82 GB/s
   the chip actually delivers into 32-byte random reads from a 2 GiB
   table). At an *estimated* 50 W that computed to 1.31 MH/W — below
   the 3090. We wrote it up as a negative result.
3. **Then we measured power instead of estimating it.** At thermal
   equilibrium the SoC downclocks aggressively while the workload stays
   memory-stalled, holding half the hashrate at one-sixth the estimated
   power. 32 MH/s at 8.28 W measured — the verdict flipped to 3.91 MH/W.

Why Apple wins sustained: the workload stalls shader cores on DRAM most
cycles, so dynamic ALU power is mostly waste. Apple's power management
collapses clocks under that profile while preserving most throughput;
a discrete GPU cannot downclock that far independent of its memory
subsystem, so a 3090 burns near TDP even when bandwidth is the wall.

## the wall (the finding that generalizes)

Eight kernel variants and two diagnostics isolated the bottleneck:

```
V1  single-nonce baseline            71.8 MH/s   ← winner
V2  dual-nonce + ulong4 loads        ~52         register pressure
V3  single-nonce + ulong4 loads      ~60         compiler already vectorizes
V4  4-up batched explicit loads      ~60         compiler already pipelines
V5  sequential 4-nonce per thread    ~58         no amortization gain
V6  DIAG: Blake2b removed            ~57         SLOWER than V1
V7  DIAG: R-table reads removed      ~669        compute is 10× spare
```

V7 proves compute is plentiful. V6 — *slower without the hash* — proves
the wall is **memory-subsystem contention on thousands of simultaneous
random 32-byte reads**, not aggregate bandwidth and not compute: the
Blake2b work at the top of V1 naturally staggers thread phases and
reduces contention at the memory controller. That rules out every
kernel-level optimization path; the limit is a hardware property of the
unified memory subsystem under this access pattern.

Corollary the compiler taught us twice: kernels that trusted the Metal
compiler beat every kernel that tried to outsmart it.

## what is in the repo

Three crates, each a standalone benchmark with its own binary:

| crate | what it measures | headline number (M4 Max) |
|---|---|---|
| `crates/blake-bench` | Blake2b-256 in MSL, 4 variants, vs a CPU reference implementation | **1.6 GH/s** sustained (fully-unrolled V2 kernel) — 8× more than mining ever needs |
| `crates/rtable-bench` | 2 GiB R-table: parallel CPU build into IOSurface-pinned memory, GPU random-read probe, byte-exact CPU↔GPU checksum | build 0.75 s (16 threads); **~82 GB/s** useful random-read bandwidth; checksums match |
| `crates/mine-bench` | the integrated per-nonce loop: seed hash → 32 indexes → 32 random reads → 256-bit sum → final Blake2b; 8 kernel variants + diagnostics | 71.8 MH/s peak, **32 MH/s @ 8.28 W sustained** |

Everything runs zero-copy on the [honeycrisp](https://github.com/cyberia-to/honeycrisp)
driver stack: one `unimem::Block` (IOSurface-pinned) holds the table,
CPU threads write it directly, the GPU reads the same pages through a
wrapped `MTLBuffer`. No staging, no copies, byte-exact agreement
verified.

## the road to earning

The efficiency study answered its question; the work since is turning the
benchmark into a miner that earns. Status, honestly:

| piece | state | verified how |
|---|---|---|
| protocol-exact Autolykos v2 (`crates/autolykos`) | **done** | reproduces sigma-rust's own chain test vector (height 614400 → hit `0x0002fcb1…412a`) |
| table-read mining path | **done** | differential-tested equal to the recompute reference |
| share search (find a nonce below a target) | **done** | found nonce re-verified via the recompute path |
| stratum client (`crates/erga-pool`) | **connects & parses live** | parsed a real herominers job: height→N, msg, target |
| GPU-exact kernel at share difficulty | **next** | — |
| a share accepted by a pool | **next** | — |

The correctness engine a share stands on is now chain-verified. What's
left to *earn* is throughput: at pool difficulty a share is ~1 in 4·10⁹
nonces — a GPU at 60 MH/s finds one in ~70 s, a CPU in ~20 min. So the
final step is the GPU kernel computing the exact hit (33 table reads +
the exact seed + a target compare, returning winning nonces) wired to the
stratum `submit` (already framed in `crates/erga-pool`, per
[`STRATUM.md`](crates/autolykos/STRATUM.md)). Until a pool accepts a
share, this does not yet earn — and the app says so.

```
cargo run --release -p erga-pool -- ergo.herominers.com 1180   # parse a live job
cargo test -p autolykos                                        # the chain-verified engine
```

### what the v0.1 app is
The released `erga.app` is still the **local benchmark** — real hashrate,
real efficiency, no pool yet. Pool mining ships when the GPU-exact kernel
above lands.

## reproduce

Requirements: Apple Silicon Mac, macOS 14+, Rust stable, and a sibling
checkout of honeycrisp (path dependencies):

```
git clone https://github.com/cyberia-to/honeycrisp ../honeycrisp
cargo build --release
```

Run the benches:

```
cargo run --release -p blake-bench     # Blake2b256 MSL variants
cargo run --release -p rtable-bench    # table build + random-read bandwidth
cargo run --release -p mine-bench      # integrated mining kernels V1..V9
```

For the sustained efficiency number, replicate the power protocol: let
the chip cool ~90 s, start `sudo powermetrics --samplers
cpu_power,gpu_power -i 1000` in a second terminal, run mine-bench V1
for 60 s+, average the power samples over the mining window.

## lineage

erga is one of the honeycrisp miner studies, next to mona (RandomX),
zoya (ProgPoWZ), trisha (Tip5) and xena (XEL) — each asks whether a
specific PoW's physics favors Apple Silicon, and each publishes the
answer either way. The full lab notebook for this one, including the
failed variants and the revised verdicts, is [plan.md](plan.md).
