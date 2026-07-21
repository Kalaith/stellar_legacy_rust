# W3 — Month-resolution clock + speed selector (dated events)

**Prerequisite: W1 complete and green.**

## Goal

Replace the bare year counter with a **month clock**. Add a **speed selector**
(`1 month → 1 year → 5 years → 10 years`); one Advance press fast-forwards up to that
span but **hard-stops the instant a decision fires**, stamping the exact date. Every
event and log line carries its month. Time still only moves on an explicit Advance
(Pillar 4).

## Binding owner decisions

- The game **advances in months**; the player can speed up. Ratified approach: the
  advance loop steps month by month; **events roll and are dated at month resolution**,
  but the **economic tick (production, decay, drift, aging, succession, contract
  progress, market) applies on year boundaries** so the W1-tested math stays intact.
- Every event carries the specific date it triggered.
- Old saves are abandoned — restructure `SimState` freely, no migration shims.

## Current state (verified facts)

- `SimState` (`src/state/sim.rs:297`): `pub year: u32`, `pub last_event_year: u32`,
  `log: Vec<LogEntry>` where `LogEntry { year: u32, text: String }`,
  `pending_event: Option<PendingEvent>` (`PendingEvent { template_id, rolled_year }`),
  `pending_dilemma: Option<PendingDilemma>` (same shape).
- Tick: `src/simulation/tick.rs::advance_year(sim, data) -> TickReport` does everything
  for one year; `TickReport { contract_completed, decision_required, dynasty_extinct }`.
- Dispatch: `UiAction::AdvanceYear` in `src/ui.rs:161`, handled in
  `src/game/actions.rs` (`Game::advance_year`, which also records the Chronicle entry
  on contract completion).
- Event chance ramps with years since `last_event_year`
  (`src/simulation/event_resolver.rs`); base/cap in config
  (`event_chance_base: 0.3`, `event_chance_cap: 0.8` — **per-year** probabilities).
- Many modules read `sim.year` (log, chronicle, achievements, UI header). The compiler
  will find every site once the field changes.

## Changes

### 1. Date model (`src/state/sim.rs`)

- Replace `pub year: u32` with `pub month_clock: u32` (months since founding, starts 0).
- Add methods:
  ```rust
  pub fn year(&self) -> u32 { self.month_clock / 12 }
  /// 1-12 for display.
  pub fn month(&self) -> u32 { self.month_clock % 12 + 1 }
  ```
- Replace `last_event_year: u32` with `last_event_month_clock: u32`.
- `LogEntry` becomes `{ year: u32, month: u32, text: String }`; `push_log` stamps both
  from the clock.
- `PendingEvent`/`PendingDilemma`: replace `rolled_year` with `rolled_month_clock: u32`.
- Add to `SimState`: `pub speed: SpeedStep` with
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "snake_case")]
  pub enum SpeedStep { OneMonth, OneYear, FiveYears, TenYears }
  ```
  default `OneYear` (implement `Default`). Add `pub fn months(self) -> u32`
  (1 / 12 / 60 / 120).
- Fix every compile error from the `year` rename by calling `sim.year()` (display) or
  using `month_clock` (arithmetic). Chronicle `completed_year` stays a year number.

### 2. Tick restructure (`src/simulation/tick.rs`)

- Extract the current `advance_year` body (everything except the event roll and the
  `sim.year += 1`) into `fn year_boundary_tick(sim, data, report)` — production, food
  upkeep, wear, drift, generation/succession/dilemma, contract progress + completion
  check, market drift. **Do not change its internal math** (W1 tuned it).
- New entry point:
  ```rust
  pub fn advance(sim: &mut SimState, data: &GameData) -> TickReport
  ```
  which loops up to `sim.speed.months()` times; each iteration:
  1. `sim.month_clock += 1;`
  2. if `sim.month_clock % 12 == 0` → `year_boundary_tick(...)` (a full year has elapsed)
  3. monthly event roll (see step 3) unless a dilemma is already pending
  4. **stop the loop immediately** if `report.decision_required`,
     `report.contract_completed.is_some()`, or `report.dynasty_extinct`.
- `TickReport` gains `pub months_advanced: u32`.
- Keep a thin test helper `pub fn advance_year(sim, data) -> TickReport` that sets
  `sim.speed = SpeedStep::OneYear` and calls `advance` — existing tests keep working
  with minimal edits.

### 3. Monthly event probability (`src/simulation/event_resolver.rs`)

The existing roll computes a **per-year** chance from base/cap + ramp. Convert to
per-month at the call site: `monthly_chance = yearly_chance / 12.0`. Ramp input becomes
`(sim.month_clock - sim.last_event_month_clock) / 12` years. On fire, set
`last_event_month_clock = sim.month_clock`. Expected events per year is preserved;
determinism per seed changes (acceptable — update test expectations, never hack around
them).

### 4. UI (`src/ui.rs`, `src/ui/dashboard.rs`)

- `UiAction::AdvanceYear` → rename `UiAction::Advance`; add `UiAction::SetSpeed(SpeedStep)`.
- Dispatcher (`src/game/actions.rs`): `Advance` calls `tick::advance`; `SetSpeed` writes
  `sim.speed`. Keyboard: Space stays Advance.
- CRT header shows the mission clock `Y143 · M07` (from `sim.year()` / `sim.month()`).
- Add a 4-button speed row (`1mo / 1yr / 5yr / 10yr`), highlighting the active step.
  Follow the existing button/widget style in `src/ui.rs`.
- Log lines render their date (`Y12·M03`).

### 5. Tests

- Update existing tick tests for the new API (`advance_year` helper keeps most intact).
- New tests in `tick.rs`:
  - one `advance` at `TenYears` with events disabled advances exactly 120 months and
    applies exactly 10 year-boundary ticks (compare against 10 `advance_year` calls on
    an identical seed);
  - with `dilemma_chance_per_generation = 1.0`, a `TenYears` advance **stops early** at
    the generation boundary month with a pending dilemma and
    `months_advanced < 120`;
  - a fired event's `rolled_month_clock` matches a dated log entry.
- Autoplay harness (`src/simulation/autoplay.rs`): drive via `advance` at `TenYears`;
  soak invariants unchanged and green.

## Acceptance criteria

- All verification commands green.
- One Advance press at 10-yr speed crosses up to 120 months and never skips past a
  decision, generation dilemma, or contract completion.
- Every log line and pending event carries a month-precise date.
- W1's autoplay soak still completes its 340-yr mission.

## Ground rules

1. Data-driven: tuning in `assets/*.json` only.
2. 800-line hard limit per `.rs` file — `tick.rs` will grow; extract
   (e.g. `simulation/tick/` children) before it crosses 600.
3. UI is a pure view; mutation only via `UiAction` dispatch / `simulation` services.
4. Determinism: randomness only through `sim.rng`.
5. Old saves abandoned; no migration shims.
6. Delete unused code outright (e.g. the old `last_event_year` plumbing).

## Verification

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo check --release --target wasm32-unknown-unknown
```
