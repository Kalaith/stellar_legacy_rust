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
| 4339 | `src/state/sim/factions.rs` | **done** — 12 files, largest 616 |
| 4252 | `src/data.rs` | **done** — 17 files, largest 546 |
| 3856 | `src/simulation/tick/tests.rs` | **done** — 11 files, largest 593 |
| 3833 | `src/simulation/event_resolver.rs` | **done** — 14 files, largest 490 |
| 1841 | `src/simulation/subsystems.rs` | **done** — 7 files, largest 509 |
| 1653 | `src/simulation/contract.rs` | **done** — 6 files, largest 568 |
| 1259 | `src/simulation/tick.rs` | **done** — 7 files, largest 261 |
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

- **Pass 1** — `state/sim/factions.rs` 4339 → `factions.rs` (245) + `roster.rs` (521),
  `sentiment.rs` (451), `announce.rs` (433), `announce/condition.rs` (214), and a
  `tests/` tree of six files (276–616). All 50 tests preserved, fmt/clippy/test green.
- **Pass 2a** — `data.rs` 4252 → `data.rs` (194) + `config.rs` (316) +
  `config/{ship,flavor,campaign,crew,onboarding}.rs` (42–546), re-exported flat.
- **Pass 2b** — the 2498-line `data/tests.rs` → nine area files (108–516) behind two
  shared fixtures. The two monster tests (821 and 1214 lines) became fourteen named
  tests; all 296 assertions preserved, 354 → 366 tests, all passing.
- **Pass 3** — `simulation/event_resolver.rs` 3833 → `event_resolver.rs` (181, availability
  and presentation) + `rolling.rs` (389) + `outcome.rs` (226), re-exported flat, and ten
  test files (174–490). All 78 tests preserved.
- **Pass 4** — `simulation/tick/tests.rs` 3856 → `tests.rs` (60, the three shared fixtures)
  + ten files (106–593) by what the year under test exercises. All 73 tests preserved.
- **Pass 5** — `simulation/subsystems.rs` 1841 → `subsystems.rs` (135, the buffering and
  softening helpers) + `verbs.rs` (201) + `effects.rs` (432), re-exported flat, and three
  test files (205–509). All 30 tests preserved.
- **Pass 6** — `simulation/contract.rs` 1653 → `contract.rs` (568, all the code, already
  under the limit) + `tests.rs` (35, fixtures) + four test files (168–418) by which part
  of a voyage's life they guard. All 32 tests preserved.
- **Pass 7** — `simulation/tick.rs` 1259 → `tick.rs` (261, the advance loop) + `beats.rs`
  (103, `fire_due_beat` and the force/clear helpers) + `beats/{threshold,collapse,recovery,
  scripted,succession}.rs` (84–240), mirroring the pass-4 test layout.
