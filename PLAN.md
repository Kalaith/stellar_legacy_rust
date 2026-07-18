# Stellar Legacy — Framework Handoff Plan

*Written 2026-07-18 after the initial framework build. Read `gdd.md` first — this
document maps the GDD onto what already exists in code and what the next agent
should build, in order.*

## Current status: framework complete, M1 mostly proven

The project is a fully compiling, tested skeleton with the GDD §11 architecture in
place. Verified: `cargo test` (18 tests green), `cargo clippy --all-targets
--all-features -- -D warnings` (clean), `cargo fmt` (applied), WASM target checks
(`cargo check --release --target wasm32-unknown-unknown`), and headless UI captures
for three scenes (`docs/verification/ui_{menu,gameplay,event}.png`, regenerate with
`.\scripts\capture_ui.ps1 -Scenes menu,gameplay,event`).

A campaign is already playable end-to-end in skeleton form: pick a legacy → accept a
charter → advance years → resolve council events → generations turn over → contract
completes → Chronicle entry recorded and persisted across saves.

## What is implemented (and where)

| System (GDD ref) | Module | State |
| --- | --- | --- |
| Data loading, all `assets/*.json` (§6) | `src/data.rs` + `src/data/*` | **Done** — serde types, embedded via `include_str!`, load-tested |
| Sim state, campaign creation (§5.1) | `src/state/sim.rs` | **Done** — serializable, deterministic per seed, serde round-trip tested |
| Yearly tick (§3, §5.1) | `src/simulation/tick.rs` | **Done** — production, food upkeep, ship wear, generation trigger, contract progress, market drift, event roll; determinism tested over decades |
| Succession (§5.3) | `src/simulation/succession.rs` | **Done** — 25-year aging, retirement at 70, best-heir 30-50 selection, 1-3 births, elder mortality (extension: needed so extinction is reachable), extinction flag |
| Contract scoring (§5.2) | `src/simulation/contract.rs` | **Done** — exact GDD formula + bands, tested; milestone/metric tracking each year |
| Event roll/scoring/resolution (§5.4) | `src/simulation/event_resolver.rs` | **Done** — chance formula (capped), distress-scaled category weights, legacy-weighted template pick, outcome auto-scoring, delegation-aware resolution |
| Market (§5.1) | `src/simulation/market.rs` | **Done** — buy/sell validation, bounded yearly price walk |
| Save/load (§7) | `src/save.rs` | **Done** — toolkit slots, migration hook stubbed for future versions |
| Chronicle (§7) | `src/chronicle.rs` | **Partial** — persistent cross-playthrough contract log works; **Heritage modifiers not started** |
| State machine (§11) | `src/state.rs`, `src/game.rs` | **Done** — Menu/Gameplay, explicit `StateTransition`, `UiAction` dispatch via `EventBus` |
| Terminal UI shell (§9) | `src/ui.rs` + `src/ui/*` | **Done as skeleton** — all 6 screens + blocking event modal, amber/green/red phosphor palette |
| Capture harness | `src/main.rs` (`STELLAR_LEGACY` prefix) | **Done** — scenes: `menu`, `gameplay`, `event` |

## What is NOT built yet (the next agent's work)

Ordered roughly by milestone (GDD §13):

### Finish M1 → M2 (playable prototype)

1. **Legacy dilemmas are loaded but never fire.** `assets/legacies.json` ships one
   dilemma per legacy with full success/failure effects, and `LegacyTrack` holds the
   real counters (`tradition_points`, `body_horror_events`, `piracy_reputation`,
   `existential_dread`) — but nothing rolls a dilemma yet. Wire them into the tick
   (e.g. as part of `legacy_moment` events or on generation boundaries) and apply
   `DilemmaEffect` including the legacy counters. Then implement the **failure-risk
   formula** (§5.5: drift > 0.7 → +30, unity < 0.3 → +25, tradition < 20 → +35,
   at-risk > 50) and surface it on the Crew & Dynasty screen.
2. **Crew management.** `crew_archetypes.json` is loaded and displayed as
   placeholder text. Add a crew roster to `SimState`, recruit/train actions
   (`UiAction` variants exist as a pattern; add `SelectHeir` etc.), and let crew
   skills modify production/event outcomes.
3. **Production bonuses.** `ProductionRates` is pre-keyed (the GDD §5.1 bug fix),
   but nothing modifies it yet — ship components and contract milestones should
   grant production deltas. The Ship Builder purchase currently only swaps the
   loadout id; make component stats matter (speed → contract progress, cargo →
   market lots, combat → wanderer dilemma odds).
4. **Game-over / retirement flow.** Dynasty extinction sets a flag and blocks
   time; the Chronicle screen should take over with a playthrough summary and a
   "retire voyage" path (GDD §7 progression table).
5. **Event content.** 4 templates exist (original parity). M2 target is 12 across
   the 4 categories (§8). Pure `assets/events.json` work; the resolver needs no
   changes. Use `consequences` (already tracked on the sim) to gate/weight
   follow-up events — that's the intended payoff of Pillar 2.
6. **Contract content**: 2 templates exist; add exploration + rescue (§8: 4 total
   for prototype).

### M3 (content-complete)

7. **Heritage modifiers** (§7): on retirement/extinction, derive small starting
   bonuses from `ChronicleStore` entries for the next campaign; 4 tiers (§8).
8. Content targets from §8: 30+ events, 5/5/5 components, 6-8 contracts, 6 dilemmas
   per legacy, doubled name pools.
9. **Terminal polish**: monospace bitmap font (default font is close but not
   monospace), typewriter/flicker fx (toolkit `fx`, keep subtle), settings screen
   (toolkit `settings`) for delegation defaults, `catalog_thumbnail.png` from the
   menu capture, ko-fi/index.html screenshots.
10. Consider `achievements` for Chronicle milestones (GDD §10 "maybe").

## Conventions the framework already follows (keep them)

- **Determinism discipline (§5.6):** all gameplay randomness goes through
  `sim.rng` (`SeededRng`, serialized in the save). Never use `macroquad::rand`
  or toolkit free-function rng in the sim. `DataRegistry` is hash-map backed —
  **sort ids** (`GameData::sorted_ids`) before any RNG-driven or displayed
  iteration (see `event_resolver::roll_event`).
- **UI is a pure view layer:** panels read `&SimState` and push `UiAction`; all
  mutation lives in `game.rs` / `simulation/*`. Add a variant to `UiAction` for
  any new interaction.
- **Data-driven:** balance/content changes belong in `assets/*.json`, not Rust
  constants. Tunables live in `assets/data/game_config.json` → `GameConfig`.
- **Time only moves on `AdvanceYear`** (Pillar 4). A pending event blocks the
  tick (`debug_assert` in `tick.rs`); keep that invariant.
- No `mod.rs`, 800-line hard cap per file (everything is currently well under),
  no `_`-prefixed dead code — the two intentionally-idle fields
  (`Game::_assets`, texture manifest) are wired through the toolkit loader
  instead.

## Build / verify loop

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
.\scripts\capture_ui.ps1 -Scenes menu,gameplay,event   # visual check
.\publish.ps1                                           # full validation + deploy
```

Note: the repo-root `Cargo.toml` workspace glob requires every top-level dir to be
a crate or excluded; `dragons_den` (gdd-only) was added to the exclude list during
this setup. `stellar_legacy` is not yet a git repository — run `git init` + initial
commit before starting M2 work (every sibling game is its own repo).

## Known cosmetic nits (fine to fix opportunistically)

- Meter color logic treats <35% as "critical" red — correct for morale/hull, but
  inverted for `cultural_drift`/`adaptation` where low is good/neutral.
- The event modal header band is empty (title is drawn in the body).
- Menu lists legacies in sorted-id order (Adaptors first); GDD implies no order,
  but Preservers-first might read better.
