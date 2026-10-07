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

## the Ergo node is NOT in the estimate above

Every tier above keeps the **Scala Ergo node** (`ergo` 6.0.7, JVM) and talks
to it over REST. A Lithos client cannot avoid a node: it needs
`/mining/candidateWithTxsAndPk`, the `extraIndex` explorer routes
(`/blockchain/box/unspent/byErgoTree` …), `/utxo/getBoxesBinaryProof`, and
the mempool. There is no Rust Ergo node, and never has been a second node
implementation in Ergo's seven years.

What a Rust node would be, measured on the repos at 2026-10-07:

| component | Scala today | Rust today (sigma-rust, 86.7k lines incl. 17k bindings) |
|---|---|---|
| ErgoTree IR + interpreter | sigma-state 6.0.7 | `ergotree-ir` 26k + `ergotree-interpreter` 19k — ErgoTree **V3 supported**, 114 v6 version gates; has `powHit`, `checkPow`, `fromBigEndianBytes`, `UnsignedBigInt`, `getVarFromInput`, `serialize`; **lacks `deserializeTo`, `encodeNbits`/`decodeNbits`** |
| chain types, merkle, AVL+, NiPoPoW | scrypto + ergo-core | `ergo-chain-types` 2.2k, `ergo-merkle-tree` 1k, `ergo_avltree_rust`, `ergo-nipopow` 0.8k — present |
| wallet / tx building / signing | ergo-wallet 2.4k | `ergo-lib` 10.6k — present |
| **p2p** (handshake, sync, modifiers) | network 2.4k + scorex 1.8k + peer 0.5k | `ergo-p2p` **1k lines, roadmap-only stub** |
| **history** (headers, sections, PoPoW bootstrap, storage) | 0.9k + 1.7k + popow 0.7k | none |
| **UTXO state** (AVL+ authenticated digest, bit-exact) | state 1.5k + 0.5k + avldb 1.5k | none |
| **mempool, mining candidate, difficulty (EIP-37), EIP-27 reemission, voting** | mempool 1.2k + mining 1.4k + 0.9k + settings 2.2k | none |
| **extraIndex** (what Lithos reads) | history/extra 2.1k | none |
| REST API | http/api 3.3k + 0.4k | none (client only: `ergo-rest` 1.7k) |
| node wallet + scanning | 3.3k + 1.1k + 0.4k | partial via ergo-lib |

Ergo node main code: **38.8k Scala** (root 26.2k, ergo-core 8.5k,
ergo-wallet 2.4k, avldb 1.5k) + 32k tests, on top of sigma-state. A Rust
node is a port of roughly all 39k lines onto sigma-rust, i.e. **60–90k lines
of Rust, 9–18 engineer-months for one or two people** to reach a syncing,
validating, mining-capable, indexed node — and that is before the risk that
defines the project: one divergence in state-digest or validation rules and
the node forks off mainnet silently. The interpreter being in Rust already is
the one piece of luck; everything consensus-adjacent above it does not exist.

For Lithos specifically, sigma-rust's gaps matter at tier 5 only: the nine
fraud-proof contracts use `deserializeTo[Header]` (six of them) and
`powHit`; the core contracts the client must *sign against* —
Holding_Logic, Evaluation, Payout, Collateral_Mainnet, LIT_Emissions,
MinerDictionary — use nothing beyond `getVarFromInput`, which sigma-rust
has. So **tiers 1–4 are signable from Rust today; tier 5 waits on
`deserializeTo` landing in sigma-rust** (or a Scala sidecar for fraud
proofs).

## economics — where "3–4× more profitable" comes from

All numbers read live on 2026-10-07; sources are the explorer API, the
LithosDex mainnet pool box, the contracts and `EmissionSchedule.scala`.

**The chain.** 507 GH/s (difficulty 60.8T ÷ 120 s). Block subsidy is the
EIP-27 floor, a flat **3 ERG**, since April 2026. Transaction fees average
**0.011 ERG/block** (40-block sample, 7 tx/block). ERG = $0.308.

**One Lithos block, by the contracts.** The lender posts 2.915 ERG and
receives the coinbase: 3 ERG + the block's fees — a 3.09 % spread plus fees,
~3.5 % per cycle. The holding box takes **2.9109 ERG** (97 %), paid to NISP
submitters by score. LIT per activation at epoch 0: **620 LIT** — 480 to the
holding box, 20 (4 %) to the block finder, **140 to founders** (100 team,
25 investors, 15 auditors; 22.6 % of every block's LIT for the first
1,000,000 activations). The public rate decays 500 → 450 → … → 50 LIT
every 250k activations and stops at 2,825,000.

**On ERG alone Lithos pays less than a pool.** 2.91 ERG vs ~2.97 at a 1 %
PPLNS pool — a 2 % cut, plus the miner's own submission costs (one NISP tx
per Lithos block, fee ~0.001 ERG, bond ≥ 0.002 ERG refunded). Block fees go
to the lender, not the miners. MEV/batching/rent go to the pool but are
small: a LithosDex executor fee is 0.003 ERG/order, storage rent collection
ships **off**, and there are 7 transactions per Ergo block.

**All of the multiplier is LIT.** LithosDex ERG:LIT (the only market) holds
**4,274 ERG / 118,641 LIT → 0.036 ERG per LIT** ($0.011; implied FDV 36M ERG
≈ $11M on a pool whose ERG side is **$1.3k deep**). History from the pool
box lineage, 163 transactions:

| date | ERG/LIT | 480 LIT in ERG | per-block value vs 3 ERG |
|---|---|---|---|
| 09-18 (seeded 10 ERG / 10k LIT) | 0.001 | 0.5 | 1.1× |
| 09-21 | 0.0127–0.0151 | 6.1–7.2 | **3.0–3.4×** |
| 10-01 | 0.0164 | 7.9 | 3.6× |
| 10-04 | 0.0226–0.0277 | 10.8–13.3 | 4.6–5.4× |
| 10-05 (mainnet) | 0.0293 | 14.1 | 5.7× |
| 10-07 (today) | **0.036** | **17.3** | **6.7×** |

So "3–4×" is exactly the late-September/early-October spot price applied to
480 LIT — a marketing-period snapshot. At today's spot it is nominally
6–7×; 3× needs LIT ≥ 0.0125 ERG, 4× needs ≥ 0.019 ERG.

**Why the number cannot hold at scale.** Emission is indexed by activations,
and an activation beyond the first 100 needs a *spent* collateral box, i.e.
a Lithos block. Lithos has found **15 mainnet blocks** in its first ~30 h
(founder payouts: 3 on 10-06, 15 on 10-07 → ~12/day), **~1.7 % of network
hashrate**; emission box counter R4 = 114. At that share, LIT minted is
~5,800/day ≈ 210 ERG/day at spot — already ~5 % of the pool's ERG side per
day if sold. If Lithos took the whole network: 720 blocks × 480 = 345,600
LIT/day ≈ 12,400 ERG/day of emissions against a 4,274 ERG pool, while the
ERG subsidy for the same blocks is 2,160. The multiplier is the ratio
(LIT price × 480) / 2.91; its numerator is set by a $1.3k pool and its
denominator is fixed. It compresses toward 1× exactly as fast as miners
adopt it — the standard shape of an emissions-subsidised yield.

**For an M4 Max (60 MH/s = 1.18 × 10⁻⁴ of the network), per day:**

| | pool (1 %) | Lithos, ERG | Lithos, LIT | Lithos total at spot |
|---|---|---|---|---|
| per day | 0.252 ERG | 0.247 ERG | 40.8 LIT = 1.47 ERG | **1.72 ERG ($0.53)** |
| per month | 7.6 ERG ($2.3) | 7.4 ERG | 1,224 LIT ($13.6) | **$16** |

— paid about 83 % of windows at the recommended τ (k = 13), after a
commitment lag (65 blocks to start, 125 to be paid) and a 24-hour rollup
(360 + 360 blocks), with a 60-block/2-hour window that must contain 10 super
shares. No minimum hashrate: τ scales so any rig averages ~13 super shares;
a smaller rig simply earns a smaller score. The honest statement for the
app: "7 ERG a month, plus LIT whose current quote is worth 6× that and
whose market is 4,000 ERG deep."

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
