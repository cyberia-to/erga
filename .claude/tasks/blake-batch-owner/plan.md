# Close the Blake benchmark batch owner

Stage draft; awaits root and fresh different-vendor PLAN approval. No production
changes. Base Erga848e17e2522db2f6225ad72d9b1797f8735d26d0; Honeycrisp9cbd4807,
Strata494df7, exact full pins/source evidence in research.md. Serves Kadek native
Image ownership prerequisite; discovered-from Honey PR14, not a phase completion.

## Smallest complete boundary

Replace arbitrary public ZeroCopyBuf pairs with one closed Batch for count
32-byte input/output records. Remove ZeroCopyBuf public type/constructor/fields/
iosurface_id; no migration shim, generic memory trait or owner framework.
Production miner and other crates only use unchanged reference::blake2b256.
RTable/mine-bench remain explicit next owner unit; no unsafe annotations there.

```text
pub struct Batch<'a> { private input/output Storage, &'a Gpu, u32 count, failed }
GpuBlake2b::alloc_batch(&self, count:u32) -> Result<Batch<'_>,BenchError>
GpuBlake2b::dispatch(&self, batch:&mut Batch<'_>, width:usize) -> Result<(),GpuError>
Batch::count(&self) -> u32
Batch::inputs(&self) -> Result<&[[u8;32]],GpuError>
Batch::inputs_mut(&mut self) -> Result<&mut [[u8;32]],GpuError>
Batch::outputs(&self) -> Result<&[[u8;32]],GpuError>
fill_inputs(batch:&mut Batch<'_>, seed:u64) -> Result<(),GpuError>
verify_zero_copy(batch:&Batch<'_>) -> Result<usize,GpuError>
```

Preserve Variant variants/methods and GpuBlake2b::open/open_v1. Make gpu/variant/
width fields private; supply read-only variant(), max_threads_per_group(),
thread_execution_width() for the existing CLI. No device/native-buffer getter.
BenchError retains Gpu/Mem variants and From conversions; add Display/source.
Post-construction methods return GpuError, not allocation wrapper. Logical bytes
start initialized; outputs before first dispatch are zero bytes, not a promised
hash. Only a successful dispatch establishes the hash relation for then-current
inputs; mutating inputs does not silently recompute output. No extra timing or
validity metadata. Failed completion closes every view and dispatch; only count
remains inspectable. Drop/recreate is recovery; no reset flag or raw adoption.

## Admission and lifecycle

1. Count0→BenchError::Mem(ZeroSize); checked count*32, page roundup and slice-limit
   overflow→Mem(SizeOverflow). u32 API pins shader domain. Positive runtime page
   discovery and buffer-capacity failures→Gpu(BufferCreationFailed) with precise
   detail. Block::plan for each identical extent; reject device max before native
   allocation. Propagate native MemError, no fixed guessed cap.
   → Pure arithmetic/count/width tests; no enormous native allocation.
2. Private Storage owns Buffer first, Block second. Before forming views/import,
   check actual==plan, actual>=logical, native page multiples, nonzero8-aligned
   address, checked end and current leaf read/write VM containment. Fallible
   getpagesize/packed short-info48/4/32/count12 FFI, fields by value, no runtime
   assert/unwrap. Raw-fill exactly actual extent zero. Only then unsafe wrap.
   → Same private initializer on real poisoned allocation; full actual zero,
   logical slice bounds and padding canaries; no fabricated Block.
3. Build two disjoint Storage owners and retain a private &Gpu lifetime from
   alloc_batch. All setup is fallible; a later error drops earlier imports before
   their Blocks. No owner published until complete. Borrowed device retention
   keeps native identity alive; dispatch compares it with its own device before
   encoding and rejects a mismatch without changing bytes/state.
   → Error-source tests and local/field drop-order review. Device mismatch is
   source-reviewed when the host exposes no alternate device; never fabricate
   an invalid Gpu or native handle to exercise it.
4. Views use alignment1 fixed byte records over exact count*32 initialized prefix.
   Shared reads coexist; inputs_mut requires &mut Batch. No Block/Buffer/raw/ID
   escape or caller-controlled count. Auto traits inherited: Buffer makes Batch
   !Send/!Sync; no unsafe impl. Nightly E0277 witnesses pin both restrictions.
   → Nightly E0499/E0502: input mutable/shared or output shared view across
   dispatch rejected; E0505 prevents dropping the factory while its batch lives.
5. Dispatch first rejects failed state, then actual current mapping/device extents
   and width0/>pipeline limit/>device width (height/depth>=1). Errors before submit
   preserve bytes and usable state. Derive hpt from private compiled Variant.
   Compute ceil as count/hpt + u32::from(count%hpt!=0), without overflow-prone
   count+hpt-1. Launch that exact grid, no group roundup: V1/2 gid<count; V3/4 highest
   gid<=2^31-1 implies gid*2+1 fits uint even for count=u32::MAX. Retain odd-partner
   shader guards unchanged. Apple Silicon supports nonuniform groups; use the
   primary arbitrary-grid contract, no invented grid-limit query/cap.
   → Pure extreme-count proof tests plus actual small odd/partial groups.
6. Replace status-blind Dispatch helper with Queue/Commands/Encoder. Retained
   batch imports outlive the exclusive borrow; declare command then encoder,
   so encoder drops before command, both before Batch can drop. Bind exact private
   buffers, push count.to_le_bytes() at index2, encode/finish, failed=true;
   adjacent submit(); wait(); with no fallible
   work/callback/panic gap. Inspect status and native error after wait. Only
   STATUS_COMPLETED+None clears failed; otherwise return CommandBufferError
   containing status/native detail. Later views return fixed unavailable error.
   Error formatting panic is after wait with failed still true. No generic guard.
   → Actual completion helper injected-observation tests and repeated denial;
   successful native test covers real submission, never malformed GPU execution.

Metal lifetime/source obligations remain real even if native calls error. Private
Storage Buffer retention does not itself make CPU/GPU concurrency safe; exclusive
batch borrow and no exported aliases are essential. Do not weaken physical-page
requirements or claim a current VM query proves them. No provider changes.

## Caller and exact scope

Migrate only blake-bench/main.rs: mutable Batch, count-free fill/verify, checked
Result dispatch and new read-only getters. End views before submit; benchmark
loops/counts/trials stay unchanged. Print/propagate actual allocation/completion
errors; never report failed dispatch as successful timing or only unsupported
width. Keep unsupported-width branch separately meaningful. Existing verify
command4096 records ×4 variants remains bounded; default bench is never run.
Construction now zeros both actual backings; commands use checked public wrappers
instead of unchecked Dispatch. Even with retained imports, timings are not
comparable without new measurement. No performance claim or measured table edit.

Normalize Cargo.lock by deleting only obsolete libc in unimem0.2.0 dependency
entry, as independently verified in research. No package version/checksum/
manifest/dependency changes. This allows the candidate's locked closure instead
of blessing a diagnostic lock as origin. Keep initial locked failure receipt.

Add one compact benchmark-specific ownership paragraph to specs/README.md §5:
“Blake benchmark batches own fully initialized input/output backing for a fixed
nonzero count. CPU and GPU access must address the same physical pages. CPU views
cover logical records, and dispatch exclusively retains their native imports
through successful completion. Failed completion makes the batch unavailable;
pre-submit errors preserve usable input.” No mining or PA/pinning proof claim. Update blake library docs to
actual sealed owner scope; no stale future AMX/ANE/handle escape claims.

Files: Cargo.lock(one line); rs/blake-bench/src/lib.rs, main.rs;
new src/batch.rs, src/import_memory.rs, optional src/batch/tests.rs,
new tests/batch.rs (+tiny literal fixture with source/license if separate);
specs/README.md paragraph. Task docs/audit additional. Every changed/new Rust
file<=500. Unchanged reference.rs, four shaders, miner/RTable/mine/app files and
all Cargo.toml files stay out of scope. Existing reference abc false witness is
recorded, not used or repaired here.

## Verification sequence

1. Preserve frozen PLAN/primary inputs, exact origin archives/locks and initial
   compile red → verify source whitelist and approved one-line lock delta.
2. Implement private Storage/Batch/checked dispatch and CLI → owned fmt/diff;
   focused debug native/tests and actual borrow-negative nightly.
3. Independent 00..1f→cb2f5160...42503d6 BoringSSL literal for each Variant;
   varied CPU-reference rows, counts1,2,3,4,63,64,65, owner reuse, exact inputs
   unchanged and output/padding canaries. Setup failure fails tests, never skips.
   Invalid group0/over-limit returns before submit with byte/state preservation.
   Simulated native completion matrix separately labeled.
   → Full32-byte equality, release repeats, `blake-bench verify` exit0.
4. Selected strict checks, direct consumers and named repository gates → preserve
   newly exposed raw-owner compile failures and known chime failures, no widening
   to fix them. A red gate remains red, no whole-workspace pass/phase claim.
5. Freeze exact source/tests/spec/lock bytes and receipts → root fresh CODE
   review/audit/commit/PR; no source changes after freeze without correction notice.

Use rustup1.95.0 exact PATH/cargo/rustc/rustdoc/Clippy/fmt, private targets, jobs<=2,
owned targets<=20GiB/shared<=30GiB; no parallel heavy builds. Real nightly2025-11-26
rustc/rustdoc for error-code doctests. macOS Apple Silicon only; no Miri hardware
claim. Standard commands use --locked --offline:

- cargo check -p blake-bench --all-targets; cargo test -p blake-bench, --release;
  strict scoped Clippy --all-targets --no-deps -- -D warnings and strict rustdoc.
- cargo run -p blake-bench --bin blake-bench -- verify. No bench/stable/bench-once,
  no network/share/power/mining loops or huge epoch buildbench run.
- cargo check --workspace --all-targets; cargo test --workspace;
  RUSTC_BOOTSTRAP=1 cargo build --release (named gate). RTable/mine expected to
  expose existing inventoried raw accesses; capture exact failures rather than
  annotate them. If dependency masks chime runtime, retain previous receipt.
- cargo check -p erga-miner --all-targets; cargo test -p autolykos -p erga-miner
  (non-mining tests); bounded existing `RUSTC_BOOTSTRAP=1 cargo run -p erga-cli
  --bin erga -- difftest`, if its declared target/dependency closure compiles.
  Named buildbench is large epoch timing and remains unrun explicit limitation;
  packaging/bundle.nu also separate product artifact gate, no packaging claim.
- Workspace strict Clippy plus full --no-deps -W warnings diagnostics, owned fmt,
  workspace fmt baseline comparison, git diff --check. No unrelated suppression.
- Explicit patch cargo semver-checks -p blake-bench against latest tag v0.30.0 (same848e17e HEAD) plus
  compile-only old-provider7b148906 and same one-line lock normalization (labeled
  diagnostic baseline), never execute its raw legacy owner. Expect source breaks;
  catalogue actual removed/changed public signatures beyond tool coverage. No
  manifest/version/tag/publishing action in this task.

Ring0 is scoped evidence, with required known reds preserved. Ring1 has no registry
row/complete product closure and remains unclaimed. Proposed packaging/buildbench
nonexecution must be visible in audit, not described as a green repository gate.

## Five failure modes

1. Count/group arithmetic permits repeated/out-of-window shader stores → exact
   grid proof at uint maximum; real odd/partial groups; no kernel changes.
2. Borrow/public Buffer escape permits concurrent CPU/device access → closed pair,
   exclusive dispatch, readonly metadata getters and negative compiler witnesses.
3. Uninitialized tail or premature native drop → poison actual extent, private
   Buffer-before-Block field order, command completion and native tail canaries.
4. Failed native command becomes readable output/pass → terminal state before
   submit, adjacent wait, native status/error check and simulated matrix.
5. Diagnostic lock/legacy chime/masked RTable errors become false closure → exact
   lock delta, original baseline, complete inventory and explicit partial scope.
