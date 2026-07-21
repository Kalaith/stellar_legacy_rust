# W2 — Real mission phases (travel → operation → return) + early truncation

**Prerequisite: W3 complete and green (month clock exists).**

## Goal

Promote `ContractPhase` from a cosmetic %-derived label to **authored phase segments** on
each charter. Objective work happens only on-station (Operation) and is a **quantified
counter** (mine X of Y). The voyage can **end early** — by catastrophe, by fortunate
find, or by a player **[ TURN BACK ]** — and **pay is strictly proportional to objective
progress** (no progress = no pay).

## Binding owner decisions

- Mission length is **fixed by the charter** (authored phases). The player never tunes
  phase lengths; they choose *between* charters.
- Early end paths: (a) bad event (disaster, resource unavailable) forces Return;
  (b) fortunate event (found something more valuable) sends the ship home early;
  (c) a player abort verb, available any time underway.
- **Pay = charter reward × objective completion fraction**, clamped to [0, 1].
  Objectives are measured amounts (mined X, built X, explored X), not timers.
- Total loss (extinction) remains possible; failure is a spectrum.
- Missions must never be hardcoded — phases and objective targets are charter JSON.

## Current state (verified facts)

- `ContractPhase` enum (`src/data/contracts.rs:29`): Preparation/Travel/Operation/
  Return/Completion. `ContractPhase::for_progress` derives the label from a 0–1
  fraction (0.2/0.8 split) — **this is the cosmetic behavior being replaced.**
- `ContractTemplate` (`src/data/contracts.rs:95`): `target_duration_years`, milestones
  (fraction thresholds), success metrics, reward, `min_renown`.
- `ActiveContract` (`src/state/sim.rs:173`): `years_elapsed`, `phase`, `bonus_progress`,
  `progress()` = elapsed/duration.
- Progression: `src/simulation/contract.rs::advance_contract` (called from the year
  boundary tick), milestone rewards, metric refresh; `score_success` bands
  Complete/Partial/Pyrrhic/Failure.
- Completion + reward payout: `Game::advance_year` in `src/game/actions.rs` — pays the
  **full** template reward on any non-failure. Chronicle entry recorded there.
- Event outcomes: `EventOutcome` in `src/data/events.rs:35` (serde-default fields).

## Changes

### 1. Charter schema (`src/data/contracts.rs` + `assets/contracts.json`)

- Add:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct PhaseDef {
      pub kind: ContractPhase,   // only Travel | Operation | Return are valid here
      pub years: u32,
  }
  ```
- `ContractTemplate` gains (required fields, no serde default — all charters are
  authored in this same change):
  - `pub phases: Vec<PhaseDef>`
  - `pub objective_target: f32` (e.g. 1200.0)
  - `pub objective_unit: String` (e.g. "proof-of-yield cores", "settlers landed",
    "systems charted", "souls recovered")
- Author all 10 charters in `assets/contracts.json`: phase years sum **exactly** to
  `target_duration_years`, typically travel ≈ operation ≈ return (e.g. deep_vein_survey
  340 = travel 110 / operation 120 / return 110). `the_long_dark` (600, exploration) may
  use travel 250 / operation 100 / return 250. Pick a sensible `objective_target` per
  charter; keep milestones as-is (they stay fraction-of-total-duration based).
- Delete `ContractPhase::for_progress` once nothing calls it.

### 2. Active contract state (`src/state/sim.rs`)

`ActiveContract` changes:
- `years_elapsed: u32` → `months_elapsed: u32` (contract time is now month-precise).
- Add `phases: Vec<PhaseDef>` (copied from template at start), `phase_index: usize`.
- Add `objective_target: f32`, `objective_unit: String`, `objective_progress: f32`.
- `phase` field stays but is now **set from the authored segments**, never derived:
  phase = `phases[phase_index].kind`; boundaries at cumulative `years * 12` months.
  Before launch it is `Preparation`; after the last segment it is `Completion`.
- `progress()` becomes `months_elapsed as f32 / (target_duration_years * 12) as f32`
  (still used for milestones and the UI bar).
- Add `pub fn objective_fraction(&self) -> f32` = `(objective_progress / objective_target).clamp(0.0, 1.0)`
  (target 0 ⇒ 1.0).

### 3. Progression (`src/simulation/contract.rs`)

- `advance_contract` now advances **one month** of contract time (call it from the
  monthly loop in `tick::advance`, not the year boundary — move the call; milestone
  rewards and metric refresh logic move with it unchanged in spirit).
- Each month: `months_elapsed += 1`; recompute `phase_index` from the segment table;
  if the phase changed, log it (e.g. "The ship makes orbit. On-station operations
  begin.") and report it (see step 5).
- **Objective accrual only during Operation:**
  `objective_progress += base_rate * speed_bonus_factor` where
  `base_rate = objective_target / operation_months` and
  `speed_bonus_factor = 1.0 + speed as f32 * config.ship.contract_progress_per_speed`
  (reuse the existing config knob; delete `bonus_progress` and its uses).
- `MetricKind::MissionCompletion` current value = `objective_fraction()`.
- Completion: when `months_elapsed >= target_duration_years * 12`, the contract is
  complete (detected in tick as today, via the report).

### 4. Early return / truncation

- `EventOutcome` (`src/data/events.rs`) gains
  `#[serde(default)] pub force_return: bool` — when an applied outcome has it set and
  a contract is active in Travel-out or Operation, the contract **jumps to its Return
  segment** (set `phase_index` to the first Return segment; set `months_elapsed` to
  that segment's cumulative start) and logs it. Works for both catastrophe and
  fortunate-find outcomes — the outcome's other deltas (e.g. a big credits windfall)
  carry the flavor.
- New `UiAction::AbortMission`: available only while a contract is active and not
  already in Return; applies the same jump; logs "The council votes to turn back."
- **Payout proration** (in `Game`'s contract-completion handling,
  `src/game/actions.rs`): replace the full-reward payout with
  `reward × objective_fraction()` applied per resource field (round toward zero).
  This rule applies to *every* completion — full-term or truncated. Failure band no
  longer zeroes the payout by itself; zero objective progress does.
- Extinction mid-mission behaves exactly as today (game over; no payout).

### 5. Tick + UI integration

- `TickReport` gains `pub phase_changed: Option<ContractPhase>`; a phase boundary is a
  **hard-stop** for the fast-forward loop in `tick::advance` (same as decisions).
- Dashboard/contract UI (`src/ui/dashboard.rs`, `src/ui/contract_systems.rs`): show the
  authored phase timeline, current phase, and the objective counter
  ("612 / 1200 proof-of-yield cores"). Add the [ TURN BACK ] button (underway only, not
  in Return). Follow existing widget style.
- Give at least two existing event templates in `assets/events.json` a
  `force_return: true` outcome (one catastrophic, one fortunate windfall) so the path
  has content. Keep it to outcomes where that narratively fits.

### 6. Tests

- `src/data.rs::embedded_data_loads` additions: every contract's phase years sum to
  `target_duration_years`; kinds only Travel/Operation/Return; at least one Operation
  segment; `objective_target > 0`.
- `contract.rs` tests: phase boundary months computed correctly; objective accrues only
  during Operation; `objective_fraction` clamps; milestone rewards still one-shot.
- Truncation test: force a `force_return` outcome mid-Operation → contract enters
  Return, completes early, payout equals `reward × objective_fraction` (assert exact
  numbers on a fixed seed).
- Abort test: `AbortMission` during Travel-out → payout 0 on completion (no objective
  progress).
- Autoplay harness: unchanged policy still completes a full mission; add an
  abort-at-year-150 scenario asserting reduced pay.

## Acceptance criteria

- All verification commands green.
- Phase labels come only from authored segments (`for_progress` deleted).
- A truncated mission pays exactly proportional to objective completion; a zero-progress
  abort pays nothing.
- Fast-forward hard-stops on phase boundaries with a dated log line.

## Ground rules

1. Data-driven: phases/objectives in `assets/contracts.json`; no mission logic constants
   in Rust.
2. 800-line hard limit per `.rs` file; extract sibling modules; never create `mod.rs`.
3. UI is a pure view; the [ TURN BACK ] button only emits `UiAction::AbortMission`.
4. Determinism: randomness only through `sim.rng`.
5. Old saves abandoned; no migration shims.
6. Delete unused code outright (`for_progress`, `bonus_progress`).

## Verification

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo check --release --target wasm32-unknown-unknown
```
