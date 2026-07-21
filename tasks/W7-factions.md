# W7 — Founding factions (Frostpunk-style population groups)

**Prerequisite: W2 complete and green.**

## Goal

Replace the pure-RNG founding roll with **factions**: the game ships **6 authored
factions** across a tech-embracing ↔ tech-averse spectrum; the player **picks 3** at new
game. Factions can be **lost mid-voyage** (wiped out, settled off-ship, departed,
assimilated); returning with fewer than 3 lets the player **recruit another group** in
drydock. v1 is **structure first**: factions are data + population segments + loss/
recruit + event/log coloring — **no approval meters, no stat modifiers** (those layer on
later; leave schema room).

## Binding owner decisions

- 6 authored factions, player chooses 3; "if they return with only 2 they can get
  another group."
- Loss paths (all in play): all members die; faction settles off-ship; schism/departure;
  soft assimilation via generational drift.
- v1 depth: structure only. No inter-faction tension mechanics yet.
- Data-driven: faction identities live in `assets/factions.json`, never in Rust.
- The existing **legacy** system (preservers/adaptors/wanderers) is unrelated and
  unchanged — factions are population segments *within* a campaign, legacies remain the
  campaign-level identity. Do not merge or confuse them.

## Current state (verified facts)

- New campaign: `SimState::new_campaign(data, legacy_id, seed)` (`src/state/sim.rs`) —
  population is one aggregate `PopulationState { count: 1000, ... }`; the founding
  dynasty is rolled by `founding_dynasty(...)` (private fn, same file). Dynasty stays
  as-is — it is the *leadership line*, orthogonal to factions.
- Menu flow: `UiAction::StartNewGame` → `StateTransition::NewCampaign { legacy_id, seed }`
  (`src/state.rs:22`, dispatch in `src/game/actions.rs`). Menu state:
  `src/state/menu.rs`, menu UI in `src/ui.rs` / dashboard files.
- Data loading: `GameData::load()` in `src/data.rs` — embedded JSON via `include_str!` +
  `DataRegistry`/`load_embedded_json_labeled`. Follow this exact pattern.
- Config struct `GameConfig` in `src/data.rs:81`.
- Drydock/port = `sim.contract.is_none()` (see the port gate in
  `Game::purchase_component`, `src/game/actions.rs`).

## Changes

### 1. Faction data (`assets/factions.json`, `src/data/factions.rs`)

New file `src/data/factions.rs` (register `pub mod factions;` in `src/data.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    /// -1.0 (tech-averse) .. +1.0 (tech-embracing). Unused mechanically in v1;
    /// reserved for later modifiers.
    pub ideology: f32,
    pub description: String,
    /// Short phrase used in logs, e.g. "the Verdant Kin".
    pub log_name: String,
}
```

`assets/factions.json` — author these 6 (flavor is placeholder; owner may rename later):

| id | name | ideology |
| --- | --- | --- |
| `ascension_circle` | The Ascension Circle | 0.9 |
| `steel_covenant` | The Steel Covenant | 0.5 |
| `meridian_accord` | The Meridian Accord | 0.0 |
| `hearth_union` | The Hearth Union | -0.3 |
| `verdant_kin` | The Verdant Kin | -0.6 |
| `first_flame` | Keepers of the First Flame | -0.9 |

Write a one-to-two-sentence `description` each, matching the game's register (see
`assets/contracts.json` prose). Load into `GameData` as
`pub factions: DataRegistry<FactionDef>` (embedded via `include_str!`, keyed on `"id"`).

Add a `GameConfig` block + JSON:

```json
"factions": {
  "starting_count": 3,
  "assimilation_share_threshold": 0.05,
  "assimilation_drift_threshold": 0.7,
  "recruit_group_cost_credits": 2500,
  "recruit_group_size": 300
}
```

### 2. Faction state (`src/state/sim.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactionStatus { Aboard, WipedOut, Settled, Departed, Assimilated }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactionState {
    pub faction_id: String,
    pub members: u32,
    pub status: FactionStatus,
}
```

- `SimState` gains `pub factions: Vec<FactionState>`.
- `new_campaign` signature gains `faction_ids: &[String]` (exactly
  `config.factions.starting_count` entries; the caller guarantees it). Starting
  population splits as evenly as possible across the chosen factions (remainder to the
  first). Log the founding: "Three peoples board together: …".
- Invariant: `sum(members of Aboard factions) == population.count`. Add
  `pub fn rebalance_factions(&mut self)` that proportionally rescales Aboard members to
  the current `population.count` (largest-remainder rounding). Call it at the end of
  every year-boundary tick so uniform growth/deaths keep shares stable. Deaths are
  uniform across factions in v1.

### 3. Loss conditions

- **WipedOut:** during `rebalance_factions`, a faction reaching 0 members while others
  survive → status `WipedOut`, log "The last of {log_name} is gone."
- **Settled / Departed:** `EventOutcome` (`src/data/events.rs`) gains
  `#[serde(default)] pub faction_loss: Option<FactionLossKind>` where
  `FactionLossKind { Settled, Departed }` (serde snake_case). Applying such an outcome
  removes the **smallest Aboard faction** (deterministic tie-break: lexicographic id):
  its members leave `population.count`, status set accordingly, dated log line. If only
  one faction is Aboard, the outcome's faction_loss is skipped (log a near-miss line
  instead) — the ship never loses its last people this way.
- **Assimilated:** on each **generation boundary** (inside the existing generation block
  in the year tick), any Aboard faction whose share `< assimilation_share_threshold`
  while `population.cultural_drift > assimilation_drift_threshold` folds into the
  largest faction: members transfer, status `Assimilated`, log "The children of
  {log_name} now answer to another name."
- Give at least one existing event template a `faction_loss: settled` outcome and one a
  `departed` outcome where it fits narratively (e.g. a garden-world or schism-flavored
  event). Content depth beyond that is W6's job.

### 4. New-game picker + drydock recruit

- `StateTransition::NewCampaign` gains `faction_ids: Vec<String>`.
- Menu (`src/state/menu.rs` + menu UI): after legacy selection, a faction picker —
  toggle 6 rows, exactly 3 must be selected before START enables. New
  `UiAction::ToggleFaction(String)`. Keep it in the existing menu screen style.
- Drydock: when in port (`contract.is_none()`) and Aboard factions
  `< starting_count`, the crew/dynasty screen offers **Recruit a people**:
  `UiAction::RecruitFactionGroup(String)` — pool = factions never yet part of this
  campaign (not chosen at founding, not lost). Costs
  `recruit_group_cost_credits`; adds a new `FactionState` with
  `recruit_group_size` members (added to `population.count`). Lost factions never
  return.

### 5. UI surfacing (read-only)

On the crew/dynasty screen, list factions: name, members, share %, status. Lost
factions show status + strikethrough-style dimming per existing UI conventions. No
other mechanics surface in v1.

### 6. Tests

- Data test: exactly 6 factions load; ideology within [-1, 1]; ids unique.
- `new_campaign` with 3 faction ids: members sum to `population.count`; deterministic
  per seed.
- Rebalance: after halving `population.count`, shares preserved, sum invariant holds.
- Assimilation: construct a sim with a 4% faction + drift 0.8 → folds on the generation
  boundary; with drift 0.3 → does not.
- `faction_loss` outcome: smallest faction removed, population reduced, status correct;
  skipped when only one faction remains.
- Recruit: only in port, only when short, only from the untouched pool; costs charged.
- Autoplay soak: extend invariants — faction member sum always equals population count;
  statuses never regress from a lost state to Aboard.

## Acceptance criteria

- All verification commands green.
- New game requires choosing exactly 3 of 6 factions; campaign is deterministic per
  (seed, legacy, faction set).
- All four loss paths reachable in tests; the last people are never lost to
  `faction_loss` (extinction remains the succession system's job).
- No faction names or balance numbers in Rust source.

## Ground rules

1. Data-driven: identities in `assets/factions.json`, tunables in `game_config.json`.
2. 800-line hard limit; `src/state/sim.rs` is at ~570 — extract
   `src/state/sim/factions.rs` (as `sim/` children of `sim.rs`) if it would cross 600.
3. UI is a pure view; picker and recruit emit `UiAction` only.
4. Determinism: randomness only through `sim.rng`; deterministic tie-breaks as specified.
5. Old saves abandoned; no migration shims.
6. Delete unused code outright.

## Verification

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo check --release --target wasm32-unknown-unknown
```
