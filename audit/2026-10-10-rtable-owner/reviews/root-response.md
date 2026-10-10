# E1 response: no missing witness; clarify attribution

Reviewed the complete raw CODE verdict, including its final “Verdict correction:
APPROVE” and its retained evidence concern E1. All proposed source blockers were
withdrawn by the reviewer. The alleged audit claim is absent:

- Frozen `/tmp/erga-rtable-owner-execution-v1/README.md:36` says the injected worker
  panic/spawn rejection “verify joining and terminal access closure” (lines 36–39).
- `.claude/tasks/rtable-owner/execution.md:20` places them under AssertUnwindSafe,
  scope joining and failure closure (lines 20–23); no row-base claim appears.
- The original outer audit `README.md:88` called these simulated failure paths;
  it did not attribute start-offset arithmetic to them.
- The quoted “exercise that path” occurs only in frozen CODE brief v2 line 5,
  immediately after the explanation of scope joining and failure recovery.
  It refers to that control path, not row-base arithmetic.

Existing executed arithmetic witness:

- `rs/rtable-bench/src/table.rs:79` computes the constant chunk stride; line 86
  enumerates `chunks_mut(chunk_bytes)`, and line 90 passes the stride-derived row
  base. Uniform chunk starts have that stride even when the last chunk is short.
- `rs/rtable-bench/tests/owner.rs:32` selects rows1/3/65/513; lines39–46 run both
  heights and workers1/2/3, then independently enumerate every logical row index
  and compare its bytes with `blake2b256(LE(index)||LE(height))`. In particular,
  rows65/workers3 yields chunks22/22/21 and row bases0/22/44. A wrong noninitial
  base changes these expected row inputs; the test observes all resulting bytes.
- `src/table/tests.rs:95` and `:117` assert completed-worker counts, expected
  error kinds and terminal closed access. They intentionally do not assert every
  callback start value, and no such assertion was claimed.

Precision correction to the review itself: full-row equality uses the existing
Rust hash reference, with independently constructed global row indices. It does
not compare every row directly against Go literals. `owner.rs:10` cross-checks
16 selected digest literals; `owner.rs:48` checks56 independent checksum literals.
The original audit distinguished these, and the clarification states it explicitly.

Applied only:

1. Outer `audit/2026-10-10-rtable-owner/README.md` evidence paragraph: explicitly
   attributes row-base arithmetic to the integration comparison, worker failure
   joining/closure to the injected callbacks, and distinguishes reference bytes
   from independent Go literals.
2. New `.claude/tasks/rtable-owner/post-review.md`: the same concise disposition,
   preserving the exact frozen execution.md input at its original path.

No source/spec/test changes; no gate reruns. No missing implementation or test
witness was fabricated to satisfy an unsubstantiated claim. CODE packs, archived
raw verdict/input bytes, source freeze and evidence.tar.gz remain unchanged.
Root owns the raw verdict, final acceptance, checksum manifest and merge.
