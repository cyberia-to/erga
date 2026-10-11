# Mine owner execution

2026-10-11: resumed frozen PLAN-only task at origin/main
8dcb33888868d74ddfda3eeeb9bc90e58677b394. Independent replay verified
162 inputs, 82 exact Git bodies and 871 archive files; origins unchanged.
PLAN pack SHA256 4275eefbd353aba978a6ede1ae30d7326c4c622ebea0e6bf0aef51be47713d27.
Root's first fresh Opus attempt exited 1 with monthly spend-limit diagnostic;
this is reviewer availability failure, not a review verdict. Root's fresh
Grok 4.7 review returned APPROVE with no blockers; its raw auto-compaction
notice is retained. No four-vendor convergence claim. Root authorized the
approved source/spec implementation and owns final CODE review and merge.

Accepted advice: document exact bind map; verify4 covers buffer variants,
texture cases use 8192/16384 rows; independent oracle reference/V6/V7 uses
height0, V8 second table height0x07060504; prior Nu schema error remains labeled
harness error; stop for any unchanged shader mismatch. Physical-pages sentence
stays literal. All implementation source changes are in this owned worktree.

## Frozen implementation and gates

Final production/test/spec snapshot:
`6175ec74aac4ccf2493db33b694b02120d4a2ae6a87356be3803cdc8b217b7bf`
(current-source.json, 20 files) against the base above. Exact origin archives
and reviewed overlay live at `/tmp/erga-mine-owner-execution/source/`;
unchanged provider/source paths are independently hash-verified. Changed Rust
files are at most 272 lines. No manifests, locks, shader algorithms, provider,
version or unrelated app source changes. Spec equals the frozen proposed file.

Commands use exact rustup1.95.0 RUSTC/RUSTDOC and two jobs; actual nightly
2025-11-26 compiler and rustdoc are used for diagnostic-code doctests. Complete
argv/environment/start/end/exit/stdout/stderr hashes are in execution/logs.
No default CLI, mining, network, large benchmark or performance sweep ran.

- `cargo test -p mine-bench -p rtable-bench --locked --offline`, then `--release`:
  41 passes each, including 17 compile-fail doctests; no ignored native setup.
  Final labels04-final-debug/05-release. Preimplementation Go oracle provides
  360 full XOR cases and18 individual digests; each native variant, wrapping
  nonce base and copied texture geometry matches. Scoped worker fault/panic/
  partial spawn/counter tests and joined finite CPU/GPU shared reads pass.
- Focused all-target check, Clippy with `--no-deps -- -D warnings`, strict private
  no-deps docs and owned rustfmt pass. Actual-nightly doc run passes17 checks.
- `cargo run -p mine-bench -- ... verify 4` and RTable counterpart pass. Mine
  verify4 covers selected buffer variants; V9 uses native8192/16384-row tests.
  Invalid exponent33 and NaN duration return1 before native setup as expected.
- Whole-workspace all-target check, release build with RUSTC_BOOTSTRAP=1,
  and strict Clippy both with/without --no-deps now pass. Exact-parent counterparts
  fail compilation in the existing mine consumer. All8 source errors are repaired.
- Whole-workspace tests reach old app chime failures: strikes_differ and
  renders_are_audible_and_unclipped panic at chime.rs132:39, multiply overflow.
  Exact-parent targeted `cargo test -p erga-app --lib` reproduces both failures;
  unchanged app source and exact test outcomes verified. Whole tests remain red.
- Whole fmt remains red: candidate138 sections equal parent154 after excluding
  only the two modified mine entry files; remaining138 sections are byte-identical
  after root-path normalization. No unrelated formatting was applied.
- Bounded autolykos/erga-miner tests pass7; correct erga-cli `difftest` passes512
  exact nonce results. This is not full protocol/performance/rung closure.
- Semver against v0.30.0 using exact historical compatible provider and disclosed
  lock-only removal of unimem's libc dependency: mine-bench12 major lint classes,
  RTable9 prior major classes. Both exit100; no version bump made. RTable against
  the actual parent is green229 checks/31 skips. Source API-break inventory is
  separate because tool diagnostics do not enumerate every caller effect.

The initial check01 was exploratory before formatting; its source hash is retained
but the exact preformat body snapshot was not kept. All final gates have full
source snapshots. Intermediate scoped debug02/Clippy03 retain their earlier
complete snapshot. This limits replay of check01 only, not final evidence.
A mistargeted difftest command20 used the library-only erga-miner package and
returned101 without running a binary. Correct command29 uses erga-cli/erga and
passes; both raw outcomes remain. Parent raw records' common source_freeze hash
identifies the candidate comparison overlay; parent-source.json independently
identifies all137 actual unchanged parent files. No candidate hash is represented
as the executed parent source.

Current reds require a decision/wip source change, not a release. Root retains
final CODE review, audit capsule, commit and merge responsibility. Canonical
physical-pages sentence remains unchanged; actual allocation observation still
does not claim universal pinning or full native Image ownership closure.

Fresh Grok4.7 CODE returned APPROVE with no blockers. Root independently checked
all33 records/66 streams, source/parent/legacy identities, literal oracle fixtures
and paired diagnostics. The final20-file source freeze stayed unchanged. Final
raw review, invocation, limitations and disposition are outer sidecars in
audit/2026-10-11-mine-owner/. Required reds remain wip/decision; the owner's
standing ordinary-source merge instruction does not authorize a release.
