# erga — specification

What the application must do, and the invariants it may not break. The
[docs](../docs/) explain how it does it.

## 1. purpose

Mine Autolykos v2 on Apple Silicon and pay the proceeds to a wallet the user
controls, with one visible action and no configuration required.

## 2. correctness invariants

These are not features; violating any of them is a defect.

| # | invariant | enforced by |
|---|---|---|
| C1 | Every submitted share is re-verified against the CPU reference before it leaves the process | `engine::mine_session` re-runs `pow_hit` and compares to the job target |
| C2 | The GPU kernel is byte-exact to the reference | `erga-miner difftest` over 512 nonces; five rows of every built table are checked against `gen_element` |
| C3 | The reference matches the chain | test vector: height 614400, nonce `0x3105` → hit `0x0002fcb1…412a` |
| C4 | The seed never leaves the machine | stored `0600` in the app-support directory; nothing transmits it |
| C5 | Only the address the user chose is credited, except for the documented development share | the address is passed to `mining.authorize`; the donation uses its own session |

## 3. the mining loop

- The table is a function of block height alone. It **must** be rebuilt when
  the height changes and **must not** be reused across heights.
- The previous table is released **before** the next is allocated. Holding both
  doubles peak memory (14 GB against 7 GB) and is the difference between
  running on a 16 GB machine and not.
- The nonce space searched is masked to the pool's extranonce prefix, so two
  connections never duplicate work.
- A dropped connection reconnects with backoff. Only an explicit stop ends the
  session.

## 4. pools

A pool is listed only after this client has held a real conversation with it: a
job parsed, and where possible a share accepted. An endpoint published in
documentation is not evidence — most Ergo pools' published endpoints no longer
resolve at all. See the survey in the [README](../README.md).

Per-pool differences that must stay behind the interface:

- **solo routing** — an address prefix (herominers) or a separate host (2miners)
- **ledger units** — nanoERG for most, plain ERG for k1pool
- **payout floor** — read from the pool's own API where it publishes one

Network difficulty and the ERG price are chain facts, not pool facts, and are
read from a single source regardless of where the user mines.

## 5. resources

| resource | requirement |
|---|---|
| memory | ~7 GB steady, 8.5 GB peak at height 1.86M, growing ~28% a year with `N` |
| GPU | Metal; the table build and the search both saturate it |
| CPU | one core's worth for verification and reporting |
| network | a few KB/s |

8 GB machines cannot run this and must not be told otherwise.

## 6. the interface

- One primary action: the crystal starts and stops mining.
- Colour carries meaning and nothing else: mint is gain, amber is cost, coral
  is failure, blue is a chain fact.
- Every number shown must be traceable to something measured. Projections are
  labelled as such and derive from live difficulty, never from a constant.
- The application never claims to have earned what the pool has not credited.

## 7. out of scope

Sending funds (import the seed into a wallet), running an Ergo node, true solo
mining against your own node, Intel Macs, and any platform that is not macOS on
Apple Silicon.
