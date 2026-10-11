# Mine benchmark ownership audit

Stage active; evidence measured. Source repair closes the benchmark's borrowed
table bindings, private initialized output/texture, synchronous completion and
joined worker failures. Current repository gates remain red: a source commit
requires `wip:` and a decision receipt. This audit establishes no release,
provider-wide safety, native Image, physical pinning or performance closure.

First current workspace-test error (`15-workspace-tests`, source below):

```text
thread 'chime::tests::strikes_differ' (172901854) panicked at rs/app/src/chime.rs:132:39:
attempt to multiply with overflow
```

`32-parent-app-test` reproduces this failure and
`chime::tests::renders_are_audible_and_unclipped` at the same location on the
exact parent. Source is unchanged. Workspace formatting and tag semver also
remain red; their full output is retained.

## Frozen inputs

All command labels below name exact argv, environment, working directory,
timestamps, exits and full stdout/stderr in the archive's `execution/logs/`.
[gates.json](gates.json) maps each command to its actual executed source.

| Input | Revision or SHA256 |
|---|---|
| Erga parent | `8dcb33888868d74ddfda3eeeb9bc90e58677b394` |
| Honeycrisp | `9cbd48070a136e24711896c930670fcff8a147fc` |
| Strata | `494df7a0973ae10a6cc1c2022e644d70449a29d6` |
| Final overlay manifest | `6175ec74aac4ccf2493db33b694b02120d4a2ae6a87356be3803cdc8b217b7bf` |
| Executed parent manifest | `d0cc022e9dc432f409d52b4976d81c228b464ab71dacfc4043d33f13441b73ef` |
| Approved PLAN brief | `4275eefbd353aba978a6ede1ae30d7326c4c622ebea0e6bf0aef51be47713d27` |
| Submitted CODE brief | `f0214fe93da40bd84bc3890fba25255a3108119084bc471b82a48a1f7f838d31` |

[source-freeze.json](source-freeze.json) lists every candidate production,
test, fixture and spec file. Archive `source/candidate.diff` reconstructs that
overlay from retained changed-path parent bodies; all parent file hashes and
provider revision manifests are retained. Manifests, locks, shaders, provider,
versions and unrelated app source have no candidate changes. The proposed spec
is byte-identical to the reviewed file; its physical-pages sentence is preserved.

## Observed gates

Counts and exits in this table come from the named command's retained raw output
at the frozen final overlay, except rows explicitly naming the parent or tag.

| Commands | Result |
|---|---|
| `04-final-debug`, `05-release`: scoped `cargo test -p mine-bench -p rtable-bench --locked --offline`, then `--release` | Each passes 24 runtime tests and 17 doctests; 41 total per run. |
| `06-final-clippy`, `07-strict-docs`, `13-owned-fmt` | Strict scoped Clippy, private docs and owned rustfmt pass. |
| `08-nightly-docs` | Separate actual-nightly compiler/rustdoc run passes 17 diagnostic-code doctests. |
| `09-mine-verify4`, `10-rtable-verify4` | Bounded verification passes; mine verify4 covers buffer variants. Texture cases run in the native tests above. |
| `11-invalid-exponent`, `12-invalid-duration` | Expected exit 1 before native setup for exponent 33 and NaN duration. |
| `14-workspace-check`, `16-workspace-build`, `17-workspace-clippy`, `26-candidate-all-deps-clippy` | Whole-workspace check, release build and strict Clippy pass. Build uses `RUSTC_BOOTSTRAP=1`. |
| `21`–`24`, `27` exact-parent counterparts | Exit 101 on eight obsolete mine field accesses. Candidate source repairs those compiler errors. |
| `15-workspace-tests`, `32-parent-app-test` | Exit 101; same two chime overflow failures. Parent whole tests were masked by mine compilation; the targeted app run exposes them. |
| `18-workspace-fmt`, `25-parent-fmt` | Both exit 1. Candidate 138 sections equal parent 154 after excluding only the two changed mine entry files; all remaining sections match byte-for-byte after declared root-path normalization. |
| `28-protocol-miner-tests`, `29-cli-difftest` | Seven protocol/miner tests pass; correct erga-cli binary matches 512 exact GPU/reference nonce results. |
| `30-semver-mine`, `31-semver-rtable` against tag | Exit 100: 12 mine and nine prior RTable major lint classes. No bump performed. |
| `33-semver-rtable-parent` | Incremental RTable API check against actual parent passes 229 checks with 31 skips. |

The semver tag is Erga `v0.30.0` at
`848e17e2522db2f6225ad72d9b1797f8735d26d0`, with historical compatible
Honeycrisp `7b148906263da5ea8aabc5ddb0e2e44ff2288ecc` and the Strata pin above.
The retained tag lock differs only by removing `libc` from unimem's dependency
entry, matching that provider. Historical owners were compile-only inputs.
Generated semver manifests/locks and the exact normalization evidence remain
in `execution/semver-inputs/`, `execution/tag-Cargo.lock` and PLAN's original
lock. `execution/api-inventory.json` separately lists removed public owners,
helpers and raw fields, changed signatures and the new exhaustive enum variant;
semver lint counts alone are not a complete caller migration inventory.

## Independent oracle and review

The preimplementation generator in `oracle/main.go` uses pinned Go and
x/crypto BLAKE2b with scalar indexing and big-integer accumulation, without
Erga/native implementation reuse. Its exact `go run -mod=readonly .` receipt
and locks are retained. `oracle/vectors.json` provides 360 complete batch XORs
and 18 individual full digests; fixture conversion is checked byte-for-byte.
Reference/V6/V7 use height zero; V8 uses a distinct second table at height
`0x07060504`. Literal nonzero texture-row indices and the published BoringSSL
eight-byte KAT establish independent indexing and primitive witnesses. Final
native tests compare every variant; no native setup or mismatch was skipped.

Fresh different-vendor Grok PLAN approval and its original auto-compaction
notices are retained in `reviews/plan-grok.txt`. The preceding Opus quota
diagnostic is retained separately as availability failure, with no verdict.
Both complete CODE briefs are retained: v1 was an unsubmitted draft; v2 is the
exact submitted brief and invocation. Fresh Grok4.7 [CODE approval](code-review-approved.txt), [invocation](code-review-invocation.json)
and [disposition](code-review-response.md) are retained as final outer sidecars.
No four-vendor convergence or uncompacted review is claimed.

## Evidence boundaries

- `01-check` was exploratory before formatting; its source hash survives but
  its exact preformat body snapshot does not. Intermediate scoped runs `02`
  and `03`, and all final runs, have complete overlay snapshots.
- `20-miner-difftest` targeted a library package and exited 101 without a
  binary run. Correct command `29` passes. The initial Go wrapper also failed
  a stale `rows` schema lookup after successful Go execution; its script,
  source, output and error receipt remain in `oracle/initial/`.
- Parent raw command records' common source hash names the candidate overlay
  under comparison. `command-source-map.json` binds their actual parent
  separately. Original `legacy-provenance.json` also has a stale `parent`
  entry naming the tag and a fmt-only purpose; its `legacy` entry is valid.
  `attribution-correction.json`, actual parent bodies, full parent manifest,
  and independent root replay establish the correct parent above. Original
  records were preserved, not rewritten.
- Private completion, counter, panic and partial-spawn injection tests are
  distinguished from actual native GPU/OS fault evidence. Cross-device and
  destruction-order claims remain source reasoning. Nonfinite timing/rate
  output at zero elapsed time is an unmeasured benchmark diagnostic boundary.
- No default CLI loop, mining, network, large benchmark, packaging, complete
  protocol, rung or performance run is claimed. Full SDK/PDF originals and
  build/module caches are omitted; their source identities, acquisition
  receipts and reviewed excerpts remain. Hash retention does not establish
  that a reviewer inspected an omitted original.

## Replay

```sh
nu audit/2026-10-11-mine-owner/verify.nu audit/2026-10-11-mine-owner --source .
```

The Nu verifier checks archive and payload hashes, every raw command stream,
lossless command dictionary expansion, both source snapshots, parent mapping,
all review section bytes, exact fixture conversion (preserving full unsigned
nonce lexemes), patch reconstruction and paired diagnostics. `replay.json`
retains this command's exact result. Native execution and omitted upstream
acquisitions are not rerun by this portable integrity check. Original execution
and independent root verification scripts/receipts are retained in the capsule.

`capsule-inventory.json` lists every losslessly compressed payload.
`SHA256SUMS` covers outer audit artifacts; the verifier also pins the archive.
