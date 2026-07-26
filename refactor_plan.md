# Stellar Legacy — File Size Refactor

`CODE_STANDARDS.md` sets a **800-line hard limit** per `.rs` file (soft target 200–400,
soft limit 600). Eleven files are over it. This plan tracks the split, one file per pass.

Rules that constrain every split here:

- **No new `mod.rs`.** A parent stays `foo.rs`; children live in `foo/`.
- Splits are **moves, not rewrites** — the same code, relocated, with visibility
  widened only as far as the new module boundary requires. No behaviour change.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
  must pass after each pass, and each pass is its own commit.
- Test modules split alongside the code they cover, keeping shared fixtures in the
  parent `tests.rs`.

## Offenders (baseline, 36,873 lines total)

| Lines | File | Status |
|------:|------|--------|
| 4339 | `src/state/sim/factions.rs` | pending |
| 4252 | `src/data.rs` | pending |
| 3856 | `src/simulation/tick/tests.rs` | pending |
| 3833 | `src/simulation/event_resolver.rs` | pending |
| 1841 | `src/simulation/subsystems.rs` | pending |
| 1653 | `src/simulation/contract.rs` | pending |
| 1259 | `src/simulation/tick.rs` | pending |
| 1178 | `src/state/sim.rs` | pending |
| 1043 | `src/ui.rs` | pending |
|  925 | `src/ui/ship_schematic.rs` | pending |
|  808 | `src/simulation/tick/economy.rs` | pending |

## Planned cuts

### `src/state/sim/factions.rs` (4339)

One `impl SimState` of ~1730 lines plus ~2480 lines of tests. Cuts by responsibility:

- `factions.rs` — `FactionStatus`, `FactionState`, band helpers, `build_founding_factions`,
  and the read-only queries (`aboard_*`, `dominant_faction_id`, `tender_approval`).
- `factions/roster.rs` — who is aboard: rebalance, loss, merge, remove, assimilate, recruit.
- `factions/sentiment.rs` — the `apply_*` approval/cohesion movers.
- `factions/announce.rs` — the ~20 `announce_*` band-crossing narrators.
- `factions/tests.rs` + `factions/tests/{roster,sentiment,announce}.rs`.

### `src/data.rs` (4252)

Almost entirely `serde` config structs with `#[serde(default)]` fn defaults. Cut into
`data/config/` by config area (`ship`, `flavor`, `campaign`, `crew`, `tutorial`), leaving
`data.rs` as the loader (`GameData`, the `include_str!` manifest, `Acquisition`, deltas).

### `src/simulation/event_resolver.rs` (3833)

~770 lines of resolver plus ~3060 of tests. `gating.rs` (`passes_gate`, availability),
`rolling.rs` (weights, `pick_weighted`, `roll_event*`), `outcome.rs` (`score_outcome`,
`apply_outcome`, `auto_resolve`), plus a `tests/` tree mirroring those.

### `src/simulation/tick/tests.rs` (3856)

Pure test file, ~80 tests. Split by what they cover: `drift`, `voice` (quiet/flavor
narration), `beats` (the large scripted/threshold beat family), `economy`, `determinism`.

### The remaining seven

Smaller and more mechanical — each has one or two cohesive responsibilities to lift out.
Planned once the four large ones land, so the shape of the shared helpers is settled.

## Log

_(one line per completed pass)_
