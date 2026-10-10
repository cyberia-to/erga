# RTable owner plan

Stage draft; implementation waits for fresh different-vendor PLAN approval.
Serves Kadek foundation row 6; discovered-from Honeycrisp PR14 and Erga PR1.
Base `14b40d667b7fa927f62f46423cd8096f01cf704f`, Honeycrisp `9cbd4807`,
Strata `494df7a`; full revisions/evidence are in research. No manifest, lock,
version, provider, shader, mine or chime changes. No release/performance/PA claim.

## Public contract

Signature notation (private fields, no raw accessor or public callback):

```text
pub struct RTable<'gpu>;
impl<'gpu> RTable<'gpu> {
    pub fn open(gpu: &'gpu Gpu, rows: u64) -> Result<Self, BenchError>;
    pub fn row_count(&self) -> u64;
    pub fn bytes(&self) -> Result<&[u8], BenchError>;
    pub fn build_parallel(&mut self, height: u32, threads: usize) -> Result<(), BenchError>;
    pub fn cpu_checksum(&self, seed: u64, count: u32) -> Result<u64, BenchError>;
}
pub struct Probe<'gpu>;
impl<'gpu> Probe<'gpu> {
    pub fn open(gpu: &'gpu Gpu) -> Result<Self, GpuError>;
    pub fn run(&mut self, table: &mut RTable<'_>, seed: u64, count: u32)
        -> Result<u64, BenchError>;
}
```

Remove public Buffer/Block/n fields and IOSurface ID, public Probe device,
arbitrary caller accumulator, read_u64/write_u64 helpers and public lcg_index.
The latter becomes a private helper over the proven nonzero row count. Keep
ROW_BYTES. No aliases/compatibility shims. Source breaks in mine are intentional,
explicitly inventoried, and reserved for the next ownership unit.

Retain BenchError::Gpu/Mem, add structured RowCountOutOfRange { rows: u64 },
InvalidThreadCount, WorkerSpawn(std::io::Error), WorkerPanicked and Unavailable;
implement concise Display/Error/source. Zero rows maps to Mem(ZeroSize); rows
above 2^32 maps to RowCountOutOfRange; integer/slice/page-rounding overflow maps
to Mem(SizeOverflow). Runtime mapping/import failures retain GpuError details.
Unavailable applies after partial worker failure or unsuccessful GPU completion;
row_count remains inspectable. No reset/rebuild recovery of an unavailable owner.

## Construction and CPU publication

1. Validate row domain; checked usize conversion and rows×32; positive runtime
   page size and checked round-up; Block::plan; check planned extent against
   device max before opening → verify: pure boundary/error tests, no huge native
   request or fabricated handle.
2. Open exact plan; validate actual==planned and ≥logical, full VM leaf/page/ulong
   compatibility and device cap; raw-zero every actual byte before a Rust view
   or import. Create import only then. Private Buffer field precedes Block and
   both precede the borrowed device's end → verify: real poisoned backing through
   production initializer, actual extent capture, padding and source drop-order.
3. Expose only logical initialized bytes. Newly opened table contains zero rows;
   hash semantics are established only by successful build, not by allocation.
   No mutable/raw/native accessor or Clone; inherited !Send/!Sync → verify: precise
   nightly trait/lifetime/private-field and live-view exclusion diagnostics.
4. Reject unavailable/threads=0 before mutation. Effective workers=min(threads,n),
   disjoint chunks of the logical prefix only. Set unavailable before work;
   private scoped-worker helper uses fallible Builder::spawn_scoped, automatic
   scope joining and catch_unwind around the entire scope. No handle Vec, detached
   thread, public worker callback or leaked borrow. On any spawn error/panic all
   started workers finish before returning; only total success restores access
   → verify: independent literal rows, varying workers/heights, last partial chunk,
   padded-tail canaries, zero-worker atomic rejection and injected worker panic
   through the same private state/join path. Actual OS spawn failure remains a
   source-proof path unless a safe deterministic witness becomes available.

The table payload never uses Vec. Thread runtime/control allocation is acknowledged;
do not retain the old claim that argv is the only heap allocation. Each chunk's
row range is bounded by validated n≤2^32, making u32 row serialization exact.
Worker count is bounded by the logical row count; no enormous worker/native test.
Hashing stays the existing simplified eight-byte LE generator, not full Autolykos.

## Probe completion

1. Borrow external Gpu in both owners; Probe owns private queue/pipeline and an
   eight-byte accumulator made with buffer_with_data([0;8]). Validate its shared
   pointer/length and atomic-u32 alignment before any view. No replaceable device
   or accumulator alias → verify: API/auto-trait compile failures and source proof.
2. run checks both availability flags, native device identity, then count=0 returns
   zero without submission. Positive count rechecks table geometry/cap, pipeline
   and actual device width/height/depth for fixed width 64. Exact count threads;
   manually serialize all 24 parameter bytes including zero pad, no transmute
   → verify: pure extreme counts/widths, count-zero no-op, odd/partial real grids.
3. Allocate command/encoder and prepare parameters before changing availability.
   Reset private accumulator, bind known shader/imports, finish encoder. Set both
   owners unavailable; submit then immediately wait without a callback, allocation
   or fallible step. Check actual native status/error; only STATUS_COMPLETED plus
   None restores both and returns copied u64. Other status/error yields Gpu error
   including actual status; no further reads/builds/probes allowed → verify:
   successful reuse plus simulated completion matrix through production helper.
4. Encoder declared after command; both live through wait/status and drop before
   exclusive borrows end. Table import outlives them; Buffer drops before Block.
   No async token, retained command export or Drop-dependent fence exists, so
   forgetting an idle owner leaks storage without releasing an active backing
   → verify: full provider/source inspection; no actual GPU fault or cross-device
   witness claimed on one-device hardware.

Pre-submit errors preserve table contents and usability. Worker failure and GPU
completion failure are terminal even though initialized memory remains allocated.
Count-zero still checks failed state and device identity first. No shader changes.

## CLI, spec and exact scope

- Migrate rtable CLI to fallible Result flow, one external Gpu, mutable table and
  Probe, copied checksum result; remove raw-ID/accumulator plumbing. Validate
  a present log2N as an integer≤32 before shifting, with checked byte reporting;
  absent arguments retain defaults. Preserve seeds,
  counts, trials and benchmark loops; only explicit bounded verify runs in gates.
  Setup/native/checksum failure exits nonzero, never becomes a throughput sample.
- Compact specs §5 ownership text to cover Blake batches and RTable/probe,
  initialized full backing/logical views, exclusive build/dispatch and terminal
  failure. Preserve the same-physical-pages requirement exactly; observed VM,
  timing and policy limits belong in audit. No broader product invariant rewrite.
  Proposed replacement:

  ```text
  Blake batches and R-tables own fully initialized backing for fixed nonzero
  record counts. CPU and GPU access must address the same physical pages.
  CPU views and table builds cover logical records. Private native imports remain
  owned through synchronous completion; conflicting CPU/device access is excluded.
  Partial table builds and failed completion make affected owners unavailable;
  pre-submit rejection preserves usable input.
  ```
- Planned production: `rs/rtable-bench/src/{lib.rs,main.rs,table.rs,probe.rs,
  import_memory.rs}`. Small private worker/layout helpers may stay in table.rs;
  private tests go under `src/{table,probe}/tests.rs`; native integration in
  `tests/owner.rs`, literal fixture/support split if needed. Every new/changed
  Rust source ≤500 lines. Copy the committed private native helper locally with
  RTable-specific error wording; no cross-crate include or shared API extraction.
- Also task research/plan/execution, one compact `specs/README.md` replacement,
  and audit receipts. No edits to mine or existing Blake production. Full mine
  source is review input and an explicit incompatible reverse consumer.

## Verification and final review

Exact-origin archived siblings and unchanged lock, pinned rustup 1.95.0 actual
cargo/rustc/rustdoc/rustfmt/Clippy PATH; jobs≤2, private targets, resource guard.
No original dirty tree input. Store every command, exit, complete raw streams,
source/lock hashes and initial failures. No legacy raw-owner runtime.

- Pure tests cover n=0/1/2^32/2^32+1/u64::MAX, usize/page boundaries, effective
  workers and checksum-index reduction. Invalid native owners are never created.
- Native tests: n=1,3,65,513, heights 0/0x07060504, workers 1/2/3 plus usize::MAX
  only on n=3. Compare all logical row bytes and literal checksum cases from the
  independent Go oracle; include published BoringSSL eight-byte row-function
  literal, full initialization, immutable padding, repeated builds and probe
  counts 0/1/2/63/64/65/257. No skipped setup failures. Capture a real Block's
  actual extent before transferring it to the initializer.
- Real nightly-2025-11-26 compiler AND rustdoc checks expected E0277 !Send/!Sync,
  E0505 device lifetime, E0616 raw-field privacy and E0502 live-byte exclusion
  across build/probe. Stable doctest success alone does not prove error codes.
- Focused debug/release tests; owned fmt; `cargo check -p rtable-bench --all-targets`;
  selected all-target `cargo clippy -p rtable-bench --all-targets --no-deps -- -D warnings`;
  strict `cargo doc -p rtable-bench --no-deps` with RUSTDOCFLAGS=-Dwarnings; all locked/offline.
- Native CLI `cargo run -p rtable-bench --bin rtable-bench -- verify 4`; reject
  invalid exponent without huge allocation. No default invocation or bench sweep.
- Named workspace test and RUSTC_BOOTSTRAP=1 release build; check/Clippy/fmt
  diagnostics. Capture mine's new field/lifetime/API errors separately from
  Honey-induced old raw errors and masked chime; no whole-workspace green claim.
  Compare unchanged formatter sections against exact parent, not suppressed output.
- Direct bounded miner check/tests and CLI 512-nonce difftest follow dependency
  scope; no mining/share/network/powermetrics, huge epoch buildbench or packaging.
  Those unexecuted named gates remain explicit limits.
- Explicit patch semver diagnostic against latest Erga tag v0.30.0 with historical
  Honeycrisp 7b148906 and its recorded sole lock normalization if current provider
  cannot compile the old API. It is a diagnostic parent, never a release closure;
  no runtime there. Catalog every source break beyond tool findings.
- Freeze all changed source paths/hashes; root runs fresh different-vendor CODE
  review. Fix blockers only with revised evidence. Root owns audit/decision label,
  commit/PR/merge; source/spec changes and red gates require that explicit decision.

Five principal failure modes: (1) pad/row-domain confusion or overflow, (2) forgotten
worker/early spawn failure leaves access racing with writes, (3) native alias or
wrong device escapes the owner, (4) unsuccessful command is published as success
or owner Drop frees active backing, (5) mine breakage/weak XOR oracles are hidden
behind scoped green tests. The gates and explicit limits above address each.

Mine follow-up projection remains the research file's open design: consume the
built table into a closed immutable concrete owner, prove concurrent read-only
CPU/GPU uses and exact import extent, then repair its output/texture ownership.
No speculative safe callback or native escape is added in this unit.
