# Changelog

## [0.30.0] — 2026-09-01

### Changed

- **Every control moved to one bar at the foot of the window**, the way a
  console program lays its commands along the bottom: solo, max·eco·min, the
  pool, copy address, change address, back up, report a bug — each with a
  keycap showing the key that also does it. The header keeps what it always
  meant to say: the name, the version, the licence, the run's status and its
  batteries.

### Added

- **Hotkeys.** `space` presses the crystal; `s` solo; `m` cycles
  max→eco→min; `p` switches the pool; `c` copies the address; `a` opens the
  address screen; `b` backs up the seed; `r` reports a bug; `esc` leaves the
  seed and address screens. Keys are matched by physical position, so a
  Cyrillic layout mines exactly like a Latin one. Letters never fire while a
  text field has focus, and full-window screens swallow them entirely.
- The pool control is now one pill that cycles — with two verified pools,
  cycling *is* choosing, and the bar keeps a single shape language instead of
  one dropdown among pills.


## [0.29.0] — 2026-08-31

### Added

- **Seamless block edges.** The next block's table is now built *while the
  current one mines*, in the compute the memory-bound scans cannot use (the V7
  diagnostic measured ALU running 10× spare). When the block arrives, the
  tables swap and mining continues — the pause at each edge disappears instead
  of shrinking. If the block arrives early, the build drops its pacing and
  finishes flat out, showing the familiar pulsing battery for the remainder.

  The price is a second table in memory while both exist, so the machine
  decides, fresh at every block: background building starts only when the
  memory *available right now* exceeds the table plus headroom — and the
  headroom follows the intensity setting, because both answer the same
  question. `max` asks +3 GB, `eco` asks +8 GB, `min` never prefetches. On a
  16 GB Mac nothing changes at all.

  The regime is visible: a calm battery in the header (no pulse — nothing is
  waiting) with `next table N%`, and a `next table` row in THE MACHINE that
  reads `ready` when the next edge will cost nothing.

### Changed

- The version caps now read `v0.29.0 · mit`. The window deliberately does not
  volunteer everything — the development share among it — and the licence is
  the honest pointer: every answer is in the code, and the code is yours.

### Removed

- The `to development` row. The count still exists in the store and the log;
  the interface does not advertise it.


## [0.28.0] — 2026-08-31

### Added

- **max · eco · min**, beside solo and the pool. How much of the machine
  mining may take, as a duty cycle: dispatch a batch, then stand aside for a
  proportional rest. Measured on an M4 Max — max 42–47 MH/s, eco 9.1, min 3.6,
  which is 21% and 8% of full tilt against the 25% and 10% asked for; the gap
  is fixed per-dispatch cost, and the hashrate shown is the real one because
  the hashes really are not being done.

  The setting is a four-byte file the miner re-reads twice a second, so moving
  the control is felt at once instead of costing an epoch-table rebuild.
  `erg mine --intensity max|eco|min` from the terminal.

### Changed

- **The scan stopped allocating three GPU buffers per dispatch.** They were
  asked of Metal eight times a second and thrown away; only their contents
  change, not their storage. Over five-minute runs the mean rate went from
  38.0 MH/s to 49.4 — about 29%, though thermal drift makes the exact figure
  soft.


## [0.27.0] — 2026-08-31

### Added

- **A battery for the table build.** Pressing the crystal is followed by a
  wait while the epoch table is built, and until now that wait was a black box.
  Beside `building table…` there is now a battery filling up and a percentage,
  and both breathe — a pulse soft enough not to nag and quick enough to say
  *working, not stuck*. Measured at 1.48× between trough and peak on a 1.8 s
  cycle.

  The progress is real, not a stopwatch pretending. The build is dispatched in
  eight pieces and each one reports as it lands, through the miner's STAT line
  to the window. `erg mine` prints it too.

### Changed

- The build kernel takes a row offset, so the build can be dispatched in
  pieces. Swept on an idle M4 Max (227M rows, 6.77 GiB): 1 piece 13.20 s,
  4 pieces 11.67 s, 8 pieces 13.37 s, 16 pieces 14.89 s, 64 pieces 21.49 s.
  Run-to-run spread is ~1.5 s, so the eight pieces the meter needs are free.
  `ERGA_BUILD_PIECES` sweeps it.


## [0.26.0] — 2026-08-31

### Added

- **A payout address you choose.** `change address` in the action bar takes any
  mainnet Ergo address, so someone who already mines can be paid where they
  already get paid. It is checked as it is typed — ergo-lib parses it, so the
  encoding's own checksum catches a single changed character, and a testnet or
  script address is refused with the reason. The window, the tray, the ledger
  query and `erg status` all read the same one address, and mining restarts on
  a change, because a pool credits whoever authorized the session.

### Changed

- **The sounds are modelled as bodies, not as tones.** Adding sine partials
  and fading them out is why an app sounds like a tone generator; a real object
  is a body that rings when something hits it. Both sounds are now a burst of
  contact noise poured through resonators — a fingertip on a wooden bar, and a
  drop of water into a bowl that rings from the splash. Each is rendered as
  three slightly different strikes, played in rotation, because nothing in
  nature repeats exactly and an identical sample is what reads as *machine*.

### Fixed

- **A click that only woke the window could press a button.** The guard against
  this started life assuming the window already had focus, which left the
  launch frames unguarded: a pointer resting where a button appears pressed it
  before the window had finished opening. Caught while testing this release —
  it silently reset a payout address and started mining.


## [0.25.0] — 2026-08-31

### Fixed

- **The command was linked where no shell would look.** `place_link` tried a
  fixed list — `/usr/local/bin`, then `~/.local/bin` — and on a Mac where the
  first needs `sudo` and the second is on nobody's PATH, it reported itself
  linked and then was not a command. It now prefers a directory that is
  actually on your `$PATH`, choosing your own before a package manager's
  prefix, and says the real directory to add when it has to fall back.

### Added

- **`erg`**, the same command under a shorter name, linked beside `erga`.
  Headless mining is `erg mine`.


## [0.24.0] — 2026-08-31

A positional release: no new surface, but the promises the README makes are
now the promises the code keeps.

### Fixed

- **The development share was never taken in short sessions.** The cadence
  counter lived only in the miner process, which is spawned afresh every time
  mining starts — so anyone mining in bursts of fewer than 20 shares donated
  nothing, and the stated 1-in-20 was quietly 1-in-never. Locally: 38 accepted
  shares, 0 donated. The count now survives restarts and is checkpointed on
  every share, so a quit or a kill costs nothing.
- **The bug report's environment table rendered as a code block.** Source
  indentation had leaked into the issue template, and four leading spaces are
  all Markdown needs. Two tests now hold the template flat.

### Added

- The development count is shown in the app, under **to development** beside
  your own all-time shares. It was previously visible only inside a filed bug
  report — which is not what "never hidden" means.

### Changed

- The README is rewritten: what you get, what it costs in memory, what it
  earns, and why the pool list is short — with the live and effective
  hashrates separated, since only the second is the one a pool agrees with.
- The panels take view structs instead of ten positional arguments, so a call
  site says what it passes. Zero clippy findings across the workspace; 14
  tests.


## [0.23.0] — 2026-08-31

### Changed

- The seed screen's way out sits at the foot of the window, at exactly the
  height of the action bar it replaces — measured at 0 px apart. A button that
  moves when the screen changes makes you hunt for it with the mouse.


## [0.22.0] — 2026-08-31

### Changed

- The menu bar wears erga's own mark rather than a hexagon glyph. It is the
  same artwork the Dock and the window use — box-averaged down to 22 points
  and keyed out of its near-black plate, which would otherwise read as a black
  tile up there.

### Fixed

- A doubled rule in the payout panel. The first belonged to the balance block
  that moved to the head of the window in v0.13.0, and had been drawing a
  line under nothing ever since.


## [0.21.0] — 2026-08-31

### Changed

- Balance, crystal and address read as one column with equal air above and
  below the crystal. Equal constants looked unequal — the balance block
  carries empty space under its digits — so the gap above is smaller by
  exactly what that block adds. Looking equal is the only kind that counts.
- The header icon is a third larger.
- The effective rate is smoothed over about fifteen seconds. The raw figure
  steps every time a table rebuild stalls the numerator while the clock keeps
  running, and a number that says "this is your real pace" should drift rather
  than twitch.


## [0.20.0] — 2026-08-31

### Changed

- The command moves to `cli/` at the root, where the cyberia layout puts it,
  and `rs/*` are libraries. `erga` with no arguments opens the window; with
  arguments it does that instead.
- **The bundle ships one binary instead of two.** The window now spawns *its
  own executable* as `erga mine … --machine`, so there is no sibling to ship,
  to go missing, or to drift out of version with the window. That also
  retires the path search that hunted for it.
- `difftest` and `buildbench` become subcommands of `erga` rather than a
  second binary's arguments, and `erga-pool`'s probe binary is gone.


## [0.19.0] — 2026-08-31

### Fixed

- The seed screen opening unasked. A click that only brings the window forward
  was landing on whatever sat under the pointer, and the action bar is at the
  foot of the window. The guard for this was written once before and lost to a
  `git checkout` during the module split, along with the seed screen's timer —
  both are back, and this time verified by reproducing the bug first.

### Changed

- The run's status moved into the header, beside the version, where the other
  facts about this run already live.
- The crystal button is one function instead of two copies, and the narrow
  layout uses the same readout as the wide one. `hero_rate` is gone with them.

### Measured, and settled

- `gpu.buffer()` is already `MTLResourceStorageModeShared`: the epoch table is
  zero-copy today, CPU and GPU reading the same pages. There is no copy to
  remove.
- The build runs at **1.45 G Blake2b compressions/s**, against `blake-bench`'s
  peak of 0.893 G *hashes*/s — so the kernel is already **1.6× more efficient
  per compression** than the standalone benchmark, by amortising setup across
  65 blocks and keeping state in registers. (An earlier note here quoted
  1.6 GH/s for that peak from memory; the bench says 893 MH/s.)
- Hybrid CPU+GPU build: the CPU adds 1.29 M elem/s to the GPU's 22.3 M, taking
  the build 10.2 s → 9.6 s. The build is 8.5% of a block period, so that is
  **+0.46% mining time** for sixteen cores at full tilt — against a project
  whose headline is MH/W.
- CPU prebuilding the next table while the GPU mines: the CPU needs **176 s**
  per table against a 120 s block time. It cannot keep up even in principle.


## [0.18.0] — 2026-08-31

### Changed

- The sounds are modelled rather than synthesised. A struck wooden bar for the
  press — inharmonic partials at the free-bar ratios, over a filtered noise
  transient, which is why wood sounds like wood — and a drop of water for a
  share, its pitch rising as the cavity behind it closes. That rise is what
  makes a droplet recognisable, and it needs the *integral* of the frequency;
  multiplying frequency by time gives a slide whistle.
- The start hint breathes more slowly and never dims below two thirds. A hint
  that swings to nearly-off reads as a warning light, not an invitation.

### Removed

- "YOUR BALANCE ON CHAIN" and "YOUR WALLET". A large number in ERG at the head
  of a miner's window, and an Ergo address under a mining crystal, do not need
  to be told what they are.


## [0.17.0] — 2026-08-31

### Added

- A menu-bar item. Mining, it carries how close the payout is; idle, its menu
  offers to start. erga is a thing you start and stop looking at, so the one
  number worth carrying out of the window is that one.
- A terminal face worth reading: `erga help`, `erga status` (what the pool owes
  you, without opening anything), `erga link`.
- Installing the app puts `erga` on your PATH by itself, from inside the
  bundle, without a prompt or an admin password.

### Changed

- The share sound is two soft notes a fourth apart, struck like wood. The
  first version was a pair of chirps near 3 kHz — the register that becomes a
  whistle you resent by the fiftieth share.


## [0.16.0] — 2026-08-31

### Added

- Two synthesised sounds, no samples and no licence: a soft wooden pluck when
  the crystal is pressed, and a two-note bird when a share is accepted —
  something small arriving, which is what a share is.
- The crystal dips and springs back over ~220 ms when pressed.

### Fixed

- The seed screen's ten-second timer, which a `git checkout` during the module
  split had quietly restored.
- Rows of buttons are centred from a **measured** width rather than a guessed
  constant. The action bar now measures 0 px off centre.
- Starting to mine no longer jerks the window upward: the start hint's row is
  reserved whether or not it is drawn.


## [0.15.0] — 2026-08-31

### Changed

- The seed screen no longer closes itself. It is being copied onto paper, and
  paper is slow.
- Crates live under `rs/`, and the window is split by role: `theme`, `widgets`,
  `panels`, `purse`. `main.rs` went from 1381 lines to 634.

### Fixed

- A click that only brings the window forward no longer presses whatever sits
  under the pointer — which, with the action bar at the foot of the window, is
  the most likely explanation for a seed screen appearing unasked.

### Measured and rejected

- Threadgroup sweep on the epoch build after the pad optimisation: 32 and 64
  tie at 10.3 s, larger is worse. 64 was already right.
- Specialising the pad's byte-swap for its known-small index: ~1%. The Metal
  compiler was already folding it.

## [0.14.0] — 2026-08-31

- All-time totals move to the payout panel, where earnings history belongs.
- "press the crystal to begin" moves to the top of the window and breathes.
- The three actions become an action bar at the foot of the window.
- The balance is no longer dimmed at zero — dimming it made the hashrate the
  brightest thing on screen.

## [0.13.0] — 2026-08-31

- The balance opens the window; the crystal is centred on both axes; the rate
  moved inside it. Two centring errors found by measuring pixels: a 48 px
  slack term landing on one side, and a height read before the header drew.

## [0.12.0] — 2026-08-30

- Two panels of one size, measured: 721×874 both.
- The repository takes the cyberia layout; `docs/` and `specs/` written.

## [0.11.0] — 2026-08-30

- Meters separate what erga costs from what the machine was doing anyway.
- Colour becomes a language: mint gain, amber cost, coral failure, blue chain.
- `report a bug` opens a prefilled GitHub issue.

## [0.10.0] — 2026-08-30

- Pools verified by conversation, not by documentation: of the eight in Ergo's
  official list, two answered.

## [0.9.0] — 2026-08-30

### Performance

- The epoch table's 8 KB pad is computed rather than read: 1.86 TB of loads
  removed from a full build. **Build 20 s → 11 s**; time spent mining rose
  from ~50% to ~88% of the clock.

## [0.8.1] — 2026-08-30

- The previous epoch's table is released before the next is built.
  **Peak memory 14 GB → 7.1 GB**, which is the difference between running on a
  16 GB Mac and not.

## [0.4.0] — 2026-08-29

- The miner became its own process. eframe's OpenGL context and honeycrisp's
  Metal work in one process aborted the app silently.

## [0.1.0] — 2026-08-29

- First release: one button, Autolykos v2 on the honeycrisp zero-copy kernel,
  shares accepted by a pool.
