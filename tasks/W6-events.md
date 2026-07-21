# W6 — Event families, phase gating, campaign skeleton, content build-out

**Prerequisite: W5 complete and green.**

## Goal

Turn the flat 46-event pool into a **family × phase × gate** catalog and give every
mission a **seeded campaign skeleton generated at LAUNCH** — so a 300–600 yr voyage
plays like a generated campaign, not a random-event stream. Fill every family to
**6–10 templates** (keep existing events; where a family already has ≥10, keep them
all). Structure is the priority; more content can land later.

## Binding owner decisions

- Each mission ≈ a campaign **generated over the course of the journey**.
- **Seeded skeleton at LAUNCH**: major beats laid out deterministically from the mission
  seed; reactive/filler events roll as time advances. Same seed ⇒ same campaign.
- 6–10 events per family suffices; structure first, content depth later.
- Long-Term Expedition beats gate on **year/generation and voyage-drift state**, so they
  read as consequences of the long voyage.
- Placeholder flavor gets in-universe names (the "Tribbles" infestation → **"Lobites"**).
- All content in `assets/events.json` + notes in `event_design_notes.md`; never Rust.

## Current state (verified facts)

- `EventTemplate` (`src/data/events.rs:58`): id, category (4-value scoring axis —
  **unchanged**), title, description, requires_decision, legacy_weight_modifiers,
  outcomes. W5 added `family: String` (serde default). W2 added
  `EventOutcome.force_return`; W7 added `EventOutcome.faction_loss`.
- Roll/score/resolve: `src/simulation/event_resolver.rs` (monthly chance since W3;
  subsystem weight/severity scaling since W5).
- Contract phases + `ActiveContract.phase` / `phases` (W2). Month clock (W3).
- Drift stats: `population.cultural_drift` / `adaptation` / `legacy_loyalty`;
  generation count: `dynasty.generation`.
- Data tests live in `src/data.rs::tests`.

## The 10 families (canonical strings)

`exploration_first_contact`, `diplomacy`, `engineering`, `biology_medical`,
`science_anomaly`, `survival`, `mystery`, `comedy`, `ethics`, `legacy_drift`.

Category mapping guidance (category stays the scoring/weighting axis):
Exploration/Anomaly/Comedy → mostly `legacy_moment` / `mission_milestone`;
Diplomacy/Ethics/legacy_drift → `generational_challenge`;
Engineering/Biology/Survival → `immediate_crisis`; Mystery → `mission_milestone`.

## Changes

### 1. Template schema (`src/data/events.rs`) — all `#[serde(default)]`

```rust
/// Which contract phases this event may fire in. Empty = any phase.
pub phases: Vec<crate::data::contracts::ContractPhase>,
/// Gates: 0 = ungated.
pub min_year: u32,
pub min_generation: u32,
pub min_cultural_drift: f32,
```

(`family: String` and `subsystem` handling already exist from W5.)

### 2. Roll filtering (`src/simulation/event_resolver.rs`)

Before weighting, exclude any template failing:
- `phases` non-empty and (no active contract, or current phase not in the list);
- `sim.year() < min_year`; `sim.dynasty.generation < min_generation`;
- `sim.population.cultural_drift < min_cultural_drift`.

Keep every existing weighting input (category scoring, legacy modifiers, consequences,
subsystem multipliers) on the survivors.

### 3. Campaign skeleton (seeded beats at LAUNCH)

- `ActiveContract` gains:
  ```rust
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct CampaignBeat {
      pub month_clock: u32,      // absolute month it fires
      pub family: String,
      pub fired: bool,
  }
  pub beats: Vec<CampaignBeat>,
  ```
- In `start_contract` (which since W4 runs at LAUNCH), generate beats from `sim.rng`:
  - one beat per full **20 years** of mission duration (a 340-yr charter → 17 beats),
    each placed uniformly at random **within its own 20-yr window** (deterministic via
    `sim.rng`), skipping the first 5 years;
  - each beat's family drawn (via `sim.rng`) from the phase-appropriate pool for the
    phase active at that month — Travel: `exploration_first_contact`,
    `science_anomaly`, `diplomacy`, `mystery`, `engineering`; Operation:
    `survival`, `diplomacy`, `engineering`, `mystery`; Return: `legacy_drift`,
    `ethics`, `mystery` — plus `biology_medical`/`comedy` allowed anywhere. Encode
    these pools as a constant table in `event_resolver.rs` (families are content
    strings, but the *pool structure* is mechanics — a code table is acceptable; the
    owner-facing content stays in JSON).
- Monthly loop (`tick::advance`): when `month_clock` reaches an unfired beat, force an
  event: filter the catalog to that family (plus all step-2 gates), pick by the normal
  weighting, mark the beat fired. If the filter leaves nothing (over-gated family),
  mark fired and fall through to the normal random roll — never crash, never stall.
- Beats **replace** that month's random roll; random rolls continue in non-beat months
  (they are the reactive/filler layer).

### 4. Content build-out (`assets/events.json`)

1. **Tag every existing template** with `family` (and `phases`/gates where they
   obviously apply). Rename any Tribbles-flavored content to **Lobites**.
2. **Fill each family to ≥6 templates** (cap new authoring at 10/family). Draw beats
   from `event_notes.md` at the repo root — it is the brainstorm catalog. Priorities:
   - **`legacy_drift` is the headline family** — author the Long-Term Expedition beats:
     home civilization changed/vanished (gate `min_year: 100`), mission-becomes-religion
     (`min_generation: 5`, `min_cultural_drift: 0.5`), ship factions split
     (`min_cultural_drift: 0.6`, pair with `faction_loss: departed` outcomes),
     descendants settle a world (`faction_loss: settled`, population drain),
     AI keeper-of-memory arcs.
   - `survival` gets the provisioning tie-ins: a garden-world stop that grants food but
     costs a faction (`faction_loss: settled`) — the owner's canonical example; fuel
     skimming; radiation/ion-storm pressure.
   - At least one `force_return: true` catastrophic outcome (Operation-gated) and one
     fortunate `force_return: true` windfall (Travel-gated), beyond W2's two.
   - `comedy` stays low-stakes (Lobites infestation, bureaucrat inspection,
     translation mishaps) — tension breakers.
3. Every outcome keeps the house style: 2–4 outcomes, meaningful deltas, a `log` line,
   `long_term_consequences` where a debt should linger.

### 5. Design notes file

Create `event_design_notes.md` at the project root: copy the "Event-System Design
Notes" section from `plan.md` (families table, generation principle, phase-weighting
rules) and add a per-family inventory table (count, gated beats, placeholders
remaining). This is the living content-authoring reference; `event_notes.md` remains
the raw brainstorm.

### 6. Tests (`src/data.rs::tests` + `event_resolver.rs` tests)

- Every template has a non-empty `family` from the canonical 10-string set.
- Every family has ≥6 templates; total ≥60.
- Every `phases` entry is Travel/Operation/Return only.
- Gate filtering: a `min_cultural_drift: 0.6` template never rolls at drift 0.2 and can
  roll at 0.7 (fixed seed).
- Skeleton: `start_contract` on a fixed seed yields identical beats twice;
  17 beats for a 340-yr charter; every beat fires (or falls through) during an autoplay
  run — assert all `fired` by mission end.
- Autoplay soak stays green; add an assertion that a full 340-yr run fires ≥10 beat
  events (campaign density).

## Acceptance criteria

- All verification commands green.
- Same seed ⇒ identical beat schedule and (given identical choices) identical campaign.
- All 10 families ≥6 templates, tagged and gated; Lobites replaces any Tribbles flavor.
- `event_design_notes.md` committed alongside the catalog.
- **This completes the redesign:** run `.\publish.ps1`, verify at
  `http://127.0.0.1/stellar_legacy/`, and capture
  `-Scenes menu,prep,gameplay,contract_active,drydock` for a visual pass.

## Ground rules

1. Data-driven: all event content in `assets/events.json`; only the beat-pool table
   lives in code (mechanics, not content).
2. 800-line hard limit; `event_resolver.rs` grows here — extract
   `simulation/event_resolver/` children (e.g. `skeleton.rs`, `gating.rs`) before 600.
3. UI is a pure view (no UI changes expected beyond the event modal already handling
   dated events).
4. Determinism: skeleton and rolls only through `sim.rng`.
5. Old saves abandoned; no migration shims.
6. Delete unused code outright.

## Verification

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo check --release --target wasm32-unknown-unknown
.\publish.ps1   # final workstream only — then verify http://127.0.0.1/stellar_legacy/
```
