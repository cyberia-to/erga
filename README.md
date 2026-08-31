# erga

**One button. Your Mac mines ERGO.**

![erga](docs/erga.png)

Press the crystal and an Apple Silicon Mac mines Autolykos v2 to a pool, under
a wallet the app generates for you. Live hashrate, accepted shares, and an
honest countdown to your first payout computed from live network difficulty.

It works end to end: it connects, mines, submits, gets shares **accepted**, and
tracks what the pool owes you — verified against the chain, not just against
itself.

---

## install

Download the `.dmg` from [Releases](https://github.com/cyberia-to/erga/releases)
and drag `erga.app` to Applications. It is not notarized yet, so clear the
quarantine flag Gatekeeper puts on downloaded apps:

```
xattr -dr com.apple.quarantine /Applications/erga.app
```

Open it and press the crystal. A wallet is generated on first launch — back up
the 15 words behind the **back up** button. That seed *is* your wallet.

**Requires** Apple Silicon, macOS 14+, and **16 GB of unified memory**
([why](#memory-is-the-real-gate)).

### from the terminal

Installing the app puts `erga` on your PATH by itself, linking
`/usr/local/bin/erga` (or `~/.local/bin/erga`) to the binary inside the bundle.
`erga link` does it on demand.

```
erga                    open the window
erga mine               mine here, no window — your wallet, the pool you picked
erga mine <host> <port> <address>
erga status             what the pool owes you, without opening anything
erga link               put `erga` on your PATH
erga difftest           prove the GPU kernel against the CPU reference
erga buildbench         time one epoch-table build
erga help
```

One binary does all of it — the window spawns *itself* to mine, so nothing in
the bundle can go missing or drift out of version with anything else.

---

## what to expect

### memory is the real gate

Autolykos v2 rebuilds a table of `N` 32-byte elements **every block**, and `N`
grows with chain height. At height ~1.86M that table is **6.8 GiB**, and the
process sits at ~7.1 GB steady / 8.5 GB peak — measured with `footprint`.

| unified memory | verdict |
|---|---|
| 8 GB | **will not work** — the table alone exceeds the machine |
| 16 GB | works today, little room to spare |
| 24–36 GB | comfortable |
| 48 GB+ | room for years of growth |

`N` rises 5% every 51,200 blocks — about every 71 days, so roughly **+28% a
year**. A 16 GB Mac that mines today will be squeezed within a year or two.
That is the protocol, not erga.

### hashrate

Two numbers matter, and the app shows both. The **live** rate is what the GPU
does while searching; the **effective** rate also counts the seconds spent
rebuilding the table at every block. The pool only ever agrees with the second
one.

| chip | memory bandwidth | live MH/s | effective |
|---|---|---|---|
| **M4 Max** (40-core GPU) | 546 GB/s | **65–74 measured** | **~51 measured** |
| M1 / M2 Ultra | 800 GB/s | ~75–95 est. | two dies — may scale sub-linearly |
| M1 / M2 / M3 Max | 400 GB/s | ~40–50 est. | |
| M4 Pro | 273 GB/s | ~26–34 est. | |
| M1 / M2 Pro | 200 GB/s | ~19–25 est. | |
| M3 Pro | 150 GB/s | ~14–18 est. | |
| M4 | 120 GB/s | ~11–15 est. | |
| M1 / M2 / M3 base | 68–100 GB/s | ~7–12 est. | |

Autolykos v2 is memory-bandwidth-bound, so the rate tracks bandwidth far more
closely than GPU core count. Only the first row is measured on hardware we own;
the rest is that ratio applied, and should be read as an estimate. Measured a
number on your Mac? Open an issue — the table gets better.

### what it earns

Honestly: not much, and the app says so. At ~60 MH/s against the current
network (~600 GH/s, 3 ERG per block) that is roughly **6–8 ERG a month** — a
couple of dollars. The app shows it live, in ERG and in dollars, so you never
have to guess.

The interesting number is not the dollars. It is the
[watts](#the-research-behind-it).

---

## how it works

```
erga.app  (eframe/egui — draws only)
   │  spawns, reads STAT lines from stdout
   ▼
erga mine  (its own process — all GPU work lives here)
   │
   ├── autolykos   protocol-exact reference, chain-verified
   ├── erga-pool   stratum: subscribe · authorize · notify · submit
   └── honeycrisp  zero-copy Metal: one IOSurface-pinned table, no staging
```

The miner runs as a **separate process** on purpose. The GUI holds an OpenGL
context (eframe/glow) while the miner drives Metal through honeycrisp; the two
graphics APIs in one process proved fragile enough to abort the app. Split, the
window only draws — and if the miner ever dies, the UI survives and says so.

### correctness

Five invariants, spelled out in [`specs/`](specs/README.md). The one that
matters most: **never submit an unverified share.** Every GPU candidate is
re-hashed on the CPU reference before it goes out, so nothing invalid ever
leaves the machine.

| piece | verified how |
|---|---|
| Autolykos v2 reference | reproduces sigma-rust's own chain vector (height 614400 → hit `0x0002fcb1…412a`) |
| GPU kernel | differential test: GPU hit is byte-exact to the CPU reference over 512 nonces |
| stratum client | live shares **accepted** by herominers and by 2miners |
| wallet | ergo-lib, the reference wallet: BIP39 → `m/44'/429'/0'/0/0` → P2PK |
| balance + ledger | public explorer plus the pool's per-address API |
| tests | 14 passing (`cargo test --workspace`) |

### the things that matter more than they look

**It keeps mining.** The Mac is held awake while mining (`caffeinate` tied to
the miner's pid — a machine asleep at night halves your month), a dropped pool
auto-reconnects instead of ending the session, and all-time shares and hashes
survive quitting, checkpointed every minute so a crash costs at most that.

**It tells the truth.** The effective rate sits beside the live one. The cpu
and memory meters draw the miner's own share solid over a dim total, so you can
see whether erga is why the machine is busy. Colours carry meaning: mint is
what you gain, amber what it costs, coral what went wrong, blue what the chain
says.

**It respects the pool list.** herominers (0.5 ERG floor, the lowest verified)
and 2miners, each with its ledger inside the app. A pool is listed only after
this client has held a real conversation with it
([why so few](#why-only-two-pools)). Regions are absent on purpose — every pool
routes you to its nearest server, and latency costs at most a stale share.

**Solo, if you want the lottery.** A switch beside the pool, wherever the pool
offers it. Note what it is: the pool still builds the block and still takes its
fee; only the accounting changes, so whoever solves a block keeps it instead of
sharing every one. The payout bar is replaced by the number that means
something there — how long a block takes at your rate. (Solo against *your own
node* is a different thing, and erga does not do it yet.)

**It is easy to report.** `report a bug` opens a GitHub issue pre-filled with
your erga version, macOS, chip, memory, pool, live state and the last 40 log
lines, and reveals the full log in Finder so attaching it is one drag.

**Small things.** The seed lives in a `0600` file, not the Keychain — the
Keychain prompts on every rebuild, because ad-hoc signatures change.
`ERGA_AUTOSTART=1` starts mining the moment the window opens, for a login item
or a machine whose only job is this.

---

## the 5% development share

One share in every 20 is mined for the project. It is implemented as a
*separate authorized session*: a pool credits whoever authorized the
connection, so shares cannot be relabeled — erga alternates sessions, 19 for
you and 1 for development. The count is shown in the app under **to
development**, beside your own all-time shares. It is never hidden.

You own the software and the choice. No rebuild needed:

```
ERGA_DONATION=off                turn it off
ERGA_DONATION=<your address>     send that 5% wherever you like
ERGA_DONATION_EVERY=50           1 share in 50 (2%) instead
```

Or edit `DONATION_ADDRESS` / `DONATION_EVERY_NTH` at the top of
[`rs/miner/src/engine.rs`](rs/miner/src/engine.rs).

---

## why only two pools

Ergo's official docs list eight. Every one was probed with this client on
2026-08-31; two answered.

| pool | what happened |
|---|---|
| **herominers** | works — shares accepted, 0.5 ERG floor, full per-address API |
| **2miners** | works — share accepted, 1 ERG floor, per-address API |
| k1pool | sends jobs this client parses, but after minutes of mining its API still reported `workers: 0` and no shares — nothing we sent was credited |
| kryptex | TCP opens on 7777, then silence: no reply to subscribe in 35 s |
| woolypooly | its own advertised host, `pool.woolypooly.com:3100`, does not resolve |
| sigmanauts | stratum hostnames do not resolve; the raw IP refuses both ports. The site is still up on GitHub Pages; the pool behind it is not |
| nanopool | hostnames resolve, every documented port closed |
| f2pool | no Ergo host exists at all |

The pattern is the same everywhere: Ergo's network fell to ~600 GH/s, most
pools quietly retired the infrastructure, and the marketing pages — and the
official list — stayed up. A menu entry that silently mines nothing is worse
than a short menu.

Endpoints in documentation are not evidence. Each pool above was checked by
DNS, by TCP, by a real stratum handshake, and where possible by an accepted
share.

---

## build from source

Apple Silicon, macOS 14+, Rust stable, and a sibling checkout of honeycrisp:

```
git clone https://github.com/cyberia-to/honeycrisp ../honeycrisp
cargo build --release           # needs RUSTC_BOOTSTRAP=1
cargo test --workspace          # 14 tests
nu packaging/bundle.nu          # → packaging/dist/erga.app + the .dmg
```

The app icon is code too — `swift packaging/icon.swift` redraws it.

| path | what |
|---|---|
| [`cli/`](cli/) | the command — opens the window, mines, reports, links itself |
| [`rs/`](rs/) | the libraries — `autolykos`, `pool`, `miner`, `wallet`, `app`, and three benches |
| [`docs/`](docs/README.md) | how it works: architecture, the mining loop, where the money is |
| [`specs/`](specs/README.md) | what it must do: correctness invariants and the resource contract |
| [`packaging/`](packaging/) | the `.app`, the `.dmg`, and the icon as code |

About 6,400 lines of Rust. What mines is a single 247-line Metal kernel; the
other fourteen shaders in the tree are the bench variants below, kept because
the negative results are the point.

---

## the research behind it

erga began as a measurement, not a product: **does Apple's unified memory make
M-series chips competitive at a memory-hard proof of work?** The answer flipped
twice.

The integrated kernel peaked at 65–74 MH/s — 83% of the memory-bound ceiling
(~78 MH/s at the ~82 GB/s the chip actually delivers into 32-byte random
reads). At an *estimated* 50 W that was 1.31 MH/W, below an RTX 3090, and it
was written up as a negative result.

Then power was measured instead of estimated:

| regime | hashrate | chip power | MH/W |
|---|---|---|---|
| **M4 Max sustained** (60 s+, thermal equilibrium) | **32 MH/s** | **8.28 W** measured | **3.91** |
| RTX 3090, best documented OC | 281 MH/s | 171 W | 1.64 |
| RTX 4090, best documented OC | 292 MH/s | 200 W | 1.46 |
| MacMetal Miner on M4 Max (proprietary baseline) | 60.3 MH/s | ~45 W | 1.34 |

**Sustained, an M4 Max delivers 2.4× the energy efficiency of an RTX 3090** on
this workload — 6.19 W mean GPU + 2.09 W mean CPU while holding 32 MH/s. Per
machine it is ~9× slower than a 3090; the win is per watt, not per box.

That row is the power study's own equilibrium regime, where clocks settle low.
The shipping miner runs the same kernel harder — 65–74 MH/s — and
correspondingly hotter; its power has not been measured the same way, so the
two numbers answer different questions.

Why Apple wins sustained: the workload stalls shader cores on DRAM most cycles,
so dynamic ALU power is mostly waste. Apple's power management collapses clocks
under that profile while keeping most of the throughput; a discrete GPU cannot
downclock that far independently of its memory subsystem, so a 3090 burns near
TDP even when bandwidth is the wall.

### the wall

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

V7 proves compute is plentiful. V6 — *slower without the hash* — proves the wall
is **memory-subsystem contention on thousands of simultaneous random 32-byte
reads**, not aggregate bandwidth and not compute: the Blake2b work at the top of
V1 staggers thread phases and eases contention at the memory controller. Every
kernel that tried to outsmart the Metal compiler lost to the one that trusted
it.

The benches are still in the repo and still run:

```
cargo run --release -p blake-bench    # Blake2b256 in MSL — 1.73 GH/s peak
cargo run --release -p rtable-bench   # table build + random-read bandwidth
cargo run --release -p mine-bench     # the integrated kernels, V1..V9
```

---

## what it is not

- **not a wallet you spend from.** Balance and seed backup, yes; sending, no.
  Import the seed into any Ergo wallet to move coins — keeping the miner and the
  spending keys apart is the safer default.
- **not notarized.** Hence the `xattr` line above.
- **not solo against your own node.** Pool solo, yes; a local node, not yet.
- **not for Intel Macs.** Apple Silicon only.

## lineage

erga is one of the honeycrisp miner studies, beside mona (RandomX), zoya
(ProgPoWZ), trisha (Tip5) and xena (XEL) — each asks whether a specific PoW's
physics favors Apple Silicon, and each publishes the answer either way. The full
lab notebook, including the failed variants and the revised verdicts, is
[plan.md](plan.md).

Built on [honeycrisp](https://github.com/cyberia-to/honeycrisp) — zero-copy
Metal, NEON/AMX/SME, ANE — part of
[soft3](https://github.com/cyberia-to/soft3).

MIT — see [LICENSE](LICENSE).
