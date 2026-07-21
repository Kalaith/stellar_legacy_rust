# W5 — Ship subsystems, decay, knowledge-gated repair, reactive upgrades

**Prerequisite: W4 complete and green.**

## Goal

Add a **module/subsystem layer** beyond hull/engine/weapon: **medical bay,
life-support/habitat, agriculture, security, education/culture, engineering bay** —
each with upgrade tiers that **buffer a specific event family**. Subsystems **decay and
need repairs en route**, and **repair requires living expertise**: a per-subsystem
**institutional-knowledge stat** carried by the population — if everyone who understands
a system dies untrained, it cannot be fixed until knowledge is rebuilt. On return, the
reward is spent in drydock to strengthen whatever hurt you ("illness → strengthen the
medical bays").

## Binding owner decisions

- **Full module family** (all six), each buffering one event family.
- Everything decays; **repair gated by living expertise** — knowledge dies with people;
  the education/culture subsystem transmits knowledge across generations.
- Knowledge model: **per-subsystem aggregate stat** (0–1), raised by education/training,
  decaying when experts die untrained. Not per-crew, not per-faction.
- Reactive loop: upgrades are drydock (port-only) purchases from mission rewards.
- Catalog and tunables in `assets/*.json`, never Rust.

## Current state (verified facts)

- Ship slots today: `ShipState { hull, engine, weapon, ... }` (`src/state/sim.rs:48`);
  catalog `assets/ship_components.json` via `src/data/ship_components.rs`; port-only
  purchase gate in `Game::purchase_component` (`src/game/actions.rs`).
- Event templates carry a `subsystem` tag **only after W6**; in W5 add the field (see
  step 4) — W6 fills the content.
- Event application: `event_resolver::apply_outcome` / `auto_resolve`
  (`src/simulation/event_resolver.rs`); roll weighting also there.
- Wear model: hull/life-support decay in the year-boundary tick with
  `maintenance_decay_relief` while parts last (`src/simulation/tick.rs`).
- Generation boundary (succession, deaths) happens inside the year tick every
  `generation_interval_years`.
- Field vs port repair pattern: `simulation/ship.rs::field_repair` / `full_repair`,
  config `repair` block.

## Changes

### 1. Subsystem catalog (`assets/subsystems.json`, `src/data/subsystems.rs`)

New data module (register in `src/data.rs`, embed via `include_str!`, load into
`GameData.subsystems: DataRegistry<SubsystemDef>`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemTier {
    /// Drydock upgrade cost to reach this tier from the one below.
    pub cost: ResourceDelta,          // authored as positive numbers; negate on spend
    /// 0-1: fraction of a matching event's negative deltas prevented at full condition.
    pub severity_reduction: f32,
    /// Multiplier on a matching event's roll weight (e.g. 0.8 = 20% rarer).
    pub weight_multiplier: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemDef {
    pub id: String,
    pub name: String,
    /// Event family this subsystem buffers (matches EventTemplate.family, W6).
    pub buffers_family: String,
    pub decay_per_year: f32,
    /// Knowledge needed to repair it (0-1).
    pub repair_knowledge_required: f32,
    /// Repair cost per use (parts + minerals), field-style.
    pub repair_parts_cost: i64,
    pub repair_minerals_cost: i64,
    /// Tier 0 is the ship's baseline (cost ignored); tiers 1.. are upgrades.
    pub tiers: Vec<SubsystemTier>,
    pub description: String,
}
```

Author all six in `assets/subsystems.json` with 3 upgrade tiers each (tier 0 baseline +
tiers 1–3). Ids and buffered families (family strings must match W6's list exactly):

| id | buffers_family | extra tier effect |
| --- | --- | --- |
| `medical_bay` | `biology_medical` | — |
| `agriculture` | `survival` | `+agriculture_food_bonus_per_tier` food production per tier |
| `security` | `diplomacy` | — |
| `education_culture` | `legacy_drift` | knowledge transmission (step 2) |
| `engineering_bay` | `engineering` | — |
| `life_support_habitat` | `""` (empty — buffers no event family) | reduces `life_support_decay_per_year` by `severity_reduction × tier's value` |

When `buffers_family` is the empty string, skip all event-family matching for that
subsystem; its tiers act only through the extra effect column.

Config block:

```json
"subsystems": {
  "knowledge_start": 0.7,
  "knowledge_decay_per_generation": 0.15,
  "education_transmission_per_tier": 0.08,
  "train_knowledge_gain": 0.1,
  "train_cost_credits": 600,
  "agriculture_food_bonus_per_tier": 0.05
}
```

### 2. Subsystem + knowledge state (`src/state/sim.rs` or `src/state/sim/subsystems.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemState {
    pub tier: u32,          // 0..=3
    pub condition: f32,     // 0-1
    pub knowledge: f32,     // 0-1 institutional knowledge for THIS subsystem
}
```

- `SimState` gains `pub subsystems: HashMap<String, SubsystemState>` (one entry per
  catalog id, created at `new_campaign` with tier 0, condition 1.0,
  `knowledge_start`). Iterate **sorted ids** whenever order matters (determinism —
  see `GameData::sorted_ids`).
- Year-boundary tick additions:
  - each subsystem: `condition -= decay_per_year` (same maintained/relief rule as hull);
  - **generation boundary:** each subsystem's knowledge changes by
    `-knowledge_decay_per_generation + education_tier × education_transmission_per_tier`,
    clamped 0–1. (Education tier = `subsystems["education_culture"].tier`.) This is the
    "knowledge dies with people / education transmits it" loop.
  - agriculture food bonus and life-support decay reduction applied here too.

### 3. Verbs (dispatch in `src/game/actions.rs`, new `src/simulation/subsystems.rs`)

- `UiAction::RepairSubsystem(String)` — underway or port. Requires
  `knowledge >= repair_knowledge_required`, else fail with
  "No one aboard remembers how to mend the {name}." Costs parts + minerals; restores
  condition by `repair.field_gain` up to `repair.field_ceiling` underway, to 1.0 in
  port (mirror the existing hull field/port split).
- `UiAction::UpgradeSubsystem(String)` — **port only** (same gate as loadout). Pays the
  next tier's `cost`; logs "The {name} is rebuilt stronger." This is the reactive-upgrade
  loop's verb.
- `UiAction::TrainSubsystemKnowledge(String)` — anytime; costs `train_cost_credits`,
  raises that subsystem's knowledge by `train_knowledge_gain` (cap 1.0). The mid-voyage
  recovery path when knowledge collapsed.

### 4. Event integration (`src/data/events.rs`, `src/simulation/event_resolver.rs`)

- `EventTemplate` gains `#[serde(default)] pub family: String` **now** (W6 fills
  content; empty = untagged).
- Roll weighting: if a template's `family` matches a subsystem's `buffers_family`,
  multiply its weight by the current tier's `weight_multiplier` scaled by condition:
  `effective = 1.0 - (1.0 - weight_multiplier) × condition`.
- Outcome application: for a matching family, scale every **negative** component of
  `resource_delta`/`ship_delta`/`population_delta` by
  `1.0 - severity_reduction × condition` (positive components untouched). Implement in
  one helper used by both `apply_outcome` and `auto_resolve`.

### 5. UI

New `src/ui/subsystems.rs` panel (or extend the Ship Builder screen, whichever fits the
existing screen enum better — follow `src/ui/ship_builder.rs` patterns): per subsystem
show tier pips, condition bar, knowledge bar, and the three verbs (Repair / Upgrade
(port) / Train), disabled states with reasons. Pure view + `UiAction`s.

### 6. Tests

- Data test: six subsystems load; every non-empty `buffers_family` matches the W6 family
  list (hardcode the expected string set in the test); tier costs positive; tiers len 3.
- Decay: condition falls yearly; knowledge falls per generation and is offset by
  education tier.
- Repair gating: knowledge below threshold → error, no cost charged; above → works.
- Severity buffering: fixed event + tier 2 medical bay at full condition → negative
  deltas scaled exactly by `1 - severity_reduction`; positive deltas untouched.
- Upgrade: port-only, charges cost, tier caps at 3.
- Autoplay: policy repairs/trains when cheap and needed; soak invariants extended —
  all subsystem conditions/knowledge within 0–1 forever.

## Acceptance criteria

- All verification commands green.
- A campaign where every expert generation dies untrained (education tier 0, no
  training) ends with an unrepairable subsystem — verified by a test.
- Upgrading the medical bay measurably reduces a biology event's damage in a test.
- All six subsystems authored in JSON; no subsystem constants in Rust.

## Ground rules

1. Data-driven: catalog + tunables in `assets/*.json`.
2. 800-line hard limit; new logic goes in `src/simulation/subsystems.rs` and
   `src/ui/subsystems.rs`, not into `tick.rs`/`actions.rs` bloat — keep dispatch arms
   thin.
3. UI is a pure view; verbs emit `UiAction` only.
4. Determinism: iterate subsystems in sorted-id order anywhere results feed RNG or logs.
5. Old saves abandoned; no migration shims.
6. Delete unused code outright.

## Verification

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo check --release --target wasm32-unknown-unknown
```
