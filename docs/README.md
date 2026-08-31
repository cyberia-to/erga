# how erga works

erga turns an Apple Silicon Mac into an Ergo miner: press one button, the GPU
searches for Autolykos v2 solutions, a pool credits the ones it accepts, and a
wallet the app generated for you holds the result.

This is the explanation. The [specs](../specs/) say what it must do; the
[README](../README.md) says how to run it.

## the shape of it

```
erga  (cli/ — one binary: window with no arguments, commands with them)
   │
   ├── erga_app        the window (eframe/egui: draws, never mines)
   │      │  spawns `erga mine … --machine` — itself — and reads its stdout
   │      ▼
   └── erga_miner      that second process; all GPU work lives here
   │
   ├── rs/autolykos   protocol-exact reference, chain-verified
   ├── rs/pool        stratum: subscribe · authorize · notify · submit
   ├── rs/wallet      ergo-lib: BIP39 → m/44'/429'/0'/0/0 → P2PK
   └── honeycrisp     zero-copy Metal: one IOSurface-pinned table
```

### why the miner is a separate process

The window holds an OpenGL context (eframe/glow) while the miner drives Metal
through honeycrisp. Two graphics APIs in one process proved fragile enough to
abort the app — silently, with no Rust panic, which is what a native abort
looks like. Split apart, the window only draws; if the miner dies the UI
survives and says so. The cost is a pipe and a line protocol, which is cheap.

The bundle ships **one** binary. The window re-invokes its own executable as
`erga mine … --machine`, so there is no sibling to ship, to lose, or to let
drift out of version with the window.

## what happens when you press the crystal

1. **connect** — stratum `mining.subscribe`, then `mining.authorize` with your
   address. The pool answers with an extranonce prefix that reserves part of
   the nonce space for this connection.
2. **take a job** — `mining.notify` carries the block height, the header
   pre-hash, and the share target.
3. **build the table** — Autolykos v2 hashes over a table of `N` 32-byte
   elements derived from the height. `N` grows with the chain; at height 1.86M
   that is 6.8 GiB, rebuilt **every block**. The GPU builds it in ~11 s.
4. **search** — a Metal kernel scans batches of 8.4M nonces, computing each
   candidate's hit and comparing it to the target.
5. **verify, then submit** — every candidate the GPU finds is re-hashed on the
   CPU reference before it is sent. Nothing invalid ever leaves the machine.
6. **repeat** — until the height changes, when the table is rebuilt.

## the money

A share is not payment; it is evidence of work at the pool's difficulty. The
pool accumulates shares, and when it finds a block it splits the reward among
whoever contributed. Below its payout floor nothing moves, which is why the app
shows progress toward that floor rather than a balance that would sit at zero.

Three different numbers, deliberately:

| number | where it comes from | what it means |
|---|---|---|
| hashrate | the miner, live | what the GPU is doing this second |
| effective | hashes ÷ session seconds | the same work, with table rebuilds counted |
| pool sees | the pool's API | what was actually credited, 24h average |

They disagree, and the disagreement is the point: the third is the only one
that pays.

## the development share

One share in twenty is mined for the project. It is a *separate authorized
session*, not a relabelled submit — a pool binds each connection to the address
that authorized it, so shares cannot be reassigned after the fact. erga
alternates: nineteen for you, one for development, and the epoch table is held
across the switch so it costs a reconnect, never a rebuild.

Change it with `ERGA_DONATION=off`, `ERGA_DONATION=<address>` or
`ERGA_DONATION_EVERY=N`.

## where things live

| path | what |
|---|---|
| `rs/autolykos` | the protocol: `pow_hit`, `gen_element`, `calc_big_n` |
| `rs/pool` | the stratum client |
| `rs/miner` | the engine, the Metal kernels, the headless CLI |
| `rs/wallet` | seed, address, transaction building |
| `rs/app` | the window, as a library |
| `cli/` | the command that opens it, and everything else |
| `rs/blake-bench`, `rs/rtable-bench`, `rs/mine-bench` | the measurements that came first |
| `packaging/` | the `.app`, the `.dmg`, and the icon as code |
