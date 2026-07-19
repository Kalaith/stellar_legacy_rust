# Stellar Legacy — Framework Handoff Plan

*Written 2026-07-18 after the initial framework build. Read `gdd.md` first — this
document maps the GDD onto what already exists in code and what the next agent
should build, in order.*

## Current status: M1–M4 COMPLETE (all M4.1–M4.8 built); only the ~30-min-floor playtest tuning is human-gated

*Status refreshed 2026-07-19 (this doc opens with the original 2026-07-18 framework
snapshot; the numbered log below records what shipped since).* Every numbered item
(1–10, the original M1–M3 scope) and all three cosmetic nits are **done**; the game is
content-complete against GDD §8 (and past several minimums) — 46 events, 5/5/5
components, 10 contracts (across a 0/100/250/400 renown ladder), 8 dilemmas per
legacy, doubled name pools.

**New direction (owner-directed 2026-07-19):** a **Voyage-and-Return Refit Loop** —
one *run* = one mission flown by a persistent ship that leaves fresh and hopeful and
arrives back worn and changed, with a between-missions **drydock** economy (repair /
upgrade / commission a new ship) funding the next run. Paced to **30–60 min** (soft cap
~1 hr, ~30 min floor; a run only ends sooner via game-over). On success the people/dynasty
**carry across** to keep building the legacy; only **game-over (dynasty extinction)** resets
to a new ship and new people. Design in `gdd.md §3.1`; the code-grounded build order is
**M4** below. This is now the active work; the ko-fi/index.html marketing screenshots
(item 9) remain the only human-gated leftover from M3.

Verified: `cargo test` (**57 tests green**, incl. a 250-year soak/integration
test), `cargo clippy --all-targets --all-features -- -D warnings` (clean), `cargo
fmt` (applied), WASM target checks (`cargo check --release --target
wasm32-unknown-unknown`), and headless UI captures for ~20 scenes under
`docs/verification/` (menu, gameplay, event, dilemma, dilemma_combat, crew, ship,
market, contracts, contract_active, drydock, boot, settings, help, green, log,
heritage, chronicle, gameover), regenerated with `.\scripts\capture_ui.ps1 -Scenes <list>`.

A campaign plays end to end: pick a legacy → accept a charter → advance years →
resolve council events and legacy dilemmas → generations turn over → contract
completes → Chronicle entry recorded and Heritage carried to the next voyage. The
UI is a full old-CRT terminal (scanlines, vignette, rolling refresh band, flicker,
rounded tube-glass corners, phosphor bloom, a power-on POST, live-streaming log,
amber/green phosphor tubes) and is keyboard-first throughout (F2 lists the keys).

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
| Terminal UI shell (§9) | `src/ui.rs` + `src/ui/*` | **Done** — all 6 screens + blocking event/dilemma modals + game-over takeover; full CRT terminal (overlay, phosphor glow, typewriter reveal, boot POST, streaming log, runtime amber/green phosphor tubes); keyboard-first with F1 settings / F2 help |
| Capture harness | `src/main.rs` (`STELLAR_LEGACY` prefix) | **Done** — ~21 scenes (`menu`, `gameplay`, `event`, `dilemma`, `dilemma_combat`, `crew`, `ship`, `market`, `contracts`, `contract_active`, `drydock`, `boot`, `settings`, `help`, `green`, `log`, `heritage`, `chronicle`, `gameover`) |

## Milestone work — progress log (all items complete)

Ordered roughly by milestone (GDD §13). Every numbered item below is **done**;
the entries record what shipped and where. The only outstanding thread is the
human-gated ko-fi/index.html marketing screenshots noted under item 9.

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
   targets met, several exceeded. **30+ events DONE** (**46 total**, 11-12 per
   category; passed the §8 minimum of 30 across successive depth passes —
   `debris_lattice`/`power_cascade`/`the_prodigy`/`aging_infrastructure`/
   `the_named_star`/`course_ratification`/`returning_signal`/`last_photograph`,
   then `coolant_freeze`/`water_bloom`/`the_slowdown`/`berth_lottery`/
   `scout_returns`/`midpoint_census`/`founders_ledger`/`the_renaming` — for
   much less repetition across long campaigns; data-load test asserts ≥46 / ≥11
   per category), **5/5/5
   components DONE (2026-07-19)** — added `habitat_ring`/`armored_prow` (hulls),
   `solar_sail`/`warp_coil` (engines), `flak_screen`/`spinal_railgun` (weapons),
   each with real stat tradeoffs that feed the item-3 hooks; the Ship Builder
   card was compacted (96px, cost folded into the button) so a five-deep column
   fits, and the data-load test asserts 5/5/5. **6-8 contracts DONE
   (2026-07-19)** — added `coronal_tap` (mining), `seedfall` (colonization),
   `starfall_beacon` (exploration), and `hollow_fleet` (rescue) for **8 total,
   two per objective (mining/colonization/exploration/rescue)**, each with
   milestone rewards; the available-charters list was converted to a
   **two-column grid** (`contract_systems::draw_available`) so it scales past
   six without a scrollbar (4 rows of 2, with headroom for more), data-load
   test asserts ≥8. **Dilemmas DONE (§8 target 6, since deepened to 8 per legacy)** —
   the §8 pass brought each legacy to 6, then a depth pass added two more each
   (Preservers `forbidden_deck`/`the_apostate`; Adaptors `the_symbiont`/
   `the_backup`; Wanderers `the_amnesty`/`the_prize_court`) for **8 each, 24
   total**; every dilemma engages its legacy's tracked counter (tradition /
   body-horror+dread / piracy-reputation); data-load test asserts ≥8 per
   legacy. **doubled name pools DONE
   (2026-07-19)** — `assets/dynasty_names.json` given names 25→50, surnames
   10→20 per legacy, specializations 10→20, traits 5→10 per legacy (matched to
   each legacy's flavor); data-load test asserts the doubled counts. Pure
   `assets/*.json`; schemas unchanged. **Item 8 complete — all §8 content
   targets met, several since exceeded (46 events / 5-5-5 components /
   8 contracts / 8 dilemmas per legacy / doubled name pools). The
   available-charters list is now a two-column grid, so contracts can grow
   further (the grid has room for several more rows) without more UI work.**
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
   ~~keyboard navigation~~ **DONE (2026-07-19)** — terminals are keyboard-first,
   so every screen is keyboard-navigable: on the **menu**, number keys pick a
   legacy, arrows move the selection, and Enter begins the voyage; in
   **gameplay**, **1-6** switch screen tabs and **Space/Enter** advances the
   year; when a council **event/dilemma modal** is up the number keys select its
   options (`Game::gather_keyboard_actions` + `digit_pressed`, the modal claiming
   the digits; suppressed while the settings/help panel is up). Legacy rows read
   `1 The Preservers` …, tabs `1 DASHBOARD` … `6 CHRONICLE`, the advance button
   `[SPACE]`, the begin button `[ENTER]`, and modal options `[1]`/`[2]` to teach
   the hotkeys. ~~help overlay~~ **DONE (2026-07-19)** — **F2** opens a
   `HELP // CONTROLS` overlay (`src/ui/help.rs`) listing every key; **Esc**
   closes whichever panel is open (help, then settings); F1/F2 are mutually
   exclusive. New `help` capture scene; settings hint updated to
   `F1 panel · F2 help · F10 CRT · ESC closes`. Still to do: ko-fi/index.html
   screenshots (marketing artifact — human-gated).
10. ~~Consider `achievements` for Chronicle milestones (GDD §10 "maybe").~~
    **DONE (2026-07-19).** `src/achievements.rs` defines six milestones (first
    charter, flawless voyage, full registry, fifth generation, year 100, 250
    renown) over the toolkit `achievements` registry; `evaluate(sim, chronicle)`
    derives unlocks purely from post-state, so `Game::check_achievements` (called
    after each year and on campaign transitions) unlocks + notifies once each and
    persists under its own `achievements` key (cosmetic — no sim effect). Shown
    as a `MILESTONES` panel on the Chronicle screen (unlocked N/M + `[x]/[ ]`
    list). New `chronicle` capture scene; 3 unit tests.

## M4 — The Voyage-and-Return Refit Loop (owner-directed 2026-07-19)

*Design intent in `gdd.md §3.1`. This section was the code-grounded build order; **all
eight items (M4.1–M4.8) are now built and verified.** The loop plays end to end: depart
fresh → the people drift and the ship wears → keep it limping with field repair + scavenged
parts → arrive home worn and changed → in drydock, full-repair / refit / commission a new
ship → accept the next (renown-gated) charter and cast off again, with a run timer gauging
the pace. **The one remaining task is human-gated:** a real playthrough to calibrate the
~30-min floor (watch `RUN`, lengthen missions if under ~30 min) and confirm the M4.1–M4.5
config numbers feel right.*

**Why this is a small build, not a rewrite.** The persistent ship the owner wants already
exists in the code: a contract completes and **auto-continues in the same `SimState`**
(`game.rs:853-903` clears `sim.contract` and banks `template.reward`, no menu round-trip,
no reset), and the whole `SimState` — ship loadout, population, dynasty — is already
autosaved to `save_slot` (`save.rs`; autosave on `ToMenu` at `game.rs:617-624`). So the
"persistent ship across missions" needs **no new carry channel**, and "in drydock between
missions" is simply the state `sim.contract == None`. Extinction (`dynasty.extinct`,
`game_over.rs`) stays the true game-over. The work is to make the *arc* felt and give the
*port phase* real verbs.

**Run model (owner-confirmed 2026-07-19, gdd.md §12 Q4):** a run = one mission on a
persistent ship. On success the people/dynasty carry across in the same `SimState` and keep
building the legacy; **game-over (dynasty extinction) is the only reset** — it ends the run
early and starts a fresh ship + fresh people. Pacing is a **~30-min floor / ~1-hr soft cap**:
a successful run must not be clearable in under ~30 min (sub-30 only via game-over), so
mission length/decision density (M4.2) must be sized to guarantee the floor.

1. ~~**M4.1 — Voyage drift (the people change).**~~ **DONE (2026-07-19).** The one
   genuinely new sim mechanic. New `game_config.json → voyage_drift` block
   (`data::VoyageDrift`: per-year `adaptation`/`cultural_drift`/`legacy_loyalty` deltas +
   `morale`/`unity` strain + a per-legacy multiplier map) applied every year in
   `simulation/tick.rs::apply_voyage_drift` (right after the hull/life-support decay):
   adaptation and cultural drift rise, loyalty to the founders fades, morale/unity take
   voyage strain, with the identity terms scaled by the legacy multiplier
   (`preservers 0.6` / `wanderers 1.0` / `adaptors 1.5`). Deterministic (no RNG), clamped
   via `PopulationState::apply`. Today these stats otherwise move only on events; now the
   crew visibly diverges over a long run even in quiet years. Dashboard gained a **FROM THE
   FOUNDING** readout (`dashboard::founder_distance` + evocative label — "true to the
   founding" … "unrecognizable"). Two unit tests: drift changes the people and stays 0–1;
   Adaptors change faster than Preservers. Verified build/clippy/fmt/48 tests + `gameplay`
   capture. **Numbers are a conservative first pass — tune against a real playthrough with
   the M4.7 timer.**
2. **M4.2 — Honest degradation curve + the 30-min floor.** *Degradation curve DONE
   (2026-07-19); the real-time floor calibration is deferred — see below.* Wear now bites
   and spare parts matter: `hull_decay_per_year` 0.005→0.011,
   `life_support_decay_per_year` 0.004→0.008, plus a parts-maintenance rule in
   `tick.rs` — each year the ship spends `parts_upkeep_per_year` (1) spare parts on upkeep
   and, *while parts remain*, eases that year's decay by `maintenance_decay_relief` (0.4);
   once the stores run dry it wears at full rate. So a fresh ship (20 parts) coasts ~20
   years, then grinds down: a 55-year voyage ends ~48% hull with parts exhausted (unit-test
   `a_long_voyage_leaves_the_ship_worn_and_out_of_parts`), and unrepaired wear compounds
   across missions in one `SimState`. Dashboard flags SPARE PARTS red at zero. Verified
   build/clippy/fmt/49 tests (soak still green). **Deferred — the ~30-min real-time
   floor:** it can only be set by measuring actual play time, which needs the M4.7 run
   timer + a human playthrough (headless captures can't measure wall-clock play). Mission
   lengths were left as-is (22–60 yr) pending that. **UPDATE: M4.7's run timer now provides
   the measurement tool** — the calibration is unblocked but is a human playtest (watch
   `RUN` during a run; lengthen missions if under ~30 min).
3. ~~**M4.3 — Two repair regimes: field (underway) vs full (port).**~~ **DONE
   (2026-07-19).** Both verbs live in `simulation/ship.rs` (testable, like `market::buy`)
   and dispatch from `game.rs`, with a new `RepairKind` enum. **Field repair**
   (`ship::field_repair`, `UiAction::FieldRepair(RepairKind)`) patches Hull/LifeSupport by
   `field_gain` (0.12) up to `field_ceiling` (0.75) — never pristine — for `field_parts_cost`
   (4) spare parts + `field_minerals_cost` (150) minerals; it is the sink that makes M4.2's
   parts matter. **Full repair** (`ship::full_repair`, `UiAction::FullRepair`) restores
   hull/life-support/fuel to 1.0 and tops parts back to `full_parts_restock` (20) for
   `full_credits_cost` (1500) + `full_minerals_cost` (500), **refused while
   `contract.is_some()`** ("in port only"). New `data::RepairConfig` + `game_config.json →
   repair` block. The dashboard SHIP STATUS panel gained a MAINTENANCE section (two field
   buttons + a full-refit button) that enables per state — the `gameplay` capture shows the
   field buttons disabled on a pristine ship and FULL REFIT — PORT ONLY disabled while a
   charter is active. 3 unit tests (field caps below pristine; refused without parts; full
   refit port-only + restores all). Verified build/clippy/fmt/52 tests.
4. ~~**M4.4 — Found parts + gated field install.**~~ **DONE (2026-07-19).** Event
   outcomes now **grant a component** into a new `sim.ship.salvage: Vec<String>` hold
   (`EventOutcome.grant_component: Option<String>`, applied in `event_resolver::apply_outcome`;
   `derelict_encounter`/`board_and_salvage` drops a `mass_driver`,
   `resupply_cache`/`claim_the_cache` a `solar_sail`, both narrated in the outcome log). A
   found part is **field-installed underway only if the crew and the part allow it** — the
   gate lives in `ship::install_eligibility` (single source of truth, an `InstallEligibility`
   enum): (a) part — new `ShipComponent.field_installable` (engines/weapons true, hulls false
   → "needs drydock"); (b) crew — an `engineer` with skill ≥ `field_install.skill_required`
   (40) aboard; (c) consumables — `parts_cost` (6) + `minerals_cost` (200). `ship::install_salvage`
   installs (swaps the slot, drops from the hold), charging the field kit underway and free in
   port. `UiAction::InstallSalvage(id)`, dispatched in `game.rs`. **UI without a layout change:**
   a salvaged part *is* a known component, so its existing Ship-Builder catalog card gets a
   brighter (primary) border and its button becomes install-from-salvage — the `ship` capture
   shows Solar Sail Array + Mass Driver as `INSTALL (SALVAGED)`, gated labels (`SALVAGED · NEEDS
   DRYDOCK/ENGINEER/PARTS`) underway. New `data::FieldInstallConfig` + config `field_install`
   block. 3 unit tests (field install gated by crew+part; free in port; grant lands in the
   hold). Verified build/clippy/fmt/55 tests + `ship` capture. **Salvage-pool deepening
   (2026-07-19):** grew the found-parts pool from 2 → **4 sources** with all three part
   kinds — `debris_lattice`/`cut_free` now yields an `armored_prow` (a *hull*, so it exercises
   the "NEEDS DRYDOCK" gate in real play) and `halfway_beacon`/`mark_the_crossing` a
   `ramscoop_array` (engine), both narrated in the outcome logs. A data-load integrity check
   now asserts ≥4 salvage-granting outcomes **and that every `grant_component` id resolves to
   a real component** (catches typos).
5. ~~**M4.5 — Commission a new ship (port-only).**~~ **DONE (2026-07-19).**
   `ship::commission_ship`, `UiAction::CommissionShip(hull_id)`, dispatched in `game.rs` —
   allowed only when `sim.contract.is_none()`. Swaps `ship.hull`, restores
   hull/life-support/fuel to 1.0 and tops parts up, and lifts `morale`/`unity` by
   `commission.hope_*` — but **never resets `cultural_drift`/`adaptation`** (a new ship, never
   new people; unit-tested). Costs the hull's catalog price **plus** a commission premium
   (`commission.premium_credits` 3000 + `premium_minerals` 800). New `data::CommissionConfig`
   + config `commission` block. **UI:** the Ship-Builder hull cards now route to COMMISSION
   instead of the bare component-swap — the `ship` capture shows each hull as
   `COMMISSION · {price+premium}` (e.g. Generation Ark 5500 cr + 1600 min), port-gated
   (`COMMISSION · PORT ONLY` underway); engines/weapons keep PURCHASE. 2 unit tests
   (commission refits + lifts hope + keeps the people; needs the full price). Verified
   build/clippy/fmt/57 tests + `ship` capture.
6. ~~**M4.6 — The drydock phase + the port/underway gate.**~~ **DONE (2026-07-19).**
   **Port-only gate closed:** `purchase_component` now refuses while `contract.is_some()`
   ("loadout changes wait for port"), and the Ship-Builder engine/weapon cards show
   `PURCHASE · PORT ONLY` (disabled) underway — so the full split holds: underway only field
   repair (M4.3) + gated found-part install (M4.4); in port full repair + full loadout +
   commission. **Drydock framing:** the Contract screen's between-missions view
   (`contract_systems::draw_available`, shown whenever `contract == None`) is retitled
   **IN DRYDOCK // AVAILABLE CHARTERS** and gained a **Homecoming** banner from the latest
   `ChronicleEntry` ("HOMECOMING · {contract} — {outcome} (score N), concluded YNN") plus a
   live **CONDITION** line (hull / life / parts / crew) reminding you to refit, above the
   charter grid (grid shifts down, all 8 still fit). New `drydock` capture scene (worn ship +
   a concluded charter in the Chronicle → the full banner). No new sim logic beyond the
   one-line purchase gate, so no new unit tests; verified build/clippy/fmt/57 tests +
   `drydock` capture. (A dedicated `Screen::Drydock` was unnecessary — the existing
   between-missions Contract view is the hub.)
7. ~~**M4.7 — Run framing + pace instrumentation.**~~ **DONE (2026-07-19).** A cosmetic
   wall-clock **run timer** lives on `Game` (`mission_started`/`last_mission_real_secs`,
   session-local; set on `AcceptContract`, frozen at contract completion, reset on any
   transition/retire) — never touches the deterministic sim (Pillar 4 intact; time still
   only moves on `AdvanceYear`). Surfaced via a new `GameplayCtx.run_clock`: the header shows
   a live `RUN mm:ss` while a mission is underway (the `contract_active` capture shows
   `RUN 19:00`), and the drydock **Homecoming** shows the finished run's in-game + real span
   (`Y41 after 40 yr · played 38m`). In-game duration is now persisted on
   `ChronicleEntry.duration_years` (serde-default). Deterministic captures via a
   `capture_run_secs` override. Verified build/clippy/fmt/57 tests + `contract_active` +
   `drydock` captures. **The ~30-min floor is now fully tooled but its calibration stays a
   human task:** play a run, watch `RUN`, and if a successful run finishes under ~30 min,
   lengthen the mission band / decision density (config-only) — I can't measure wall-clock
   play headlessly. **Follow-up (2026-07-19):** the Chronicle screen now displays each
   entry's `duration_years` ("Mining charter · **40 yr** · gen 5 · under … · score 0.92"),
   surfacing the recorded mission length that was previously stored but never shown;
   `chronicle` capture updated.
8. ~~**M4.8 — Charter tiering by renown.**~~ **DONE (2026-07-19).** New
   `ContractTemplate.min_renown` (serde-default 0); richer charters gate behind accumulated
   Chronicle renown (`heritage::renown`, the existing score→points sum): tier-0 (the modest
   rescues/mining) open from the founding, `veiled_expanse_survey`/`seedfall` at 100
   (≈ Remembered), the century-long `founding_colony`/`starfall_beacon` at 250 (≈ Storied).
   The drydock charter grid greys a locked charter's title and shows a
   **`LOCKED · RENOWN N`** button (a terminal access-gate — the escalation path stays in
   view, on-theme for the CRT); `AcceptContract` also refuses below the threshold
   (defense-in-depth). Data-load test asserts both gated and founding-available charters
   exist. Verified build/clippy/fmt/57 tests + `drydock` capture (renown 82 → four charters
   open, `LOCKED · RENOWN 100/250` on the rest). This was the last M4 item. **Content pass
   (2026-07-19): 8→10 charters** — added `warden_patrol` (rescue, renown 100) and
   `the_long_dark` (exploration, renown 400 — an 80-year capstone for storied dynasties),
   extending the ladder to **0 / 100 / 250 / 400** (4/3/2/1 charters); the two-column grid
   holds 10 (5 rows) cleanly; data-load floor bumped to ≥10. **Legibility fix (2026-07-19):**
   the drydock CONDITION line now shows the dynasty's current **`RENOWN N`** next to the
   `LOCKED · RENOWN N` gates, so the escalation path reads at a glance (e.g. renown 82 → the
   100/250/400 locks are clearly "how far to go").

**Resolved (2026-07-19):** run-model = persistent ship, carry on success / reset only on
game-over (gdd.md §12 Q4); pacing = ~30-min floor, ~1-hr soft cap; **repair/loadout split —
field repair + gated found-part install underway, full repair + full loadout + commission
port-only** (M4.3/M4.4/M4.6). No open questions block M4.1–M4.2.

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
- **Hardening:** `tick::tests::long_campaign_stays_internally_consistent` soaks
  a well-fed campaign 250 years across many generations, resolving every
  council decision, and asserts the invariants (0–1 fractions, non-negative
  resources, a living dynasty always has a leader, the survey completes). Keep
  it green when touching the tick/succession/resolver paths.

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
this setup. `stellar_legacy` is its own git repository (default branch `master`),
like every sibling game — commit work there. In this workspace, prefer
`cargo fmt -p stellar_legacy [-- --check]` (bare `cargo fmt` can intermittently
report "Failed to find targets" across the multi-crate workspace).

## Known issues

- ~~**WASM/web crash on first load** (`Cannot read properties of undefined (reading
  'length')` → later a bogus `focus` "already borrowed" panic)~~ **FIXED (2026-07-19,
  toolkit).** Root cause was a **`macroquad-toolkit::wasm_storage::storage_get`** bug, not a
  missing asset: a persisted key absent from localStorage (any first-run web load) made JS
  `getItem` return `null`, which `to_string`'d via `js_string_length(undefined)` and threw —
  unwinding the miniquad frame and poisoning the event-handler `RefCell`, so the next browser
  focus event aborted with the misleading "already borrowed" panic. Only this game tripped it
  because `Game::new()` eagerly loads chronicle/display/delegation/achievements at startup.
  Fixed in `macroquad-toolkit` (`storage_get` now gates on `storage_exists`, commit `58a05f5`);
  `cargo check --release --target wasm32-unknown-unknown` clean. **Re-publish (`.\publish.ps1`,
  which rebuilds the wasm) to pick up the fix** — the change is compiled into the binary.

## Known cosmetic nits (fine to fix opportunistically)

- ~~Meter color logic treats <35% as "critical" red — inverted for
  `cultural_drift`/`adaptation`~~ **FIXED (2026-07-18)** — `term_meter_toned`
  takes a `MeterTone` (`LowCritical`/`HighCritical`/`Neutral`); the dashboard
  bars tag adaptation `Neutral` and cultural drift `HighCritical`.
- ~~The event modal header band is empty~~ **FIXED (2026-07-18)** — the
  category/legacy line now renders centered in the header band; the title leads
  the body.
- ~~Menu lists legacies in sorted-id order (Adaptors first); Preservers-first
  might read better~~ **FIXED (2026-07-19)** — `Game::new` reorders the menu's
  `legacy_ids` to Preservers → Adaptors → Wanderers (cosmetic; legacy choice is
  the player's, never RNG-driven), so the founders' path leads and is the
  default selection.
