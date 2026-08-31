# Claude Code Instructions

## Git workflow
- **Commit by default.** After completing a change, commit it.
- **Atomic commits.** One logical change per commit.
- **Conventional commits.** `feat:`, `fix:`, `perf:`, `refactor:`, `docs:`, `ui:`, `chore:`.

## What erga is

A one-button ERGO miner for Apple Silicon. The GPU searches Autolykos v2 on
the honeycrisp zero-copy stack; a pool credits the shares; a wallet the app
generated holds the result.

```
rs/app      the window (eframe/egui) — draws, never mines
rs/miner    the engine, the Metal kernels, the headless CLI
rs/pool     the stratum client
rs/wallet   ergo-lib: BIP39 → m/44'/429'/0'/0/0 → P2PK
rs/autolykos  the protocol reference, chain-verified
rs/*-bench  the measurements that came first
```

## Rules that are not negotiable

- **Never submit an unverified share.** Every GPU candidate is re-hashed on the
  CPU reference before it is sent. See `specs/README.md` for all five
  invariants.
- **Release the epoch table before allocating the next.** Holding both doubles
  peak memory and puts a 16 GB machine into swap.
- **The miner runs as its own process.** eframe holds an OpenGL context;
  honeycrisp drives Metal. Both in one process aborts the app.
- **Measure, do not eyeball.** Layout and performance claims in this repo are
  backed by numbers read off the rendered pixels or a stopwatch, because three
  separate "fixes" here were wrong until something was actually measured.
- **A pool is listed only after a real conversation with it** — a job parsed,
  and where possible a share accepted. Published endpoints are not evidence.

## Building

```
git clone https://github.com/cyberia-to/honeycrisp ../honeycrisp
cargo build --release          # needs RUSTC_BOOTSTRAP=1
cargo test --workspace         # 19 tests
nu packaging/bundle.nu         # → .app + .dmg
erga buildbench          # time one epoch-table build
erga difftest            # GPU kernel == CPU reference
```
