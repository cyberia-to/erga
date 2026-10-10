# Blake batch owner audit

The Blake benchmark now owns initialized input/output allocations in one private
`Batch<'a>`. Exclusive CPU views and synchronous GPU dispatch preserve allocation,
device and completion lifetimes; unsuccessful native completion closes access.
This unit follows Honeycrisp PR14's raw-access repair and serves Kadek foundation
row 6. RTable/mine ownership and the physical-page policy remain open.

## Frozen inputs and review

| input | committed revision / explicit overlay |
|---|---|
| Erga origin/main, tag v0.30.0 | `848e17e2522db2f6225ad72d9b1797f8735d26d0` |
| Honeycrisp origin/main | `9cbd48070a136e24711896c930670fcff8a147fc` |
| Strata origin/main | `494df7a0973ae10a6cc1c2022e644d70449a29d6` |
| eight-path candidate overlay | `3a828fd00f62bb4e61c12e0737b1ff4093e43b5908a39b11dae09638431e7b2e` |

[Source freeze](source-freeze.json) records every changed path and hash, SHA-256
`297e4959c0f4257c7b985ee9aeaa0194677e367a6e0e7ec0748bd187298d2d98`.
No post-freeze source edits. Original working trees were not build inputs or
edited. The sole lock change deletes obsolete `libc` from unimem 0.2.0's normal
dependency list; no package version, checksum or manifest changes.

Fresh independent Claude Opus [PLAN](reviews/plan.txt) and [CODE](reviews/code.txt)
reviews both APPROVE. CODE found no blockers; optional cosmetic changes were
deferred. [Review identities and hashes](reviews/identity.json) bind both verdicts
to their frozen inputs. The root agent owns final acceptance, the `decision`
label, commit, PR and merge. Passing scoped tests does not make the red workspace
or the deliberate source incompatibilities green.

## Execution

[evidence.tar.gz](evidence.tar.gz) preserves 140 payloads: complete receipts,
21 command records, all 42 raw streams, source snapshots, tested intermediate
overlays, original locks, source/archive inventories and review input indexes.
[capsule-inventory.json](capsule-inventory.json) identifies every archived payload.
Archive SHA-256:
`169ebb6357b95fd8c4252b6ff108a6d5224135bb50071a61ccaf08b3427bdb55`.
Archive `README.md` contains the exact command table; `commands.json` and
`logs/<label>.json` retain argv, environment, executable paths, times and hashes.
Twelve commands exited 0 and nine exited nonzero, including the initial corrected
Clippy finding and both parent/candidate formatting diagnostics.

| check | observed result; archive record label |
|---|---|
| final debug / release | each 9 library tests, 1 integration test and 6 doctests pass; `focused-debug-final`, `focused-release` |
| nightly borrow/lifetime/trait witnesses | 6 expected-error-code doctests pass; `nightly-borrow-docs` |
| scoped Clippy, check, strict docs, owned fmt | pass; `scoped-clippy-2`, `scoped-check`, `strict-docs`, `owned-fmt` |
| native Blake CLI | 4 variants × 4096 hashes pass; `cli-verify` |
| bounded direct miner checks | 7 tests and 512-nonce GPU/CPU difftest pass; `miner-tests`, `cli-difftest` |
| workspace check/test/release/Clippy | stop at remaining RTable errors; `workspace-check`, `workspace-test`, `workspace-release-build`, `workspace-clippy`, `workspace-clippy-diagnostic` |
| workspace fmt | parent and candidate red; 160 unchanged sections compare byte-for-byte equal after declared normalization; `fmt-comparison.json` |
| patch semver diagnostic | tool reports 229 checks: 223 pass, 6 fail; 31 skip; `semver-patch` |

Native tests run on Apple M4 Max with pinned rustup 1.95.0, LLVM 22.1.2 and SDK
26.4.1; error-code doctests use actual nightly-2025-11-26 compiler and rustdoc.
Archive `toolchains.json` records executable paths and hashes. Jobs were capped
at 2 with private targets. No benchmark, huge epoch buildbench, mining,
share/network or powermetrics loop ran. Packaging was not run.

The first scoped Clippy command found `manual_is_multiple_of`; the equivalent
arithmetic correction preceded the freeze and final debug/release checks.
Workspace diagnostics now expose three E0133 and one E0596 in RTable. These are
remaining failures induced by Honeycrisp PR14's raw-access boundary, newly
visible after this Blake migration. Further mine-owner errors and the two earlier
chime overflow failures remain masked. Full Erga/rung gates remain red.

Semver uses a compile-only diagnostic parent with historical Honeycrisp
`7b148906263da5ea8aabc5ddb0e2e44ff2288ecc` and the same one-line lock normalization;
no legacy raw-owner runtime executed. Intentional breaks include removing
`ZeroCopyBuf`, replacing `alloc_buffers` with lifetime-bound `alloc_batch`, private
factory fields, exclusive batch dispatch/views and fallible fill/verify APIs.
Archive `README.md` catalogs type, lifetime and result changes beyond the tool's
findings, including its misleading `doc_hidden` name for newly private fields.

## Evidence boundaries

Real tests cover all four shaders, counts 1, 2, 3, 4, 63, 64 and 65, two fills and
reuse, all output bytes, poisoned full-allocation initialization, padding canaries
and pre-submit preservation. The [independent published vector](primary/vector.json)
comes from pinned BoringSSL/Go generation; the existing `known_vector_abc` test is
a false witness and supplies no independent evidence here.

Failed completion uses a simulated status/error matrix against the production
completion helper plus source inspection. Different-device rejection and relative
Buffer-before-Block destruction rely on source evidence; this single-device run
does not measure them. No fabricated handles or invalid allocations were used.
VM-region compatibility does not settle physical contiguity or pinning. Generic
provider soundness, Kadek row 6, performance, packaging and release remain open.
Zeroing both complete backings changes allocation cost; old timings are not
comparable and no performance claim is made.

Full Apple SDK/web/PDF snapshots and full review packs remain in acquisition
caches; [primary provenance](primary/README.md) retains their pins and hashes.

## Replay

Run from the repository root:

```text
nu audit/2026-10-10-blake-batch-owner/verify.nu audit/2026-10-10-blake-batch-owner
```

The wrapper checks the archive hash, extracts into its own temporary directory,
verifies all capsule payloads/records/streams/overlays/locks and removes that
extraction. It replays evidence integrity without rerunning native workloads.
[Recorded replay](replay.json) exited 0. Historical absolute paths inside records
identify actual execution directories. The root agent creates the final audit
checksum manifest after adding its acceptance and merge receipts.
