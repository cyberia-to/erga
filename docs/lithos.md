# Lithos — what it is, and what a pure-Rust integration costs

Researched 2026-10-07 against the live sources: `lithos.work` (a stub landing
page: logo, TESTNET/LIVE badges, four links), the Ergo blog and docs, and the
three repos cloned at the commit of that day —
[Lithos-Client](https://github.com/Lithos-Protocol/Lithos-Client) `v1.0.2`,
[Lithos-SDK](https://github.com/Lithos-Protocol/Lithos-SDK), and the
one-file [LitePaper](https://github.com/Lithos-Protocol/LitePaper).

## what Lithos is

**Not a new chain and not a new PoW.** Lithos is a decentralised mining pool
*for Ergo*, built as an optimistic rollup in ErgoScript contracts. Autolykos v2
is untouched; the miner hashes exactly what it hashes today.

The thing a miner runs is the **Lithos Client** — a Scala/Play application
(≈44k lines of Scala + 1.3k Java, 53k lines of tests, 34 ErgoScript contracts
≈5.4k lines, CC0) that sits beside a *fully synced, indexed (`extraIndex=true`),
mining-enabled* Ergo node and:

1. serves standard Autolykos stratum on `:4444` to any miner (SOAT, Rigel —
   "the `ERG_WALLET` and worker name have no effect on Lithos");
2. builds its **own block candidate** through the node's
   `/mining/candidateWithTxsAndPk`, inserting a *collateral transaction*
   (the genesis of a rollup, funded by a lender's collateral box), plus
   rollup/emission/storage-rent/ErgoDEX/LithosDex transactions it batches
   itself for fee revenue;
3. grades every share: `fH < b` is a block; `fH ≤ tau/10000` is a
   **super share**, where `tau` is the difficulty the miner *committed
   on-chain*;
4. packs a super share as `[N:4][header][txSize:2][numLevels:1][collateral tx
   + collateral box][merkle levels ×33][pk y-coord:32]` and stores it in a
   local LevelDB; **10 super shares inside a 60-block window (2 h) before a
   block** form a **NISP** (`[score:8][10 × SuperShare]`, 8–30 KB);
5. submits NISPs to the rollup with a bond (`max(score/25, 0.002 ERG)`),
   evaluates *other miners'* NISPs off-chain and files any of **nine fraud
   proofs** (NotInWindow, NonMatchingCommitment, MalformedGE, InvalidDiff,
   MalformedGenesis, NonUniqueHeaders, InvalidFormat, TransactionNotIncluded,
   IncorrectN) to slash bad submissions;
6. syncs the whole rollup state machine (LFSM) from chain — holding period 360
   blocks + eval period 360 blocks, miner dictionary (an AVL+ authenticated
   dictionary), difficulty commitments (take effect 65 blocks after, pay 125
   blocks after, changes take up to ~28 h), payouts, LIT token emissions,
   collateral queue/market — and claims its own payouts.

So the "protocol" has three layers: a stratum dialect (trivial), an on-chain
contract set (fixed; deployed and hash-pinned — `ContractHashesSpec` warns
that moving a hash is a hardfork), and a large *off-chain client* whose job is
candidate assembly, NISP construction, rollup sync, fraud proving and
transaction building. The client is the integration surface.

### status, as of 2026-10-07

- Mainnet `v1.0.0` released **2026-10-05**, `v1.0.2` on 10-07 — two days old.
  Testnet ran 2025-11-17 → 2026-09-28 (28 releases). Needs Ergo node ≥ 6.0.7.
- Eight stars, four forks, one external contributor. Open issue #10 reports a
  Payout contract path that lets anyone re-id a payout box (griefing, no
  theft); #8 a collateral parser crash; #12 launch-day operator notes.
- The LitePaper is one README; the whitepaper is promised. The only spec is
  the Scala source and the `.ergo` contracts.
- LIT token: mainnet id `c1980d82…6de7`, 1 billion minted; emitted to miners
  per block via `LIT_Emissions.ergo`; three founder addresses are hard-coded.
- Launcher (Windows, Linux) — **no macOS packaging** exists anywhere in the
  org. SOAT miner is CUDA/Vulkan, Linux/Windows only.

## erga against Lithos today

The stratum the client speaks is the stratum4ergo / ErgoStratumServer dialect
erga already captured from herominers (`rs/autolykos/STRATUM.md`): the same
`mining.notify` nine-element params with `b` as a decimal bigint at `[6]`,
`extraNonce1Size = 2`, 5-param submit. Two differences:

| Lithos behaviour | erga today | cost |
|---|---|---|
| `mining.set_difficulty` is always `1`; the real target is `b` | engine ignores `set_difficulty`, uses `b` (`engine.rs:475,585`) | none |
| `mining.set_extranonce` on duplicate-nonce or rotation (`rotateExtraNonceInterval`, default off) | not parsed (`stratum.rs:parse_line`) | ~30 lines |
| address/worker ignored, payout goes to the *node wallet* | erga authorises `<address>.erga` | none; the wallet erga generated is simply not where ERG lands |

**Pointing erga at `127.0.0.1:4444` should work with zero code changes**, and
is worth a live test. But every Mac user would also need a JVM Ergo node
(~30 GB disk, indexed) and the JVM Lithos client (6–8 GB RAM) — on a 16 GB
Mac already holding a 7 GB epoch table that does not fit. That is the actual
reason a Rust client is interesting, not the stratum.

## what a pure-Rust Lithos client would take

Scope assumed: replace the Scala client, keep the Scala Ergo node (there is no
Rust Ergo node; the client talks to it over REST). Contracts stay as deployed
— a Rust client **must not** recompile ErgoScript; it uses the deployed
ErgoTrees (`sigma-rust` has no complete ErgoScript compiler, and the hashes
are pinned on-chain anyway). ErgoTree bytes are exported once from the Scala
build and pinned by hash, as `ContractHashesSpec` already does.

`ergo-lib 0.28` (already a dependency of `rs/wallet`) gives: header and
transaction (de)serialisation, ErgoTree + interpreter, box/register types,
tx builder and signing, `ergo-merkle-tree`, `ergo_avltree_rust`, Autolykos
pow checks in `ergo-nipopow`. That covers the primitives; nothing in the
rollup layer exists in Rust.

| tier | what | Scala today | Rust estimate | notes |
|---|---|---|---|---|
| **0. mine to someone's Lithos client** | erga → `:4444`, add `set_extranonce` | — | **~1 day** | zero protocol work; the node+client stay JVM |
| **1. share grader + NISP builder** | stratum *server*, candidate from node, `fH` vs `tau/10000`, SuperShare/NISP serialisation, merkle levels from `txProofs`, LevelDB-equivalent store, 10-in-60 window | ≈3.9k (mining + nisp + stratum lib) | **2–3 weeks** | byte-exact serialisation must be proven against Scala fixtures; the fraud-proof contracts parse these bytes on-chain |
| **2. candidate assembly with collateral** | pick a collateral box, build the genesis/collateral tx, pin it into `candidateWithTxsAndPk`, rebuild on chain advance, emission tx | ≈2.1k (candidate + emissions) | **2–3 weeks** | needs ErgoTree templates for `Collateral_Mainnet`, `Emission_*`, `Collateral_Enforcer`; tx cost sizing (`TransactionCostSizingSpec`) |
| **3. rollup sync (LFSM)** | reduce every block from `startHeight`, track rollup/holding/eval/payout boxes, miner dictionary (AVL+), commitments, snapshots, reorgs | ≈6.4k (synchronization + persistence) | **4–6 weeks** | the hardest part; the only spec is `SyncHandler.scala` (1.6k lines) and `BlockReducer.scala` (1k) |
| **4. submit + claim** | NISP submission with bond, difficulty commitment, payout claim, storage rent | ≈3.4k (rollups + rent) | **2–3 weeks** | AVL+ lookup/insert/remove proofs as context vars — `ergo_avltree_rust` exists but is thinly used |
| **5. fraud prover** | off-chain evaluation of others' NISPs, nine proof transactions | ≈1.4k (evaluation + fp contracts) | **2–3 weeks** | optional for *earning*, mandatory for the protocol's security model |
| **6. revenue batching** | ErgoDEX + LithosDex order batching, LithosDex AMM | ≈6.5k | **skip** | fee revenue, not protocol; LithosDex is its own product |
| **7. web panel / stats / API** | Play controllers, OpenAPI, Docusaurus | ≈9k | **skip / fold into erga's UI** | |

**Minimum viable Rust client that earns = tiers 1–4: 10–15 engineer-weeks**,
one person, *if* nothing in the contracts moves. Adding the fraud prover
(tier 5) makes it a full peer: **13–18 weeks**. Everything else is deliberately
cut. For scale: erga itself is 8.1k lines and took ~3 days to v0.30 because
Autolykos was a 20-line spec with a chain test vector; Lithos has no spec,
only 44k lines of Scala that are themselves two days into mainnet.

### risks that set the error bars

- **No specification.** Every byte format and state transition must be read
  out of Scala. The client changed every few days through September; a
  contract change would be a hardfork, but the off-chain *rules* can drift
  without one.
- **Hash-pinned contracts** we did not write, with a known open hole (#10).
  A Rust client inherits the protocol's risk; it cannot fix it.
- **Node coupling.** The client needs the node's wallet (`storagePath`,
  `pass`) to sign — the node *is* the key holder. A Rust client could sign
  with its own `ergo-lib` wallet instead (erga already has one), which is
  cleaner but diverges from the reference and must still fund bonds and fees
  (≈0.05 ERG floor).
- **Memory on a Mac.** Indexed Ergo node (JVM, several GB) + 7 GB epoch table.
  The Rust client removes the JVM *client* only; the node stays. 16 GB Macs
  are out regardless; 32 GB+ is the realistic floor for solo-on-Lithos.
- **Economics unchanged.** Lithos does not change hashrate or reward; it
  removes the pool's fee and custody and adds LIT emissions. The absolute
  ERG/month for an M4 Max stays "a couple of dollars".

## recommendation

1. **Tier 0 now** (one afternoon): a Linux box with node + Scala client,
   erga from a Mac over LAN to `:4444`, confirm a super share lands and
   `/mining/bestNISP` fills. That is the only cheap, real signal.
2. **Do not start a Rust client before Lithos has a written spec or a stable
   month on mainnet.** Re-evaluate at the first contract-stable milestone;
   if it comes, tiers 1–4 are a ~3-month project that fits the honeycrisp
   doctrine (one process, zero-copy table, node over REST) and would be the
   first non-JVM Lithos client.
3. If the goal is "on our terms" rather than Lithos per se: the piece worth
   owning is the NISP idea (10 rare shares in a window as a compressed proof
   of work) — it maps directly onto settlement sampling in `tru/specs`, and
   is a one-page spec, not a 44k-line port.
