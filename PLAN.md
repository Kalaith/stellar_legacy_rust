# Stellar Legacy — Framework Handoff Plan

*Written 2026-07-18 after the initial framework build. Read `gdd.md` first — this
document maps the GDD onto what already exists in code and what the next agent
should build, in order.*

## Current status: framework complete, M1 mostly proven

The project is a fully compiling, tested skeleton with the GDD §11 architecture in
place. Verified: `cargo test` (18 tests green), `cargo clippy --all-targets
--all-features -- -D warnings` (clean), `cargo fmt` (applied), WASM target checks
(`cargo check --release --target wasm32-unknown-unknown`), and headless UI captures
for four scenes (`docs/verification/ui_{menu,gameplay,event,dilemma}.png`, regenerate
with `.\scripts\capture_ui.ps1 -Scenes menu,gameplay,event,dilemma`).

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
| Chronicle (§7) | `src/chronicle.rs` + `src/heritage.rs` | **Done** — persistent cross-playthrough contract log + Heritage modifiers (renown → tier → new-campaign bonus) |
| State machine (§11) | `src/state.rs`, `src/game.rs` | **Done** — Menu/Gameplay, explicit `StateTransition`, `UiAction` dispatch via `EventBus` |
| Terminal UI shell (§9) | `src/ui.rs` + `src/ui/*` | **Done as skeleton** — all 6 screens + blocking event modal, amber/green/red phosphor palette |
| Capture harness | `src/main.rs` (`STELLAR_LEGACY` prefix) | **Done** — scenes: `menu`, `gameplay`, `event` |

## What is NOT built yet (the next agent's work)

Ordered roughly by milestone (GDD §13):

### Finish M1 → M2 (playable prototype)

1. ~~**Legacy dilemmas are loaded but never fire.**~~ **DONE (2026-07-18).**
   Dilemmas roll on generation boundaries (`simulation/legacy.rs::roll_dilemma`,
   wired in `tick.rs`; chance in `game_config.json` →
   `dilemma_chance_per_generation`), always block (never delegated), suppress the
   same year's event roll, and apply `DilemmaEffect` including the legacy counters.
   The §5.5 failure-risk formula lives in `simulation/legacy.rs::failure_risk`
   (thresholds in config → `failure_risk` block; drift/unity threaten all legacies,
   counter terms only their own legacy) and is surfaced with its contributing
   factors on the Crew & Dynasty screen. New capture scene: `dilemma`. Note:
   dilemma content is still 1 per legacy — M3 target is 6 per legacy (§8).
2. ~~**Crew management.**~~ **DONE (2026-07-18).** One post per archetype:
   `SimState.crew` roster, `simulation/crew.rs` (recruit/train verbs, costs in
   config → `crew` block), crew age out on generation boundaries. Skill effects
   are data-driven on `crew_archetypes.json`: `production_per_skill` multipliers
   (applied in the tick), medic `famine_loss_reduction_per_skill`, security-chief
   `unity_recovery_per_skill` (below a config ceiling). `SelectHeir` designates a
   successor stored on `Dynasty.designated_heir`, honored by succession over the
   best-leadership fallback. Crew UI lives on the Crew & Dynasty screen (`crew`
   capture scene). Event-outcome hooks (navigator/combat) intentionally deferred
   to item 3 where ship component stats land.
3. ~~**Production bonuses.**~~ **DONE (2026-07-18/19).** `simulation/ship.rs`
   aggregates the installed hull+engine+
   weapon `ComponentStats` (`loadout_stats`) and applies yearly effects in the
   tick (`apply_loadout_effects`, after base production): speed → credits,
   cargo → minerals, fuel_regen → ship fuel, scaled by a new
   `game_config.json → ship` block (`ShipConfig`). The Ship Builder now shows a
   per-component stat readout (`CARGO/CREW/SPD/CBT/FUEL`), so loadouts visibly
   differ; new `ship` capture scene. **speed → contract progress DONE
   (2026-07-18)** — `ActiveContract.bonus_progress` (serde default) accrues
   `speed × ship.contract_progress_per_speed` each year in `advance_contract`
   (speed passed from the tick via `loadout_stats`); `progress()` counts it, so
   milestones/mission-completion score higher without shortening the duration.
   Surfaced as a `DRIVE ASSIST: +N yr` line on the active-contract screen; new
   `contract_active` capture scene; unit-tested. **cargo → market lot size DONE
   (2026-07-18)** — the Market's buy/sell lot is now the ship's aggregate cargo
   (`loadout_stats(...).cargo`, min 50) instead of a fixed 100, shown as
   `HOLD N (lot size)`; bigger hulls trade bigger lots (the buy/sell verbs
   already took an amount, so no sim change). New `market` capture scene.
   **combat → wanderer-dilemma odds DONE (2026-07-18)** —
   `legacy::dilemma_odds(sim, data, base)` adds `combat ×
   ship.combat_dilemma_odds_per_point` on Wanderer dilemmas (capped by
   `dilemma_odds_cap`); used both for the `resolve_dilemma` roll and shown
   honestly in the modal as `Success odds: N% (combat +M%)` (Pillar 3). New
   `dilemma_combat` capture scene; unit-tested (lifts Wanderer odds only,
   respects the cap). **contract-milestone deltas DONE (2026-07-19)** —
   `MilestoneDef`/`MilestoneState` carry an optional `reward: ResourceDelta`
   (serde default) applied once when the milestone is first reached
   (`advance_contract` collects rewards then applies them after the contract
   borrow ends); several `assets/contracts.json` milestones now grant
   intermediate payoffs, shown as `(+N res)` on the active-contract milestone
   list. Unit-tested (lands once, no repeat). **Item 3 complete.**
4. ~~**Game-over / retirement flow.**~~ **DONE (2026-07-18).** Dynasty
   extinction now triggers a full-screen `VOYAGE TERMINATED` terminal takeover
   (`src/ui/game_over.rs`, intercepts `draw_gameplay` before header/tabs) with a
   playthrough summary readout (years, generations, final population, tradition,
   contracts logged for the legacy, last commander) and a blinking
   `> RETIRE VOYAGE` prompt. `UiAction::RetireVoyage` clears the dead campaign
   save (no autosave) and returns to the menu; the Chronicle persists
   separately. New `gameover` capture scene; the dashboard's old inline extinct
   message was removed (now unreachable). Heritage modifiers from the retired
   run remain item 7.
5. ~~**Event content.**~~ **DONE (2026-07-18).** `assets/events.json` now holds
   the M2 target of 12 templates, 3 per category (added `micrometeoroid_storm`,
   `coolant_breach`, `skills_drought`, `youth_unrest`, `halfway_beacon`,
   `derelict_encounter`, `founders_creed`, `the_naming`). Pure content — the
   resolver was unchanged. Several outcomes record `long_term_consequences`
   (`scarred_reactor`, `lost_craft`, `banked_resentment`, `grave_robbed`,
   `revised_charter`) feeding the Pillar-2 consequence log; `legacy_weight_modifiers`
   bias templates toward the fitting legacy. `event_categories_all_represented`
   now asserts 12 total / ≥3 per category. (M3 target is 30+, §8.)
6. ~~**Contract content**~~ **DONE (2026-07-18).** Added the two missing
   prototype charters to `assets/contracts.json` — `veiled_expanse_survey`
   (exploration) and `tarssen_relief` (rescue) — so all four §8 objectives
   (mining, colonization, exploration, rescue) are covered. Both follow the
   schema (milestones, four success metrics summing to 1.0, failure_risks,
   reward) and surface automatically on the Contract screen's available-charters
   list. New `contracts` capture scene. (M3 target is 6-8 total, §8.)

### M3 (content-complete)

7. ~~**Heritage modifiers** (§7)~~ **DONE (2026-07-18).** `src/heritage.rs`
   derives a *renown* total from the `ChronicleStore` (each completed contract's
   success score ×100) and places a new dynasty in a heritage tier
   (`game_config.json → heritage`: Founding / Remembered / Storied / Renowned /
   Mythic — base + 4 bonus tiers, §8) granting starting credits/influence/
   tradition. Applied in the `NewCampaign` transition (`heritage::apply`, with a
   founding log line) and surfaced on the menu ("HERITAGE: {tier} · renown N ·
   +cr/+inf/+tradition"). Deterministic (derived from the persisted Chronicle,
   applied once at creation). New `heritage` capture scene; 3 unit tests. This
   closes the Chronicle "Partial" status.
8. ~~**Content targets from §8**~~ **DONE (2026-07-19)** — all §8 bulk-content
   targets met. **30+ events DONE** (**30 total**, 7-8 per category; the
   final pass added `reactor_scram`, `pirate_shadow`, `scrubber_failure`,
   `idle_generation`, `apprentice_glut`, `slingshot`, `dark_stretch`,
   `resupply_cache`, `native_leader`, `memory_wall`; data-load test asserts ≥30
   / ≥7 per category), **5/5/5
   components DONE (2026-07-19)** — added `habitat_ring`/`armored_prow` (hulls),
   `solar_sail`/`warp_coil` (engines), `flak_screen`/`spinal_railgun` (weapons),
   each with real stat tradeoffs that feed the item-3 hooks; the Ship Builder
   card was compacted (96px, cost folded into the button) so a five-deep column
   fits, and the data-load test asserts 5/5/5. **6-8 contracts DONE
   (2026-07-19)** — added `coronal_tap` (mining) and `seedfall` (colonization)
   for **6** total (in the §8 6-8 band), each with milestone rewards; the
   available-charters card was compacted (78px) so 6-8 fit, data-load test
   asserts ≥6. **6 dilemmas per legacy DONE (2026-07-19)** — three per legacy
   (2026-07-19 pass) plus three more this pass (Preservers `calendar_reform`/
   `bloodline_registry`/`relic_engine`; Adaptors `neural_lace`/`ageless_trial`/
   `hive_protocol`; Wanderers `the_splinter`/`smugglers_offer`/
   `flag_of_convenience`) for **6 each**; each engages its legacy's tracked
   counter (tradition / body-horror+dread / piracy-reputation); data-load test
   asserts ≥6 per legacy. **doubled name pools DONE
   (2026-07-19)** — `assets/dynasty_names.json` given names 25→50, surnames
   10→20 per legacy, specializations 10→20, traits 5→10 per legacy (matched to
   each legacy's flavor); data-load test asserts the doubled counts. Pure
   `assets/*.json`; schemas unchanged. **Item 8 complete — all §8 content
   targets met (30 events / 5-5-5 components / 6 contracts / 6 dilemmas per
   legacy / doubled name pools).**
9. **Terminal polish**: monospace bitmap font (default font is close but not
   monospace), ~~flicker fx~~ **CRT overlay DONE (2026-07-18)** —
   `macroquad_toolkit::fx::CrtOverlay`/`CrtStyle` (new toolkit module
   `fx/crt.rs`): scanlines + corner vignette + slow rolling refresh band +
   subtle flicker, drawn screen-space at the end of `Game::draw`, amber preset,
   F10 toggle. ~~typewriter text reveal~~ **DONE (2026-07-18)** — toolkit
   `fx/typewriter.rs` (`typed_prefix`/`typed_char_count`/`is_fully_typed`,
   pure + tested); modal body text streams in at `REVEAL_CPS` with a blinking
   underscore cursor (`event_modal::draw_typed_block`). A cosmetic reveal clock
   lives on `Game` (`modal_reveal`, reset per modal, instant in capture).
   ~~power-on boot sequence~~ **DONE (2026-07-18)** — `src/boot.rs`
   (`BootScreen`) streams a terminal POST log once before the menu on launch
   (amber banner + green status lines, blinking cursor, ~2.5s, any input skips);
   frozen-frame `boot` capture scene added. ~~phosphor text glow~~ **DONE
   (2026-07-18)** — toolkit `ui::draw_text_glow` (dim offset copies fanned to a
   radius + crisp foreground) gives bright headings a CRT bloom; applied to the
   menu title, the gameplay header game-name, and the boot banner (subtle
   alphas so body text stays crisp). ~~settings screen (CRT toggle)~~ **DONE
   (2026-07-18)** — F1 opens a `DISPLAY // CRT MONITOR` overlay
   (`src/ui/settings.rs`) with toggles for CRT effect / scanlines / flicker and
   an amber↔green phosphor choice; prefs persist under their own `display`
   key (`src/settings.rs::DisplaySettings`, loaded at startup, saved on change,
   separate from the sim save so determinism is untouched). F10 still hard-
   toggles the effect. New `settings` capture scene. ~~phosphor recolor~~ **DONE
   (2026-07-18)** — the `term` palette is now runtime phosphor-aware: every hue
   is a `fn` reading a thread-local tube (`term::set_phosphor`), so choosing
   GREEN recolors the *entire* monochrome UI (text, borders, panels, surface
   fills), not just the overlay tint; alerts stay warm-red on both tubes. New
   `green` capture scene. ~~`catalog_thumbnail.png`~~ **DONE (2026-07-18)** —
   root 16:9 title capture from the menu scene. ~~screen curvature~~ **DONE
   (2026-07-18)** — `CrtOverlay` now masks rounded tube-glass corners
   (`CrtStyle::corner_radius`/`bezel`, baked corner texture flipped per corner,
   drawn last so the bezel clips every layer); presets ship a 26px radius. UI
   content is inset so nothing clips. ~~ship's-log streaming~~ **DONE
   (2026-07-18)** — the newest ship's-log line types in like live console
   output (`dashboard::draw_log_panel`, `LOG_CPS`, blinking cursor), driven by a
   `Game::log_reveal` clock that resets when the log grows (instant in capture);
   frozen `log` capture scene added. **Monospace font DEFERRED** —
   the toolkit font API (`set_default_ui_font_from_bytes`) is ready, but no
   monospace TTF is bundled in the repo (only proportional Rajdhani/DejaVuSans);
   sourcing one is a licensing decision for a human. ~~delegation defaults in
   the settings overlay~~ **DONE (2026-07-19)** — the F1 `DISPLAY // CRT
   MONITOR` overlay gained a `DELEGATION DEFAULTS // NEW VOYAGES` section with a
   per-category COUNCIL/DELEGATED toggle; the choice persists under a
   `delegation` key (`settings::load_delegation`/`save_delegation` over the
   sim's `DelegationSettings`) and is applied to `sim.delegation` in the
   `NewCampaign` transition. Panel grown to fit; `settings` capture updated.
   Still to do: ko-fi/index.html screenshots.
10. ~~Consider `achievements` for Chronicle milestones (GDD §10 "maybe").~~
    **DONE (2026-07-19).** `src/achievements.rs` defines six milestones (first
    charter, flawless voyage, full registry, fifth generation, year 100, 250
    renown) over the toolkit `achievements` registry; `evaluate(sim, chronicle)`
    derives unlocks purely from post-state, so `Game::check_achievements` (called
    after each year and on campaign transitions) unlocks + notifies once each and
    persists under its own `achievements` key (cosmetic — no sim effect). Shown
    as a `MILESTONES` panel on the Chronicle screen (unlocked N/M + `[x]/[ ]`
    list). New `chronicle` capture scene; 3 unit tests.

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

- ~~Meter color logic treats <35% as "critical" red — inverted for
  `cultural_drift`/`adaptation`~~ **FIXED (2026-07-18)** — `term_meter_toned`
  takes a `MeterTone` (`LowCritical`/`HighCritical`/`Neutral`); the dashboard
  bars tag adaptation `Neutral` and cultural drift `HighCritical`.
- ~~The event modal header band is empty~~ **FIXED (2026-07-18)** — the
  category/legacy line now renders centered in the header band; the title leads
  the body.
- Menu lists legacies in sorted-id order (Adaptors first); GDD implies no order,
  but Preservers-first might read better.
