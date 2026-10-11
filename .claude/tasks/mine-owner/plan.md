# Mine owner plan

Stage draft; implementation waits for fresh different-vendor PLAN approval.
Serves Kadek foundation row6; discovered-from Honeycrisp PR14 and Erga PR2.
Base `8dcb33888868d74ddfda3eeeb9bc90e58677b394`, Honeycrisp `9cbd4807`,
Strata `494df7a`; full pins/evidence in research. No manifest, lock, version,
provider, shader algorithm, actual miner/chime or physical-policy changes.

## Canonical API and ownership

Signature notation; all owner fields private:

```text
impl RTable<'_> {
    pub unsafe fn read_only_buffer(&self, gpu: &Gpu)
        -> Result<&Buffer, rtable_bench::BenchError>;
}
pub struct GpuMiner<'table, 'gpu>;
impl<'table, 'gpu> GpuMiner<'table, 'gpu> {
    pub fn open(gpu: &'gpu Gpu, variant: MineVariant,
        table: &'table RTable<'gpu>, second: Option<&'table RTable<'gpu>>)
        -> Result<Self, BenchError>;
    pub fn variant(&self) -> MineVariant;
    pub fn run(&mut self, m: &[u8; 32], nonce_base: u64,
        count: u32, width: usize) -> Result<[u8; 32], BenchError>;
}
pub fn mine_one(m: &[u8; 32], nonce: u64, rows: &[u8])
    -> Result<[u8; 32], BenchError>;
pub fn cpu_accumulate(m: &[u8; 32], nonce_base: u64, count: u32,
    table: &RTable<'_>) -> Result<[u8; 32], BenchError>;
```

Add MineVariant::V9Texture; preserve existing immutable enum helpers, clarify
is_real means this benchmark's CPU reference. Remove separate GpuMinerV9,
open_v1/run_dual, public texture_ffi/SHADER_MINE_V9, arbitrary output helpers,
and public timed helpers. Timed behavior moves to private CLI modules. No aliases
or release/reconfigure/shared-owner factory. All source breaks are catalogued;
only measured direct consumer is this CLI.

Mine BenchError: Table(rtable_bench::BenchError), Gpu(GpuError),
InvalidInput(&'static str), WorkerSpawn(std::io::Error), WorkerPanicked,
CounterOverflow, Unavailable, with Display/Error/source/From. InvalidInput covers
incomplete/empty/too-many CPU rows, wrong second-table shape, invalid width or
texture geometry, invalid duration/worker count/batch. Details name the actual
field. Existing RTable errors remain intact. Native failure includes actual status
and optional native detail. Failed executor has no recovery; metadata remains
inspectable. All pre-submit argument/native-preparation failures preserve usability.

1. Add only the unsafe RTable method, borrowing its existing import. It performs
   available, same_device and existing full-actual-extent validate before return.
   #Safety: prohibit CPU/raw/native writes through returned Buffer or derived
   aliases (including Buffer::write and contents_ptr); limit GPU reads to logical
   rows with required shader alignment; retain table, device, backing/import and
   any encoded/native use until completion; exclude builds/mutation; publish no
   safe alias escape. Immutable CPU reads may overlap proven read-only GPU work.
   Returning a borrow does not itself wait or constrain Metal's retained aliases
   → verify: E0133 compile-fail, full method/caller audit, existing RTable tests.
2. GpuMiner retains external Gpu and immutable table(s), private output/texture,
   queue/pipeline/variant and failure flag. Every actual raw boundary is inside
   this concrete owner. No native/raw getters, public callback, Clone, unsafe
   Send/Sync or async handle. Borrow prevents table build/drop and device drop;
   private output never shares an alias. Known shaders read table inputs only
   → verify: actual nightly E0277 traits, E0505 backing/device lifetime, E0502
   conflicting build, E0616 private native/output fields.
3. Constructor accepts second iff V8, requires equal row counts and actual native
   device identity for both, and checks logical bounds/32-byte input alignment.
   It borrows the existing RTable Buffer only during validated binding; no stored
   self-reference or second import. V9 privately initializes its texture instead.
   Constructors consume no table, so failure leaves caller inputs usable
   → verify: missing/extra/mismatched second table, real initialized/unbuilt tables,
   no fabricated GPU handle; cross-device rejection source-only on one-device host.

Only executor/output becomes unavailable after failed read-only completion.
Borrowed tables remain initialized read-only inputs; existing RTable build/probe
failure policy is unchanged. No failed GPU-written output is published. Table
lifetime extends past all executor fields and submitted work, including error
returns. Forgetting an idle executor leaks owned native objects without creating
future commands. Foreign exceptions/process abort are outside recoverable errors.

## Dispatch and texture

1. Build queue/pipeline for immutable variant; private accumulator comes from
   buffer_with_data([0;32]). Check exact size, shared pointer and alignment4 before
   any view; all lanes initialized. Parameters use explicit 56-byte LE writes and
   zero pad, no struct transmute → verify: full parameter bytes and actual output
   storage tests, source lifetime/drop order, no raw output export.
2. run checks Unavailable first; positive width and actual pipeline/device x/y/z
   limits; retained table availability/identity/actual mapping and alignment.
   count0 then returns copied zero without submit/reset. Positive count launches
   exactly count/k + (count%k!=0), k=1/2/4; checked native conversion, no padded
   grid. Nonces wrap modulo2^64 in CPU and GPU. No fabricated compute-grid cap
   → verify: pure count0/1/u32MAX and width0/max boundaries; native odd/partial
   counts and wrap-near-u64MAX; no enormous allocation/dispatch.
3. Create command then encoder, reset private accumulator, bind proven inputs and
   params, encode/finish. All fallible work precedes failure flag; set failed,
   submit then immediately wait, with no callback/allocation/Result/panic gap.
   Restore only for STATUS_COMPLETED and no native error; then copy all32 bytes.
   Other status/error is terminal, including later zero-count run. Encoder and
   command stay alive through wait/status, and drop before owner borrows end
   → verify: native reuse plus simulated completion matrix through private helper;
   pre-submit invalid width preserves output/usability. Real GPU failure and
   destruction timing are source/simulated evidence, not injected-native claims.
4. V9 geometry is fixed width16384, RGBA32Uint16B, mip0/slice0, rows multiple8192
   with 1≤height=rows/8192≤16384 (rows≤2^27). Check arithmetic/extent before native
   allocation, then actual width/height/depth1/pixel format. Private descriptor
   FFI uses SDK NSUInteger/BOOL, literal C strings and existing autorelease_pool;
   returned texture retained independently. Private upload accepts &[u8] and
   verifies exact logical length; immediate CPU replace_region initializes the
   complete sole mip before GPU work. Use bytesPerRow0 for height1 as official
   method guidance requests; otherwise 262144. Bind private texture via exact
   setTexture:atIndex: ABI. No arbitrary texture/pointer callback/export
   → verify: pure geometry edge cases; real8192/16384-row upload/readback or GPU
   full digest across texture-row boundary; descriptor failure stays an error.

No GPU shader algorithm changes. Existing V6/V7 are diagnostic algorithms; V8
selects different equal-length tables by local gid parity. V9 remains a copy.
Native proof reuses pinned Apple arbitrary-grid/nonuniform, same-region/import,
commit/completion contracts plus new texture/ABI evidence. Canonical physical
pages requirement remains unchanged; observations/limits belong only in audit.

## CPU reference and CLI

1. mine_one derives n from slice length; reject empty, nonmultiple32 or n>2^32
   before pointer arithmetic. Private reference kernel accepts only this validated
   shape, keeps existing prefetch/add/hash algorithm, and uses checked in-bounds
   offsets with exact row decoding. cpu_accumulate validates table bytes once and
   uses wrapping nonce addition. No unchecked public rows/count constructor
   → verify: invalid slices including31/33bytes, independent full digest literals,
   count0, and wrapping batches. Safe slice input itself carries byte lifetime.
2. Migrate all CLI commands to one borrowed Gpu and new owner/copy outputs; use
   table.row_count/bytes and mutable build with error propagation. Preserve chosen
   benchmark counts/trials/groups; no timed result after setup/dispatch/comparison
   failure. V9 unsupported shape or mismatch exits nonzero. Numeric args present
   but invalid reject instead of silently selecting default; exponent≤32 before
   shift; finite positive durations/windows convert fallibly with checked deadline.
   No default/benchmark/sustained/profile/mining execution in gates
   → verify: small verify command and malformed/overflow argument diagnostics.
3. Keep timed CPU/GPU/hybrid functionality in private CLI modules. Validate positive
   threads, batch and duration/deadline before workers. A private scoped-worker
   helper uses Builder::spawn_scoped; a stop guard declared INSIDE the scope closure
   before spawning signals every exit (success, spawn error, GPU error, panic).
   Workers return (), with shared atomic fault bits for rejected CPU reference
   input and counter overflow; every fault also sets stop. Checked local increments
   and AtomicU64::fetch_update avoid counter wrap. Each worker catches a panic only
   to set stop, then resumes it; catch around the whole scope observes it after
   all joins. Scope joins independent of handle Drop; no returned worker Result
   can disappear in a dropped handle. Publish total only after joins, with fixed
   precedence: WorkerPanicked, main/spawn/GPU error, CounterOverflow, then reference
   InvalidInput. CPU workers receive initialized immutable bytes, never native
   owners, and check stop at most every1024 hashes. Main GPU loop checks stop;
   CPU-only wait uses bounded polling (≤10ms) so errors need not wait the requested
   duration. GPU runs synchronously on the calling thread; neither submit/wait nor
   backing safety relies on the stop guard. No callback or handle escapes
   → verify: private deterministic spawn/panic/main-error/reference-error and
   counter-overflow seams confirm started
   workers joined; finite native concurrent CPU-read/GPU-read witness with copied
   independent results. Actual OS spawn failure remains source-only unless observed.
4. Since borrowing excludes writes but permits shared reads, existing hybrid
   overlap needs no RTable mutability escape or new public session type. A worker
   panic/read error changes no table data. End every worker before returning an
   executor completion error; no outstanding work accompanies a reported result.
   Timed accumulator resets/copy publication now happen per bounded run; record
   this timing-scope change without benchmark/performance equivalence claims.

## Exact source scope and checks

- Production: `rs/mine-bench/src/{lib.rs,reference.rs,owner.rs,texture_ffi.rs,
  main.rs,cli.rs,cli/bench.rs,cli/timed.rs}`; private tests under matching child
  directories, integration `rs/mine-bench/tests/{owner.rs,vectors.rs}` or a small
  support submodule. Existing502/545-line files split only along these concrete
  reference/owner/CLI duties. Every changed/new Rust file≤500lines.
- RTable: `rs/rtable-bench/src/table.rs` and its private tests only, narrow unsafe
  import borrow + rustdoc. Narrowly update existing “no native alias”/“excludes GPU”
  prose to distinguish safe methods and conflicting access; initialization/build/
  probe behavior stays unchanged.
- Exact specs §5 replacement below. `/tmp/erga-mine-owner-plan-v1/proposed/`
  carries complete proposed README.md, compact specs.diff and hash/unchanged-clause
  verification; repository specification remains untouched before PLAN approval.

  ```text
  Blake batches and R-tables own fully initialized actual backing for fixed nonzero
  logical record counts. CPU and GPU access must address the same physical pages.
  Builds/views cover logical records; native objects stay retained through
  synchronous completion, excluding conflicting access. Mine executors retain
  immutable table/device borrows and initialized private output; their CPU/GPU
  table reads may overlap. V9 owns an initialized texture copy of logical bytes.
  Partial builds and failed Blake/probe completion close affected owners. Failed
  mine completion closes executor/output; read-only tables remain initialized.
  Pre-submit rejection preserves usable input.
  ```
- Task docs/execution and root-owned audit/review/merge. No dependency additions,
  manifests/lock/version edits, shader edits, provider fix or unrelated formatting.

Gates use exact-origin archives plus explicit reviewed candidate overlay, unchanged
lock, rustup1.95.0 exact tool PATH/RUSTC/RUSTDOC, jobs≤2; own active targets≤20GiB,
combined≤30GiB. Record every command/exit/full stream, source/tool/lock hashes and
initial failure. No unproved legacy runtime or dirty-sibling build input.

- Pure errors/layout/grid/serialization/reference and private completion/worker
  tests; no invalid native handles, huge allocations or unbounded benchmark loops.
- Real V1–V9 full32-byte output: tables1/3/65 for buffers,8192/16384 for texture;
  counts0/1/2/3/4/5/63/64/65, nonce base0/u64MAX−1, repeated runs and widths1/64.
  Use independent Go literals for reference/V6/V7 and distinct-height V8, including
  individual count1 witnesses that cannot hide behind XOR cancellation. At least
  one nonzero texture y row is selected. Fail on native setup; zero skips.
- Real concurrent immutable CPU/GPU reads with finite counts and joined workers;
  RTable existing initialization/padding/build/probe tests repeat after boundary
  addition. New !Send/!Sync/private/lifetime/unsafe-call compile-fail docs use actual
  nightly2025-11-26 compiler AND rustdoc to verify diagnostic codes.
- Focused debug/release tests for mine and RTable; all-target check; owned rustfmt;
  `cargo clippy -p mine-bench -p rtable-bench --all-targets --no-deps -- -D warnings`;
  strict no-deps docs with RUSTDOCFLAGS=-Dwarnings. All locked/offline.
- Bounded `cargo run -p mine-bench -- verify 4`, plus planned native texture tests
  rather than V9 CLI's benchmark sweep; invalid exponent/NaN/infinite duration
  reject before allocation. Existing RTable `verify 4` gate remains meaningful.
- Named workspace test, RUSTC_BOOTSTRAP=1 release build, check/Clippy/fmt diagnostic
  gates, paired exact-parent outcomes. Distinguish discharged RTable-induced mine
  errors from newly unmasked old chime/other failures. No whole-workspace-green
  claim if red; compare unchanged formatter diagnostics rather than suppress them.
- Direct bounded miner checks/tests/difftest follow actual dependency closure;
  buildbench, packaging, mining/network and performance sweeps remain explicitly
  unrun. No release/rung result when component graph has no Erga row.
- Semver patch diagnostic against latest v0.30.0 using its exact historical
  compatible provider/lock-normalization receipt if needed; no old runtime. Also
  run source API-break inventory, including raw/timed removals and enum addition,
  since tool findings do not enumerate every consumer effect.
- Freeze complete paths/hashes, full source/primary/gate CODE inputs; root requests
  fresh different-vendor CODE review and owns audit/decision label/commit/PR/merge.
  Kernel disagreement is a new blocking scope, never a quiet ownership repair.

Five failure modes: (1) unsafe borrow escapes or backing/device dies during work;
(2) row/vector/texture/grid overflow admits out-of-bounds reads; (3) bad native
completion publishes output/reopens executor; (4) a worker survives error/forget
and scope never finishes; (5) shared Rust/GPU mistakes, diagnostic variants, weak
XOR witnesses or newly unmasked failures are reported as protocol/full closure.
