# Blake benchmark owner research

Stage draft; no production authorization. Serves Kadek foundation row6 native
Image ownership prerequisite; discovered-from Honeycrisp PR14 raw Block boundary
repair and its reverse-dependent audit. This unit repairs a concrete Erga owner;
it does not close Kadek row6, all Erga consumers, mining or physical/pinning policy.

## Exact inputs and bounded observations

Owned worktree `/Users/master/cyber/erga-unimem-bench-owners`, branch
`fix/unimem-bench-owners`, origin/main848e17e2522db2f6225ad72d9b1797f8735d26d0.
`git ls-remote --symref origin HEAD` and fetch confirmed this2026-10-10. Original
tree untouched. Latest tag v0.30.0 resolves to the same848e17e HEAD
(`git rev-parse v0.30.0^{}`). Read current cyberia/dev.md and Erga CLAUDE.md/specs/README.md.
Honeycrisp origin9cbd48070a136e24711896c930670fcff8a147fc;
Strata origin494df7a0973ae10a6cc1c2022e644d70449a29d6.

`/tmp/erga-blake-batch-plan-v1/` contains committed archives/manifests, exact
commands/streams, primary vector and scans. Preparation `/tmp/erga-blake-prepare.nu`,
runner `/tmp/erga-blake-gate.nu`, verification `/tmp/erga-blake-inventory.rb`.
Rustup1.95.0, matching PATH/RUSTC/RUSTDOC, jobs2, private target; no old raw owner
runtime was executed. Three fresh commands only:

| Cargo command, all --locked --offline | exit | observed |
|---|---:|---|
| metadata --format-version1, original lock |101|lock update required|
| metadata --format-version1, retained diagnostic lock |0|12 local packages, private sibling closure|
| check -p blake-bench --lib, same diagnostic lock |101|three E0133 plus one E0596|

Original lock SHA4f9da5cf08c519b0bea192bf33b96e5ccf984c29b3fb987779c07f3e526a2198;
diagnostic606817eddfdf749ea453cadcaa37838d1c9e4da820b13edf5be715377d214973.
The exact sole delta deletes obsolete `libc` from unimem0.2.0's dependency list;
no version/checksum/other package changes. This diagnostic lock is not committed
origin. A reviewed one-line lock normalization is needed for honest locked gates.
814 committed archive files checked; only that declared diagnostic delta exists.
No broad compile/test matrix or native benchmark was repeated.

Historical Honey audit has the same four newly introduced compile diagnostics:
blake-bench/src/lib.rs99 wrap,107 as_bytes_mut,114 as_bytes are E0133; line107
also E0596 through &self. They are not four E0133. Previous parent workspace
runtime reached11 passes then chime::tests::{strikes_differ,
renders_are_audible_and_unclipped} panicked app/chime.rs132 multiply overflow.
Current compilation masks those existing failures; no chime repair in this unit.
Exact historical logs live under Honey audit2026-10-10-unimem-block-access and
`/tmp/unimem-dependent-{baseline.cX82Mt,candidate.DU0t4L}/logs/erga-localresolved/`.

## Actual owner and reverse scope

All35 committed Rust files scanned with explicit zero-match rows in
caller-inventory.json (SHA bec94cd1e318fae0de21fe58cb1c114660aae884792500e0869e9910e2cfad85).
Source-backed findings:

- blake-bench/lib.rs91 exports both Buffer and Block publicly. Its safe shared
  as_bytes_mut106 permits aliasing; open96 wraps a lazy uninitialized extent
  without page/VM proof. Public fields permit independent move/drop and arbitrary
  device aliasing. Field-order comments alone do not close that boundary.
- GpuBlake2b::dispatch166 accepts arbitrary two owners, count and group width,
  and accesses output through &ZeroCopyBuf. It trusts caller extents and mutable
  public variant136; zero group width panics. Dispatch::dispatch_with_bytes202
  submits/waits, discards status/error and releases command/encoder internally.
  It cannot establish successful readback. Full provider command APIs can.
- Only blake-bench/src/main.rs consumes this owner/dispatcher API. Three allocation
  sites/helpers and all dispatch/fill/verify calls are in that file. Production
  erga-miner/src/difftest.rs19 uses only blake_bench::reference::blake2b256.
  rtable-bench/mine-bench similarly depend on the reference, not ZeroCopyBuf.
  Thus a closed Blake input/output batch is independently deliverable.
- Remaining separate owner: RTable lib46 exports Buffer/Block/n; wrap58,
  build_parallel(&self)76/byte write80 and checksum106/read107 require repair.
  Its whole-actual build can process padding rows. n truncation, threads0 and
  row-domain bounds also need that later owner plan.
- Further masked Block reads: mine-bench/lib.rs306,443 and main.rs109,269,444.
  GPU imports directly use RTable.buffer at lib272,274,285. These expose a
  cross-crate owner/exclusion boundary, not a mechanical unsafe annotation.
  Existing mine lib502/main545-line files remain untouched in this unit.
- Actual miner uses its own ordinary Metal Buffer/Dispatch paths; no new proof
  for its generic provider aliases or completion behavior is claimed here.

Fresh22-origin source scan (previous Honey closure+soft3/cyb/cyber/kadek), exact
HEADs/commands/matches in external-inventory.json, SHA
c6d79a0840c404d507fc7f3f3822a3866e180037f7e9d8a3733defc682978b7b.
Only Erga matches the exact identifiers/dependency names. The initial case-insensitive
RTable scan also matched “portable”; its raw output is preserved as
external-inventory-initial.json and is superseded by the case-sensitive bounded
identifier scan. Xena is now merged36ca87ef8c0a4e98947d397e4da6f7fb8021ef9c. This is source inventory, not a
whole-organization/rung build. soft3 components.toml at18fd6b22ec45725aa776e37d377c5a75aeeaf553
has no Erga/blake-bench/rtable-bench row; ring/kelvin closure is unavailable, not
fabricated. Component snapshot and bounded metadata preserve this limit.

## Shader and native proof

The four shaders are unchanged primary source. V1 lines51–61/V2 lines47–57 guard
gid<count and use `(ulong)gid*32`. V3 lines59–80/V4 lines72–89 compute uint
idxA=gid*2 and idxB=idxA+1 before count guard, then use ulong byte offsets.
Their absent odd partner reads A and suppresses B output. Exact grid
ceil(count/hashes_per_thread), without whole-group rounding, ensures
V1/2 gid<=count-1 and V3/4 gid<=2^31-1 even at count=u32::MAX: no idx overflow.
An arbitrarily rounded surplus dual-hash lane can instead wrap uint and write a
previous row. Do not infer safety from practical memory capacity. Private variant
must remain matched to compiled pipeline; actual width bounds remain checked.

Existing provider APIs: Block::plan/allocation_size/open/address/size; Gpu wrap,
max_buffer_length/max_threads_per_threadgroup/buffer_with_data; Queue commands;
Commands encoder/submit/wait/status/error; Encoder bind/bind_buffer/push/launch/finish. Encoder::push already sends inline
bytes; use it for the four-byte count without a new buffer/provider API. Apple
setBytes method primary (primary/metal-setbytes.md plus acquisition.json receipt)
confirms copied inline data up to4KB. The four-byte source need not outlive encoding.

Unlike Block, Buffer has no Send/Sync implementation; retained imports therefore
make Batch neither Send nor Sync. No unsafe trait assertion is justified. A
private retained &Gpu ties Batch to an alive device, and dispatch compares native
device identities before binding persistent imports; do not assume every
system-default discovery always returns the same device.

Reuse committed provider native fixture's proof mechanism, not its asserts:
honeycrisp/aruminium/tests/support/import_memory.rs at9cbd4807; fallible private
helper with actual getpagesize, checked extent, Mach short-info ABI and complete
current read/write leaf containment. Validate ulong8 alignment, whole actual
extent and device max before import; zero actual memory before any Rust view.
A private Storage holds Buffer before Block (field drop order); no native handle,
ID, pointer, raw factory, closure lending Buffer, or shared-write escape. Batch
borrows exclude GPU dispatch from CPU views. Private imports remain alive until
owner drop; synchronous commands always finish before that borrow ends.

Primary API/ABI evidence already acquired, pinned and read for Honey/Xena:
`/tmp/kadek-unimem-block-access-primary/REVIEW-V2-SHA256SUMS` SHA
09d22d36516f1f26c7daef7c1c9c386407cf3f19a3bfea361d21e1508f918f40;
SDK26.4.1 short struct48bytes/align4/is_submap32/count12, XNU
f6217f891ac0bb64f3d375211650a4c1ff8ca1ea region-recurse behavior.
`/tmp/xena-gpu-metal-dispatch-primary/` holds official Apple arbitrary-grid,
CPU-before-commit/GPU-after-completion and M1–M5 family/nonuniform support evidence.
Feature PDF SHA9f31df15dd6827545702c5a0845f6e36e1889878cd0e534123bd70211e5c00a8;
MSL PDF SHA41538b30d2f1140a5b2a0c84ce0a9f7b67bf0c707e224cfea0bfe5a44aa26cf5.
Only attributed necessary excerpts enter review; licensed full originals stay
in temporary cache. Virtual compatibility does not prove physical/pinning policy.

## Independent oracle and exclusions

Existing reference uses registry blake2 0.10.6, upstream RustCrypto commit
1f727ce37ff40fa0cce84eb8543a45bdd3ca4a4e. Its abc test discards both values and
asserts nothing; do not count it as known-vector evidence or expand this task to
rewrite that unrelated test. GPU inputs are exactly32 bytes, so empty/abc vectors
do not exercise the owner/kernel path anyway.

Pinned [BoringSSL 32-byte vector](https://boringssl.googlesource.com/boringssl/+/09f7078f953362d5c5afdd224118845327b60fb4/src/crypto/blake2/blake2b256_tests.txt):
input bytes00..1f; expected
cb2f5160fc1f7e05a55ef49d340b48da2e5a78099d53393351cd579dd42503d6.
Source blob8e52cf474275073793a704a2e1509ad131e1847b, SHA
de04a0c8d63df6a46ed7c6c47eaa961bc3ce33daa4b58cf80b3d76f4d931e00b.
Actual curl/base64 acquisition and literal extraction are in primary/vector.json.
Published generation uses Go blake2b.New256(nil), independent of Erga/RustCrypto/
GPU. Preserve its license notice with the test fixture. New small tests compare
all32 output bytes for all four shaders, odd/partial groups, varied inputs and
reuse; literal check prevents trusting only the same runtime CPU code.

No algorithm, chime, miner, RTable, generic Buffer, manifests/dependency versions,
bench loops or physical policy changes. Full-workspace diagnostics may newly
expose the inventoried RTable/mine failures; success is scoped explicitly.
