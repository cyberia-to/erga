# RTable owner research

Stage draft; research/PLAN only. Serves Kadek foundation row 6 native ownership;
discovered-from Honeycrisp PR14 and Erga PR1's remaining RTable diagnostics.
This unit covers the table, its concrete GPU probe and rtable CLI. Mine ownership,
native Image, performance and physical/pinning policy remain open.

## Exact sources and observations

Owned worktree `/Users/master/cyber/erga-unimem-rtable-owner`, branch
`fix/unimem-rtable-owner`, fresh origin/main
`14b40d667b7fa927f62f46423cd8096f01cf704f` (Erga PR1 merge).
Committed Honeycrisp `9cbd48070a136e24711896c930670fcff8a147fc`, Strata
`494df7a0973ae10a6cc1c2022e644d70449a29d6`; remote HEADs rechecked 2026-10-10.
Current dev/engineering/quality/projects/documentation and Erga CLAUDE/spec read.
Original working trees remain untouched and are not build inputs.

`/tmp/erga-rtable-owner-plan-v1/` retains exact archives, 846 unchanged committed
paths, origins, source hashes, commands and raw streams. Preparation and verifier:
`/tmp/erga-rtable-{prepare.nu,gate.nu,inventory.rb}`. Rustup 1.95.0 with exact
PATH/RUSTC/RUSTDOC, jobs 2, private target. Two fresh compile-only observations:

| command (both `--locked --offline`) | exit | observation |
|---|---:|---|
| `cargo metadata --format-version=1` | 0 | 12 local packages resolve inside exact archives |
| `cargo check -p rtable-bench --all-targets` | 101 | three E0133 plus one E0596 at lib 58/80/107; repeated by lib-test compilation |

Original lock SHA
`606817eddfdf749ea453cadcaa37838d1c9e4da820b13edf5be715377d214973`
is unchanged; the previous unit already normalized it. No legacy native owner
executed. Completed Blake targets alone were removed after PR1 merge: `du -sk`
recorded 2486360, 30788 and 12144 KiB; paths, command and absence checks are in
`completed-target-cleanup.json`. Logs, archives and evidence remain.

## Caller and escape inventory

`caller-inventory.json` scans all 39 committed Rust files with explicit zero rows,
full-file hashes and line matches. Only four files use the RTable public surface:

- `rs/rtable-bench/src/lib.rs`: public Buffer/Block/n at 46–50; import 58 before
  initialization; shared `build_parallel` 76 writes the entire actual allocation
  at 80, including padded rows. Zero workers divides by zero. n narrows to usize,
  row indices narrow to u32, and size overflow panics. CPU checksum 107 reads lazy
  bytes before a build is required. Public IOSurface ID supplies another alias.
- Same lib: `Probe` 137 exposes a replaceable Gpu and accepts a caller accumulator
  at 161. `Dispatch::dispatch_with_bytes` waits but discards completion status.
  Public `read_u64`/`write_u64` accept arbitrary Buffer size/storage/aliases.
  Public `lcg_index` panics for n=0. Only the concrete probe uses these helpers.
- `rs/rtable-bench/src/main.rs`: table construction/build at 43/52 and 104/111;
  raw accumulator creation/reset/read at 74–81 and 124–127, then benchmark loops.
  It moves Gpu into Probe after constructing a currently unbounded-lifetime table.
  `1u64 << log2n` and byte arithmetic accept unchecked CLI values.
- `rs/mine-bench/src/lib.rs`: GPU binds public imports at 272/274/285; CPU reads
  Block at 306/443; count fields feed shader params and references. `GpuMiner`
  and V9 expose devices, output buffers escape, and waits ignore native status.
- `rs/mine-bench/src/main.rs`: table factory 43–55; reads at 109/269/444. The
  concurrent path 275 onward lends immutable bytes to scoped CPU workers while
  GPU kernels read the table. V9 copies the table into a separate texture through
  `texture_ffi.rs::upload_rtable`; that safe raw-pointer API has its own proof gap.

Complete mine lib/main/texture FFI and shader bodies belong in PLAN review even
though unchanged. Mine is the sole package depending on rtable-bench; the actual
erga-miner/CLI depend on Blake reference and use separate ordinary Metal buffers.
No direct mine migration or runtime is authorized here. Private fields/lifetimes
and changed signatures will introduce new mine diagnostics, distinguishable from
the current Honey-induced E0133/E0596. Earlier chime overflow failures stay masked.

Fresh `external-inventory.json` checks the same 22-origin bounded closure as the
previous audit; only Erga matches exact identifiers/dependencies. Source inventory
is not a whole-organization/rung build. Soft3 origin
`18fd6b22ec45725aa776e37d377c5a75aeeaf553`, components SHA
`02aae8e6f2def3e23c124c76be6cc52b01cc54e86ab514c0a519dcbaf7c21e8e`,
still has no Erga/rtable row. `resolved-local-packages.json` records the measured
Cargo graph; no registry row or kelvin result is invented.

## Native and worker proof

One logical row is 32 bytes, generated from `LE(row:u32) || LE(height:u32)`; this
benchmark is the existing simplified generator, not full Autolykos table semantics.
Valid n is 1 through 2^32 inclusive: the final row then has index u32::MAX. Keep
the public u64 count but reject larger values before native calls; check usize,
multiplication, page rounding and isize bounds. Actual backing may exceed n×32.
Initialize the full actual Block extent before any Rust view/import; build and
read views touch only the logical prefix. Padded rows must never be generated.

Reuse the reviewed private native preflight from committed Blake
`rs/blake-bench/src/import_memory.rs` as a local module, without new dependencies,
cross-crate textual includes, public framework or provider changes. Runtime page
size, ulong alignment, whole actual single read/write VM leaf, exact planned
extent and device max are separate checks. Buffer drops before Block; borrowed
Gpu survives both. Only immutable metadata and logical bytes escape; no Buffer,
Block, ID, native handle, mutable bytes, Clone, unsafe Send/Sync or callback export.
Buffer's native pointer fields mean table and Probe inherit !Send/!Sync. Workers
receive only disjoint mutable chunks, not a reference to either native owner.

Pinned native primaries are the already reviewed SDK 26.4.1/XNU
`f6217f891ac0bb64f3d375211650a4c1ff8ca1ea` and Apple Metal sources retained in
Erga PR1's audit. They establish current VM/import geometry, CPU writes before
commit and readback after checked completion; physical policy is unchanged.
The probe shader's `device const ulong*` reads row `idx*4`; n≤2^32 implies no
u64 multiplication wrap. Exact `count` threads keep every gid in uint range;
count=0 needs no submit. Fixed width 64 still requires real device/pipeline checks.
Two atomic_uint accumulator lanes occupy exactly eight initialized bytes.
No shader changes are needed.

CPU worker scope must complete even when spawn fails, a worker panics or a handle
is dropped/forgotten. Official Rust 1.95.0 source at compiler commit
`59807616e1fa2540724bfbac14d7976d7e4a3860`, `library/std/src/thread/scoped.rs`,
was fetched to `primary/rust-scoped.rs` (SHA
`5030a18b1ae9745b48e98b06c0877f31d010ad8838796a5f79685a01f1cf92bb`).
`scope` waits for its running-thread count before returning or resuming panic;
`Builder::spawn_scoped` returns an io::Result. A catch around the entire scope
converts worker panic after joining. Scope-owned joining, not handle Drop, is the
memory-safety boundary. A private worker closure allows a meaningful injected
panic test through the same control path; no callback reaches the public API.

Probe uses private initialized accumulator, retained queue/command/encoder, and
exclusive mutable borrows of Probe/table. All fallible preparation precedes the
failed flags. Submit and wait are adjacent; only completed status plus absent
native error restores access. Both objects remain unavailable after failure.
No async result/guard can be forgotten between submission and waiting. Forgetting
an ordinary idle owner only leaks its retained objects. Process abort and foreign
exceptions are outside recoverable Rust error guarantees.

## Independent bounded oracle

`oracle/main.go` uses cached, pinned Go x/crypto v0.48.0, revision
`e08b06753d6a72f1fe375b6e0fefefb39917c165`; published source fetched independently
and hashed in `primary/acquisition.json`. Go 1.26.3, offline cache, local toolchain,
private GOCACHE and jobs 2 generated 16 full row digests and 56 checksum cases,
maximum 513 rows / 257 reads. No Erga code or native owner is used.
`oracle/{execution,verification,vectors}.json`, go.mod/go.sum and source make the
oracle reproducible without adding Go to production or Rust test dependencies.

Published BoringSSL record input `0001020304050607` at commit
`09f7078f953362d5c5afdd224118845327b60fb4`, lines 66–67 of
`src/crypto/blake2/blake2b256_tests.txt`, independently matches generated row
0x03020100 / height 0x07060504:
`77065d25b622a8251094d869edf6b4e9ba0708a8db1f239cb68e4eeb45851621`.
Native tests will also use literal bounded rows 0/1/2 and checksums; no huge table
is needed to test the published high-index row function.
An initial Nu receipt expression misread unquoted go.sum as a command after
writing output. Those outputs are retained as initial artifacts; the corrected
persisted runner's complete `-mod=readonly` exit-0 receipt is authoritative.
Web reader could not open versioned Go/Rust docs; pinned raw-source curl receipts
succeeded. No conformance or new native-test result is claimed before implementation.

## Mine follow-up remains open

A concrete mine owner can consume a built RTable by value and keep it private and
immutable, with its own imports/accumulators and synchronous completion. CPU
workers can borrow immutable slices while known read-only GPU kernels execute.
Existing unsafe `Gpu::buffer_wrap` could import the same allocation using a
narrowly added exact-backing extent contract plus the logical slice's pointer;
never round beyond a slice by guess or expose a safe arbitrary native callback.
That getter/native retention design, concurrent-read proof and texture ownership
require the next PLAN. This unit adds no speculative export and no Drop-only
fence. Private ownership allows this additive follow-up without reopening fields.
