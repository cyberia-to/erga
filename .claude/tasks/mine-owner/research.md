# Mine owner research

Stage draft; research/PLAN only. Serves Kadek foundation row 6 native ownership;
discovered-from Honeycrisp PR14 and Erga PR2's remaining mine consumer errors.
The next concrete unit is the benchmark's read-only table binding, private GPU
output/texture, and sole CLI. Native Image, Bita, physical/pinning policy,
protocol conformance, mining and performance remain outside this unit.

## Exact inputs and baseline

Owned `/Users/master/cyber/erga-unimem-mine-owner`, branch `fix/unimem-mine-owner`,
origin/main `8dcb33888868d74ddfda3eeeb9bc90e58677b394` (PR2 merge; source
`210a59ea68f8a9268fdec5177a7b4492a56141d3`). Committed provider closure:
Honeycrisp `9cbd48070a136e24711896c930670fcff8a147fc`, Strata
`494df7a0973ae10a6cc1c2022e644d70449a29d6`. Fresh remote HEAD inventory agrees.
Current dev and Erga CLAUDE/spec read. Original working trees are untouched.

`/tmp/erga-mine-owner-plan-v1` holds git archives, per-file checksums, source,
commands, complete streams and independent oracle. `erga-mine-{prepare.nu,
gate.nu,inventory.rb}` under /tmp reproduce the preparation. Exact rustup 1.95.0
PATH/RUSTC/RUSTDOC, jobs 2, isolated target; unchanged lock SHA
`606817eddfdf749ea453cadcaa37838d1c9e4da820b13edf5be715377d214973`.

| command, both `--locked --offline` | exit | observed |
|---|---:|---|
| `cargo metadata --format-version=1` | 0 | 12 local packages inside exact archives |
| `cargo check -p mine-bench --all-targets` | 101 | eight library errors: three E0609, five E0616; repeated for lib-test |

The errors are at mine lib 257/307/444 (`n`), 272/274/285 (`buffer`), 306/443
(`block`). These are new RTable API consequences, following the earlier
Honey-induced raw-access errors. Main's lifetime/mutability/raw-view errors and
old chime failures remain masked; no claim that they disappeared. No legacy
native owner executed. Baseline verifier checks 871 committed archive paths,
46 Rust inventory entries, two records/four streams and the unchanged lock.

Before preparation, five completed owned RTable targets alone were deleted:
4,666,436 KiB total, with before/after/path/process receipts in
`/tmp/erga-mine-owner-preparation-20261010`. Logs/source/capsules remain. The
initial process guard matched its own command text and deleted nothing; the
corrected anchored executable guard then allowed cleanup. No other target touched.

Fresh 22-origin source scan finds the actual dependency only in Erga. Zoya's
`crates/progpow/src/gpu.rs::GpuMiner` is an independent same-name type; its local
manifest/imports disambiguate it. All zero matches and revisions are retained.
This is a bounded source inventory, not a whole-organization/rung build. Soft3
`18fd6b22ec45725aa776e37d377c5a75aeeaf553`, components SHA
`02aae8e6f2def3e23c124c76be6cc52b01cc54e86ab514c0a519dcbaf7c21e8e`,
contains no Erga/mine row; no registry/kelvin result is invented.

## Complete caller and unsafe boundary inventory

Only `rs/mine-bench/src/main.rs` consumes mine's public Rust API in the measured
closure. Full lib/main/texture FFI, all nine shaders, RTable and native provider
bodies are PLAN inputs; every committed Rust file has explicit zero/match rows.

- `lib.rs` 195–296: mutable public device/variant, caller-owned arbitrary output,
  unconstrained RTable bindings, unchecked width/rounded grid, parameter transmute,
  and Dispatch wait without status. V8 can omit its second binding or use a
  different row count. Public variant can disagree with the compiled shader.
- `lib.rs` 117–172: safe `mine_one` accepts arbitrary n and slice; modulo zero,
  checked/saturating slice behavior does not protect the preceding raw pointer
  `add` used for prefetch. n×32 must match admitted initialized bytes before any
  pointer arithmetic. `cpu_accumulate` also uses nonwrapping nonce addition.
- `lib.rs` 322–423 and `texture_ffi.rs`: V9 takes arbitrary public texture/output/n;
  public safe upload accepts an arbitrary raw pointer. Descriptor/index integer
  ABI is hand-written, dimensions multiply unchecked, no extent/format checks,
  no bounded autorelease pool, and completion errors are discarded.
- `lib.rs` 433–502 and main 265–314: timed CPU/hybrid workers terminate only when
  the main path sets a flag. Spawn failure, invalid-duration panic or GPU failure
  before that assignment can strand workers while scope waits. Private scoped
  cancellation must run on all exits; GPU completion must not depend on Drop.
- main 43–55/70–116/122–203: table construction and repeated variant sweeps; raw
  fields/output helpers; separate Gpu objects; old factory lacks a device lifetime.
  main 220–419: CPU/GPU/hybrid/profile/sustained loops. main 422–512: V9 copies
  table bytes into a distinct texture, compares only V1 GPU XOR, then benchmarks.
  Unsupported shape or failed comparison currently returns success. Present
  invalid numeric args silently fall back; exponent shifts are unchecked.

The existing files are 502/545 lines; migration needs small reference/owner/CLI
modules, each changed/new Rust file ≤500. Shader algorithms and benchmark count,
trial and group sweeps stay fixed; no benchmark/mining/default CLI runs in gates.
The actual `erga-miner` protocol engine has separate owners/kernels and only uses
Blake reference. Repairing this benchmark does not assert protocol correctness.

## Smallest retained binding

RTable already owns the exact initialized actual Block and one private Buffer,
with Buffer declared before Block; logical bytes are rows×32. It exposes no safe
native alias. Reuse that existing import through one reviewed unsafe method,
`read_only_buffer(&self, &Gpu) -> Result<&Buffer, rtable_bench::BenchError>`.
It checks availability, native device identity and existing actual VM/import
preflight. Caller must retain the table/device through every native use, forbid
all writes/native alias escape and conflicting builds, limit shader reads to
logical rows, and wait before releasing/rebuilding. Buffer::write, raw contents,
retained commands and other threads are part of these obligations. Its name
alone supplies no read-only guarantee; this is an explicitly unsafe boundary.

The concrete safe GpuMiner borrows immutable table(s), keeps output, texture,
pipeline and device private, and exposes synchronous mutable dispatch plus copied
results. Borrowing retains existing imports/backing and excludes build/drop;
known const shaders permit concurrent immutable CPU reads. It reuses one table
across CLI variants without a release/reconfigure API, repeated large builds,
new shared-owner type or duplicate native import. Consuming RTable was considered
but would add those mechanisms solely to recover existing reuse. No self-reference
is stored: table buffer is borrowed only while binding a synchronous command.

Failed completion permanently closes executor/output, including count-zero runs.
Both tables are immutable inputs to these specific shaders; their initialized
bytes remain usable after completed read-only work. RTable's mutating build/probe
failure policy remains unchanged. No table is marked successful after a write
failure. Borrowed ownership plus adjacent submit/wait retains all native objects;
forgetting an idle executor cannot create future native access to released bytes.

## Native and shader proof

Existing SDK26.4.1/XNU/Apple import, CPU-commit and GPU-completion primary evidence
from PR1/PR2 is retained. New official method pages, exact SDK header snapshots,
URLs/hashes and selected attributed excerpts are in `primary/`. Full originals
stay in /tmp, never vendored into the public repository.

- All nine inputs are const device buffers or read-only texture. V1 reads four
  ulong lanes per row; V2–V8 use ulong4, whose MSL size/alignment is 32 bytes
  (MSL4.1 pages28–29). Check actual table pointer alignment32 before binding;
  runtime page/whole-region validity remains RTable's separate preflight.
- n is 1..=2^32; every index is a u32 reduced modulo n, hence within logical rows.
  V8 needs two equal row counts. Different contents are safe: even local gid reads
  A, odd reads B. Tests use different heights to prove the second binding matters.
- Exact grid ceil(count/k), k=2 for V2,4 for V5,1 otherwise, avoids surplus padded
  uint lanes. With count≤u32MAX, last k×gid+(k−1)≤u32MAX for k=2/4. V2 computes
  the unselected odd tail but suppresses its XOR; its table index stays bounded.
  Nonces wrap modulo2^64 on GPU; CPU/reference adopts explicit wrapping arithmetic.
- Apple M-series supports nonuniform dispatch; current official API describes
  arbitrary grid size and publishes no lower compute-grid bound/query in these
  sources. Check actual pipeline/device group limits separately, not a made-up
  memory-derived grid cap. Eight atomic_uint output lanes require 32 initialized
  shared bytes and pointer alignment4; serialize all 56 parameter bytes manually.
- V9 shader fixes width16384, two RGBA32Uint texels per logical row. Admit positive
  multiples of8192 rows, height=rows/8192≤16384, total rows≤2^27; this conservative
  profile fits Apple7–10. Query actual returned dimensions/depth/format. Upload
  exact initialized logical bytes before any GPU work and retain private texture.
  It remains a one-time copy, never a zero-copy/physical-sharing claim for texture.
- Official replace-region documentation says the CPU copy is immediate and supplies
  no GPU synchronization. New texture has no outstanding access. Fixed nonmipmapped
  slice0 is initialized completely. Private FFI uses exact NSUInteger/BOOL and
  setTexture ABI; existing aruminium::autorelease_pool scopes the descriptor while
  Gpu::texture returns its retained object. No provider API extension is needed.

## Independent bounded evidence

`oracle/main.go` imports no Erga/Kadek/native code. Go1.26.3 with cached x/crypto
v0.48.0 (`e08b06753d6a72f1fe375b6e0fefefb39917c165`) uses scalar indexing and
math/big sum reduced modulo2^256, distinct from the Rust/MSL limb implementation.
Offline readonly execution generated 360 XOR cases and 18 individual digests:
rows1/3/65/8192/16384, counts0/1/2/3/4/5/63/64/65, base0 or u64MAX−1, reference,
V6, V7 and V8 with distinct second-table height. These describe the simplified
benchmark, including diagnostic variants; they are not Autolykos vectors.

The original Nu wrapper successfully captured Go exit0/source/tool/output hashes,
then its copied RTable-only verification queried a nonexistent rows field. That
wrapper/output/source are preserved under oracle/initial. A structural check
verified unchanged output and all40 empty XORs. The final successful replay adds
an explicit published BoringSSL eight-byte KAT and nonce0/16384-row index witness
that reaches texture y=1; current main.go/output/receipt hashes agree. Go reran
once for these additional checks, not to replace a failing candidate result.
No candidate GPU result exists yet. Tests must use individual count1/full32-byte
literals as well as XOR batches, fail on setup, and stop for a new scope if an
unchanged kernel disagrees; do not silently fix an algorithm in this owner unit.

The full proposed spec and exact diff are prepared only in the task evidence
root's `proposed/` directory; the physical-pages clause is verified unchanged.
No production specification has been edited before review.
