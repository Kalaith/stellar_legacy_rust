# Stellar Legacy — Framework Handoff Plan

*Written 2026-07-18 after the initial framework build. Read `gdd.md` first — this
document maps the GDD onto what already exists in code and what the next agent
should build, in order.*

## Current status: M1–M3 complete; M4 (the refit loop) newly specified, not yet built

*Status refreshed 2026-07-19 (this doc opens with the original 2026-07-18 framework
snapshot; the numbered log below records what shipped since).* Every numbered item
(1–10, the original M1–M3 scope) and all three cosmetic nits are **done**; the game is
content-complete against GDD §8 (and past several minimums) — 46 events, 5/5/5
components, 8 contracts, 8 dilemmas per legacy, doubled name pools.

**New direction (owner-directed 2026-07-19):** a **Voyage-and-Return Refit Loop** —
one *run* = one mission flown by a persistent ship that leaves fresh and hopeful and
arrives back worn and changed, with a between-missions **drydock** economy (repair /
upgrade / commission a new ship) funding the next run, paced to ~45–75 min. Design
captured in `gdd.md §3.1`; the code-grounded build order is **M4** below. This is now the
active work; the ko-fi/index.html marketing screenshots (item 9) remain the only
human-gated leftover from M3.

Verified: `cargo test` (**46 tests green**, incl. a 250-year soak/integration
test), `cargo clippy --all-targets --all-features -- -D warnings` (clean), `cargo
fmt` (applied), WASM target checks (`cargo check --release --target
wasm32-unknown-unknown`), and headless UI captures for ~20 scenes under
`docs/verification/` (menu, gameplay, event, dilemma, dilemma_combat, crew, ship,
market, contracts, contract_active, boot, settings, help, green, log, heritage,
chronicle, gameover), regenerated with `.\scripts\capture_ui.ps1 -Scenes <list>`.

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
| Capture harness | `src/main.rs` (`STELLAR_LEGACY` prefix) | **Done** — ~20 scenes (`menu`, `gameplay`, `event`, `dilemma`, `dilemma_combat`, `crew`, `ship`, `market`, `contracts`, `contract_active`, `boot`, `settings`, `help`, `green`, `log`, `heritage`, `chronicle`, `gameover`) |

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

*Design intent in `gdd.md §3.1`. This section is the code-grounded build order. Nothing
here is built yet — items are ordered so each is shippable and verifiable on its own.*

**Why this is a small build, not a rewrite.** The persistent ship the owner wants already
exists in the code: a contract completes and **auto-continues in the same `SimState`**
(`game.rs:853-903` clears `sim.contract` and banks `template.reward`, no menu round-trip,
no reset), and the whole `SimState` — ship loadout, population, dynasty — is already
autosaved to `save_slot` (`save.rs`; autosave on `ToMenu` at `game.rs:617-624`). So the
"persistent ship across missions" needs **no new carry channel**, and "in drydock between
missions" is simply the state `sim.contract == None`. Extinction (`dynasty.extinct`,
`game_over.rs`) stays the true game-over. The work is to make the *arc* felt and give the
*port phase* real verbs.

**Working assumption (see gdd.md §12 Q4):** a run = one mission on a persistent ship
(recommended). If the owner instead wants runs to be disposable campaigns with the ship
carried across *campaigns*, M4.3–M4.5 still hold but would need a ship-carry channel like
Heritage — flag before building if that's the intent.

1. **M4.1 — Voyage drift (the people change).** The one genuinely new sim mechanic. Add
   a `game_config.json → voyage_drift` block and apply it per year in
   `simulation/tick.rs` (alongside the hull/life-support decay at `tick.rs:73-74`): small
   deltas that raise `population.adaptation` and `cultural_drift`, nudge `legacy_loyalty`
   toward the faction pull, and slightly erode `morale`/`unity` under voyage strain,
   scaled by a per-legacy modifier (Adaptors fastest, Preservers slowest). Deterministic
   (no RNG), clamped via `PopulationState::apply` (`sim.rs:80-88`). Today
   `adaptation`/`cultural_drift`/`legacy_loyalty`/`stability` move *only* on events — this
   makes the crew visibly diverge over a long run even with no events. Surface a "distance
   from the founders" readout on the dashboard. Test: over N years drift accrues
   monotonically and stays in 0–1.
2. **M4.2 — Honest degradation curve.** Retune (config-only where possible) so a single
   long mission ends ~40–55% hull — "held together on hope and prayers." Raise
   `hull_decay_per_year`/`life_support_decay_per_year` and/or widen the default mission
   length so wear is felt; add a small per-year `spare_parts` consumption (today
   `spare_parts` moves only via events) so restocking matters. Decay already compounds
   across missions in one `SimState` (nothing resets it), so skipping repairs bites.
3. **M4.3 — Repair verbs (drydock sink).** New `UiAction::Repair(RepairKind)` (Hull /
   LifeSupport / Fuel / Parts) dispatched next to `purchase_component`
   (`game.rs:906-935`): each costs credits+minerals from config, restores its stat toward
   1.0 partially per purchase, gated by `resources.can_afford`. Priced-anytime, but
   foregrounded in port.
4. **M4.4 — Commission a new ship.** New `UiAction::CommissionShip(hull_id)`: a
   large-credit purchase that swaps `ship.hull`, restores hull/life-support/fuel/parts to
   full, and grants a one-time morale/hope lift — but does **not** reset
   `cultural_drift`/`adaptation` (a new ship, never new people). Log a christening line.
   This is the owner's "buy a new ship for the next run."
5. **M4.5 — The drydock phase.** When `sim.contract == None` and the dynasty lives,
   frame the arrival-and-refit beat: a one-time **Homecoming** summary on arrival (years
   this mission, hull + population change since departure, reward banked) and an "IN
   DRYDOCK" hub surfacing repair / upgrade / commission / accept-next-charter. Simplest
   build: a between-missions banner + summary on the existing Contract/Ship/Market
   screens; optionally a dedicated `Screen::Drydock` (`gameplay.rs:9-26`). UI stays a pure
   view — new `UiAction`s only. Capture a `drydock` + `homecoming` scene.
6. **M4.6 — Run framing + pace instrumentation.** A cosmetic wall-clock run timer on the
   HUD and in the Homecoming/Chronicle summary (elapsed real time for the mission) — not
   background sim; time still only moves on `AdvanceYear`. Record per-mission real and
   in-game duration into the `ChronicleEntry` (`chronicle.rs:13-49`). Use it to tune M4.2
   and decision density toward the ~45–75 min target.
7. **M4.7 — (Stretch) Charter tiering by renown.** Gate larger/richer charters behind
   accumulated renown or missions-completed (ties into `heritage::derive`,
   `heritage.rs:43-65`) so later runs escalate. Keep flat for v1; add tiers only if runs
   start to feel same-y.

**Open questions for the owner** (don't block M4.1–M4.2 on these): run-model interpretation
(gdd.md §12 Q4); is 1 hour a soft target (recommended, tuned via M4.2/M4.6) or a hard cap;
should repair/commission be port-only or priced-anytime (recommended anytime).

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
