# RTable owner audit

RTable and Probe now retain initialized private native storage, lend only logical
CPU bytes and require exclusive access for builds and synchronous dispatch.
Partial worker failure or unsuccessful GPU completion permanently closes access.
This prerequisite follows Honeycrisp PR14 and Erga PR1 and serves Kadek foundation
row 6. Mine ownership and the physical-page policy remain open.

## Inputs and review

| input | committed revision |
|---|---|
| Erga origin/main | `14b40d667b7fa927f62f46423cd8096f01cf704f` |
| Honeycrisp origin/main | `9cbd48070a136e24711896c930670fcff8a147fc` |
| Strata origin/main | `494df7a0973ae10a6cc1c2022e644d70449a29d6` |

The [ten-path source freeze](source-freeze.json), SHA-256
`9985b42ba39fba4d78b43ba26c1b33147e71ce9d784f1feb255d6efee7451bde`,
identifies the local candidate overlay. No source edits followed that freeze.
No manifest, lock, shader, provider or mine source changed. Original working
trees were neither modified nor used as build inputs.

Fresh independent Claude Opus [PLAN review](reviews/plan.txt) approved the unit.
Final [CODE review](reviews/code.txt) retracts its source findings and approves.
Root [resolved its evidence-attribution concern](reviews/root-response.md) without
source edits. [Root acceptance](root-review.md) retains the red gates and requires
`decision` accounting for the authorized ordinary source merge.
[Review identities](reviews/identity.json) pin the sessions and input hashes.
Passing scoped checks does not make the workspace or source compatibility green.

## Execution

[evidence.tar.gz](evidence.tar.gz) contains 189 manifest-listed payloads: all 29
execution records and 58 raw streams, both ten-path source snapshots, original
locks, source inventories, complete PLAN baseline streams, independent oracle
outputs, review indexes and verification scripts. [Capsule inventory](capsule-inventory.json)
lists every payload. Archive SHA-256:
`d8ac363e27d6fba2dc2f9a186420163a4b5533dac092cdeae63b0efc74a4999c`.

The [gate index](gates.json) names exact commands, exits and stream hashes;
archive `logs/<label>.json` also preserves executable paths, environment and
start/end times. Thirteen commands exited 0 and sixteen exited nonzero.

| check | observed result; archive label |
|---|---|
| focused debug / release | each 9 private tests, 2 integration tests and 10 doctests pass; `focused-debug-final`, `focused-release` |
| actual nightly borrow checks | 10 expected-error-code doctests pass; `nightly-borrow-docs` |
| scoped check, Clippy, strict docs, fmt | pass; `selected-check`, `selected-clippy-final`, `strict-docs`, `owned-fmt-final` |
| bounded native CLI | verify exponent 4 passes; invalid exponent 33 and text each exit 1; `cli-verify`, `cli-invalid-high`, `cli-invalid-text` |
| direct production-miner diagnostics | 7 tests and 512-nonce GPU/CPU difftest pass; `miner-tests`, `cli-difftest` |
| paired workspace check/test/release/Clippy | parent stops at four RTable errors; candidate at eight new mine API errors; `parent-workspace-*`, `workspace-*` |
| workspace fmt | both exit 1; 154 unchanged sections compare byte-exact after declared normalization; `fmt-comparison.json` |
| explicit patch semver | exits 100; 229 checks, 220 pass and 9 major lint classes fail, with 31 skipped; `semver-patch` |

Commands ran on Apple M4 Max with pinned rustup 1.95.0, LLVM 22.1.2 and matching
Cargo/Clippy/rustfmt. Nightly error-code checks used the actual
nightly-2025-11-26 compiler and rustdoc. `toolchains.json` records executable
paths, hashes and versions. Jobs were capped at 2 with private targets.

The first owned fmt check found edition-2024 formatting applied to an edition-2021
crate. Only import ordering/assert wrapping changed; the failed check and v1
bytes remain archived. Final fmt/debug/Clippy passed after correction. Initial
artifact-verifier lock-pattern and capsule-builder errors affected only their
harnesses, not candidate source or Cargo outcomes. No initial Cargo failure was
removed or replaced by a later receipt.

The old RTable's three E0133 and one E0596 are discharged. Candidate workspace
checks now expose `mine-bench` lib errors at `n` accesses 257/307/444, private
`buffer` accesses 272/274/285 and `block` accesses 306/443. These are source breaks
introduced by this migration. Further mine main/lifetime/mutable-receiver errors
and earlier chime errors remain masked. Production-miner checks are separate:
that crate does not depend on RTable, so their success proves no reverse closure.

Semver's compile-only diagnostic parent uses Erga tag `v0.30.0`/`848e17e` and
historical Honeycrisp `7b148906`, with the previously recorded one-line lock
normalization. Neither current lock changed and no legacy raw owner executed.
Intentional API breaks include borrowed device lifetimes, private fields, removal
of native ID/raw accumulator helpers, mutable builds/probes and fallible results.
Archive `README.md` catalogs changes beyond the tool's findings.

## Evidence boundaries

Real native tests cover 1, 3, 65 and 513 rows; two heights; workers 1/2/3; probe
counts 0/1/2/63/64/65/257; repeated builds/probes; poisoned full actual-extent
initialization and unchanged padding. The [integration row comparison](../../rs/rtable-bench/tests/owner.rs#L32)
witnesses row-base arithmetic by independently enumerating logical indices and
comparing every row with the Rust hash reference, including 65 rows/3 workers
(22, 22, 21 rows). Independent Go supplies 16 selected row-function digests and
56 checksum literals; a published BoringSSL eight-byte record cross-checks it.
These literals are separate from the full-row reference comparison. High-index
function vectors do not claim enormous native allocations. Unbuilt zero rows
and nonzero built checksums are distinct.

The [injected worker failures](../../rs/rtable-bench/src/table/tests.rs#L95) witness
joining and terminal access closure; they do not assert row-base values. Worker
panic, spawn rejection after one started worker, and completion status/error
combinations are simulated through private production paths. Real OS spawn failure,
GPU fault injection, different-device rejection and relative native destructor
tracing were not executed. Identity/drop-order conclusions rely on source evidence.
No invalid native handles were constructed.

Erga bench-owner maintenance owns the deliberate duplicated native preflight until
a separately scoped consolidation. Future mine import needs a reviewed exact
actual-extent boundary; a logical slice alone does not provide it. Unused direct
manifest dependencies remain unchanged. The same-physical-pages normative clause
is preserved; VM observations do not settle physical contiguity or pinning.

No benchmark/default CLI, epoch buildbench, mining/network/powermetrics or packaging
run occurred. No performance, provider-wide soundness, Image, row 6, whole-Erga,
rung or release closure is claimed. Full SDK/web/third-party originals remain in
acquisition caches; the capsule records their URLs/revisions/hashes and reuses
[committed native provenance](../2026-10-10-blake-batch-owner/primary/README.md).

## Replay

```text
nu audit/2026-10-10-rtable-owner/verify.nu audit/2026-10-10-rtable-owner
```

The portable Nu wrapper checks the archive, extracts into its own temporary
directory, verifies payloads, both source snapshots, command records/dictionaries,
raw streams, locks and oracle receipts, then removes its extraction. It reruns no
native workload and does not require the historical absolute execution paths.
[Recorded replay](replay.json) exited 0. Root retains its independent capsule check,
completed CODE verdict, acceptance and final audit checksum manifest separately.
