# Changelog

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
