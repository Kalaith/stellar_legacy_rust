# W1 — Rescale missions to generational length (300–600 yr) + autoplay harness

## Goal

Every charter becomes a **≥300-year** voyage. Retune ship decay, drift, and upkeep so a
300–600 yr voyage is *survivable but demanding* (today's numbers were tuned for ~55 yr and
would destroy a 300-yr ship). Stand up an **automated full-mission playthrough harness**
(policy-driven soak test) — the owner's primary playtest channel going forward.

This workstream is **data + tests only**, plus one tiny code change (moving a hardcoded
constant into config). No new mechanics.

## Binding owner decisions

- Missions are ≥300 years, no cryosleep; the ship returns home (round trip).
- Numbers live in `assets/data/game_config.json` / `assets/contracts.json`, never in Rust.
- Temporarily worse pacing (1 press = 1 year, ~300 presses) is accepted; W3 fixes it.
- Most playtesting will be automated — the soak/autoplay harness is a first-class deliverable.

## Current state (verified facts)

- Charters: `assets/contracts.json` — 10 templates, `target_duration_years` 22–80.
- Config: `assets/data/game_config.json` — `hull_decay_per_year: 0.011`,
  `life_support_decay_per_year: 0.008`, `maintenance_decay_relief: 0.4`,
  `parts_upkeep_per_year: 1`, `generation_interval_years: 25`,
  `voyage_drift.adaptation_per_year: 0.004`, `voyage_drift.cultural_drift_per_year: 0.004`.
- Yearly tick: `src/simulation/tick.rs::advance_year` (production → food upkeep → wear →
  drift → generation/succession → contract progress → market → event roll).
- Starting spare parts are **hardcoded** as `spare_parts: 20` in
  `src/state/sim.rs` (`SimState::new_campaign`, ShipState literal, ~line 357).
- Field repair caps at `repair.field_ceiling: 0.75`, costs 4 parts + 150 minerals per use;
  full (port) repair restocks parts to `full_parts_restock: 20`.
- Existing soak test: `long_campaign_stays_internally_consistent` in
  `src/simulation/tick.rs` (250 iterations, picks outcome 0 for every decision).
- Config struct: `GameConfig` in `src/data.rs:81`.

## Changes

### 1. Rescale `assets/contracts.json`

Set `target_duration_years` per charter (bands 300–600, roughly preserving today's
relative ordering):

| id | new duration |
| --- | --- |
| `tarssen_relief` | 300 |
| `hollow_fleet` | 310 |
| `coronal_tap` | 320 |
| `deep_vein_survey` | 340 |
| `veiled_expanse_survey` | 360 |
| `warden_patrol` | 380 |
| `starfall_beacon` | 400 |
| `seedfall` | 420 |
| `founding_colony` | 450 |
| `the_long_dark` | 600 |

Rewrite each `description` string so the prose matches (they currently say "Forty years…",
"sixty years…" etc. — change to the century scale, keep the tone). Do not change
milestones, metrics, rewards, or `min_renown`.

### 2. Move starting spare parts into config

- Add `pub starting_spare_parts: i64` to `GameConfig` (`src/data.rs`) and
  `"starting_spare_parts"` to `game_config.json`.
- Use it in `SimState::new_campaign` instead of the literal `20`.

### 3. Retune `assets/data/game_config.json`

First-pass values (the tests in step 4 are the arbiter — adjust until they pass, keeping
the *spirit*: a maintained, repaired ship completes 400 yr worn-but-alive; a neglected
one fails around year 150–200):

- `hull_decay_per_year`: `0.011` → `0.0035`
- `life_support_decay_per_year`: `0.008` → `0.0025`
- `starting_spare_parts`: `60`
- `repair.full_parts_restock`: `20` → `60`
- `voyage_drift.adaptation_per_year`: `0.004` → `0.0018`
- `voyage_drift.cultural_drift_per_year`: `0.004` → `0.0018`
- `voyage_drift.legacy_loyalty_per_year`: `-0.003` → `-0.0014`
- `voyage_drift.morale_strain_per_year` / `unity_strain_per_year`: keep (recoverable via
  crew and events).

Leave `event_chance_base`/`event_chance_cap`, generation, and crew numbers alone — W3/W6
own pacing.

### 4. Autoplay harness + extended soak

Create `src/simulation/autoplay.rs`, registered in `src/simulation.rs` as
`#[cfg(test)] pub mod autoplay;`. It contains:

- `pub fn play_mission(sim: &mut SimState, data: &GameData, contract_id: &str, max_years: u32) -> MissionOutcome`
  — a policy player: starts the contract, then loops `advance_year`, resolving every
  pending dilemma/event by **first choice (index 0)** (same policy as today's soak),
  doing a field repair whenever `hull_integrity < 0.5` and affordable, and buying food
  when below `low_food_threshold` and affordable. Returns an outcome struct
  (`completed: bool`, `extinct: bool`, final year, final score).
- Move the existing `long_campaign_stays_internally_consistent` test here, extended:
  - run `play_mission` on `deep_vein_survey` (now 340 yr) with `max_years: 420`;
  - keep every per-year invariant assertion (0–1 fractions, non-negative resources,
    living dynasty has a leader);
  - assert the contract **completes** and the dynasty is **not extinct**
    (pick/keep a seed that survives; determinism makes this stable);
  - assert `sim.dynasty.generation >= 12` at completion (12+ successions over 340 yr).
- Add a second test: same policy, `the_long_dark` (600 yr), asserting only invariants
  and that the run ends in either completion or extinction (total loss is a legal
  outcome at 600 yr).

### 5. Update broken tests

- `a_long_voyage_leaves_the_ship_worn_and_out_of_parts` (`src/simulation/tick.rs`):
  re-derive its expectations from the new constants — extend the loop to ~300 years and
  update the asserted hull range so it still expresses "worn but flying".
- `contract_completes_at_target_duration` loops `target_duration_years` times — now 340
  iterations; it should still pass unchanged, but verify.
- `src/data.rs::embedded_data_loads`: add an assertion that **every** contract has
  `target_duration_years >= 300`.

## Acceptance criteria

- All verification commands green (see below).
- All 10 charters ≥300 yr; new data-test assertion enforces it.
- Autoplay harness completes a 340-yr mission with a living dynasty, ≥12 generations.
- No Rust file exceeds 800 lines (`src/simulation/tick.rs` shrinks — the soak moved out).
- `git diff` shows no balance number introduced in Rust source (config/data only, except
  the `starting_spare_parts` plumbing).

## Ground rules

1. Data-driven: all tuning in `assets/*.json`; never hardcode balance in Rust.
2. 800-line hard limit per `.rs` file; extract sibling modules; never create `mod.rs`.
3. UI is a pure view; mutation only via `UiAction` dispatch / `simulation` services.
4. Determinism: randomness only through `sim.rng`.
5. Old saves are abandoned; no migration shims.
6. Delete unused code outright.

## Verification

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo check --release --target wasm32-unknown-unknown
```
