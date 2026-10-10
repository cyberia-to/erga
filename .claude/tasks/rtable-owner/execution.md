# RTable owner execution

Stage active; serves Kadek foundation row 6, discovered-from Honeycrisp PR14 and
Erga PR1. Implementation base is Erga `14b40d667b7fa927f62f46423cd8096f01cf704f`,
Honeycrisp `9cbd48070a136e24711896c930670fcff8a147fc`, Strata
`494df7a0973ae10a6cc1c2022e644d70449a29d6`. Original working trees are untouched.
The frozen research/PLAN remain the reviewed proposal. Fresh PLAN APPROVE SHA256
`5bcec21ed87eaa9ddea2b91cf7f14725c6a6f3d3e29c6d4d074a44f6e613c86c` authorized this
unit; final CODE review and root merge decision are separate.

The ten-path implementation has private initialized RTable and Probe owners,
logical-only views/builds, fallible scoped workers, exact probe grids and checked
synchronous completion. No manifest, lock, shader, provider or mine source changed.
No source file exceeds 500 lines. Native gates use exact archived siblings with
an explicit overlay, pinned rustup 1.95.0/LLVM22.1.2, jobs≤2 and private targets.
Nightly error-code doctests use actual nightly-2025-11-26 compiler and rustdoc.

PLAN advisory disposition:

- `AssertUnwindSafe` documents its invariant at the worker scope: owner closed
  before work; all started workers joined before any error; only total success
  restores access. Private tests inject worker panic and spawn rejection after
  one started worker. These are simulated failures, not native OS exhaustion.
- Fresh zero-filled rows have explicit CPU and GPU zero witnesses; only a
  successful build establishes the hash relation. Built literals include nonzero
  one-read checksums. Independent Go row/checksum literals and the published
  eight-byte BoringSSL vector ground the bounded native tests.
- The four baseline RTable diagnostics (three E0133 and one E0596) are discharged.
  Newly exposed mine private-field/missing-field/lifetime/mutability API breaks
  follow from this unit and remain a separate required owner migration. Old chime
  errors can remain masked; no whole-workspace success is implied.
- Future mine import requires a new exact-actual-extent accessor or a narrower
  provider contract. The current logical slice does not expose that extent.
  No speculative raw export was added. Physical/pinning policy remains open.
- `import_memory.rs` intentionally duplicates the reviewed Blake native preflight.
  Erga bench-owner maintenance owns keeping both copies aligned; consolidation
  needs its own reviewed caller/contract scope, with no invented registry row.
- Unused direct `acpu`/`blake2` dependencies are observed; manifests are untouched.
  RowCountOutOfRange documents that zero instead maps to Mem(ZeroSize).

Execution records live initially in `/tmp/erga-rtable-owner-execution-v1/` for root
verification and later audit packaging. Source freeze v1 was followed by an
edition2021 formatting-only correction: the first owned fmt used edition2024;
its failed check, original bytes and manifest are preserved. Source freeze v2
SHA256 `9985b42ba39fba4d78b43ba26c1b33147e71ce9d784f1feb255d6efee7451bde` is final
pending review. No semantic correction was required by the initial native tests.

Actual different-device rejection, native GPU fault injection, native destructor
tracing and real OS spawn failure were not executed. Identity/drop order are
source evidence; completion and partial worker failures have explicit simulated
witnesses through production private paths. No invalid native handles were made.
No benchmark sweep, default CLI, mining/network/powermetrics, huge epoch build or
packaging run is part of this unit. CLI verify retains its original counts and
prints incidental timing; no performance claim or timing-table update follows.
No phase, row 6, Image, provider-wide soundness, PA/pinning, rung or release closure.
