# Stellar Legacy — Content Depth (long-term, non-deterministic goal)

This is the **standing north star** for deepening the generational-voyage experience after
the v3 redesign (`plan.md`, W1–W7 all shipped). It is deliberately **not a checklist with an
end state** — it defines *directions* of depth, a *rotation discipline*, and *quality bars*,
so that any number of iterative passes (human or `/loop`-driven) can keep adding content
without the game ever feeling "done wrong." There is no finish line; there is only "deeper
than last iteration, still coherent."

## North star

> A 300–600-year charter should play like a **campaign that was written by the voyage
> itself** — every run surfacing beats the player hasn't seen before, every crisis traceable
> to a decision or a neglected subsystem, every returning generation measurably different
> from the founders. Depth means **more distinct situations**, **more consequence coupling**,
> and **more voice** — never just bigger numbers.

## Hard rules (inherited, non-negotiable)

1. **Data-driven everywhere.** New content lands in `assets/*.json` (+ tuning in
   `assets/data/game_config.json`). Rust changes are for *new mechanics/schema*, never for
   embedding content. Missions are never hardcoded in Rust.
2. **Determinism preserved.** Seeded campaign skeleton at LAUNCH stays reproducible — same
   seed ⇒ same campaign. New randomness goes through state-owned RNG.
3. **Structure before volume.** If a content idea needs a schema field, add the field
   (`#[serde(default)]`, back-compatible) and one exemplar, then grow the catalog in later
   iterations.
4. Repo constraints hold: 800-line file limit, no `mod.rs`, UI stays a pure view layer
   pushing `UiAction`, clippy `-D warnings` clean, soak/playthrough tests green.

## Baseline

*Origin baseline 2026-07-21: 72 events, 10 charters, 6 factions, 6 subsystems, no event gates beyond phase/year/generation/drift.*

| Axis | Current (updated 2026-07-22, after iteration 22) |
| --- | --- |
| Events | **149** total — legacy_drift 25, engineering 20, survival 19, biology_medical 15, diplomacy 13, exploration_first_contact 12, mystery 12, comedy 12, ethics 11, science_anomaly 10 (every family ≥10; 8 of 10 at ≥12) |
| Event gates | phase, min_year/gen/drift, **consequence chains**, **charter tags**, **dominant faction**, **factions aboard**, **subsystem knowledge-below** & **condition-below**, **provisioning shortage** (food/fuel/parts/energy) |
| Charters | **14** in `assets/contracts.json`, all destination-`tags`-annotated; shapes include 2 long-station (parked on-site) + 1 double-hop (two Operation legs) + 1 return-dominant (short op, 320-yr homeward haul) |
| Campaign beats | seeded skeleton (data-driven pools + era layering) + **drift-threshold beats** (legacy_drift as cultural_drift crosses 0.3/0.5/0.7/0.85) + **adaptation-threshold beats** (biology_medical as adaptation crosses 0.35/0.6/0.8) |
| Factions | **6** authored (pick 3), structure-first v1 (no approval meters yet); each has a signature + schism beat, 3 friction pairs, targeted `faction_loss_id` schisms |
| Subsystems | **6** (medical, life-support, agriculture, security, education, engineering) with decay + knowledge-gated repair; events can now `subsystem_deltas` condition/knowledge |
| Campaign | Seeded skeleton beats + reactive fills; phase-gated event weighting |

Update this table occasionally (not every iteration) so drift from baseline stays visible.

## Depth axes — what "deeper" means per axis

Each iteration picks **one axis** and makes it deeper. Rotate; don't camp
(see rotation discipline below).

### 1. Event families (the workhorse axis)
- Grow families toward and past parity (comedy at 6 vs legacy_drift at 14 is fine short-term;
  long-term every family wants 12+ with real internal variety).
- **Complications and twists over new one-offs**: an event that can arrive with 2–3
  complications is worth three flat events. Prefer adding `complication` branches /
  follow-up chains to existing templates where the schema allows; extend the schema when
  it doesn't.
- **Chains**: multi-event arcs where an early choice re-fires a consequence decades later
  (the schema for delayed follow-ups is itself a depth deliverable if missing).
- Replace remaining placeholder flavor (anything still smelling of `event_notes.md`
  brainstorm, e.g. Tribbles-derivatives) with in-universe voice.

### 2. Charters / missions
- New charters should introduce a **new shape**, not a reskin: different phase structures
  (long-station vs double-hop vs deep survey), different quantified objectives, different
  risk profiles, destination flavor that colors which event families weight up.
- Charter-specific event pools / beat overrides (data-driven: charter tags that gate events).

### 3. Factions
- Deepen from structure-v1 toward the layered future the schema reserved: faction-colored
  event reactions, inter-faction friction events, faction-specific dilemma outcomes,
  schism/assimilation beats with more texture, recruitable pool personalities.
- Approval meters / stat modifiers only when an iteration deliberately takes that step —
  it's a mechanics iteration, not a content sprinkle.

### 4. Subsystems & knowledge
- More failure/repair texture per subsystem: distinct breakdown events per module, knowledge
  crisis beats ("the last person who understood the reactor is dying — arrange a teaching
  succession?"), tier-specific flavor on upgrades.
- Cross-couplings (agriculture failure → survival event pressure → medical load) expressed
  in data where possible.

### 5. Campaign skeleton & pacing
- More beat archetypes for the seeded skeleton; era texture (early-voyage vs mid vs
  homecoming beats); drift-threshold beats that fire on `cultural_drift`/`adaptation`
  crossings; better dead-air detection (long stretches with nothing eventful = a bug in
  content coverage, not a mercy).

### 6. Provisioning & opportunity
- More shortage-driven opportunity events (garden-world stop archetype), fuel-crisis
  branches, cargo/salvage texture on Return legs.

### 7. Voice & presentation (cheap, high-value)
- Log-line variety, dated flavor lines, generational obituary/succession flavor, help/hint
  text, richer outcome prose. Pure JSON edits; a good "small iteration" axis.

## Consequence coupling — the depth multiplier

Whenever adding content, prefer variants that **touch two systems**: an event that damages a
subsystem AND shifts a faction, a charter whose objective interacts with knowledge decay, a
dilemma whose cost lands a generation later. Isolated content adds breadth; coupled content
adds depth. One deliberate coupling per iteration is a good bar.

## Iteration discipline (for /loop or manual passes)

1. **Pick the axis** — rotate: the axis least-recently touched wins ties; never the same
   axis twice in a row unless finishing a schema change started last iteration.
2. **Scope one iteration** at "one sitting": e.g. 4–8 events with complications, or 1–2 new
   charters, or one schema extension + exemplars, or one coupling mechanic wired through
   `event_resolver.rs`/tick.
3. **Author in JSON first**; touch Rust only for schema/mechanics the content needs.
4. **Verify**: `cargo test -p stellar_legacy` (incl. the long soak + automated playthrough
   harness), `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt -- --check`.
   If the iteration changed pacing-relevant content, eyeball a harness transcript for
   dead air / event spam.
5. **Log it** in the rotation log below (one line), commit with a message naming the axis.

## Quality bars (reject content that fails these)

- **Choice matters**: every event offers options with genuinely different consequences —
  no "OK" buttons on anything above flavor weight.
- **Phase-appropriate**: gated to the phases where it makes fictional sense.
- **Century-aware**: content that only makes sense at generational scale (drift gates,
  min_year/min_generation) is preferred over timeless filler.
- **No repetition tells**: if a template will plausibly fire 3+ times in one voyage, it needs
  complication variety or tighter gating.
- **In-universe voice**: no placeholder names, no fourth-wall, consistent tone with
  `event_design_notes.md`.

## Non-goals

- No new game modes, no cryosleep, no one-way colonization — the round-trip refit loop is
  the frame.
- No content in Rust; no balance constants in Rust.
- No breaking the automated playthrough harness — it is the primary playtest channel and
  every iteration leaves it green.

## Rotation log

*(append one line per iteration: date · axis · what landed)*

- 2026-07-21 · (bootstrap) · baseline captured, this document created.
- 2026-07-21 · event families + coupling · added `requires_consequence` gate (schema + `passes_gate` wiring) so an early choice can unlock a later event; authored the `sealed_ward → the_ward_reopens` chain plus 5 parity events (biology_medical 6→8, survival/mystery/diplomacy/comedy/science_anomaly 6→7). Events 72→79. Couplings used: consequence chain, `faction_loss` on harsh rationing, fuel/drift trade, gated salvage grant. +1 test (92 total).
- 2026-07-21 · charters + coupling · new charter↔event coupling: added `tags` to charters (all 10 tagged) copied onto the active contract, and a `requires_charter_tag` event gate so a destination colors its own event pool. Authored 4 tag-keyed events (boarding scare on `hostile_space`, settler-drain on `garden`, starless-reach drift on `deep_space`, richer-find early return on `uncharted`). Events 79→83. Data-load check rejects tags no charter carries. +1 test (93 total).
- 2026-07-21 · factions + coupling · faction↔event coupling: `dominant_faction_id`/`is_faction_aboard` helpers + two event gates — `requires_dominant_faction` (signature events when a faction runs the ship) and `requires_factions_aboard` (inter-faction friction). Authored 4 faction-colored events (Ascension cradle-augmentation, First Flame reactor-rite, Verdant Kin garden-vs-salvage, Flame×Ascension partition). Events 83→87. Data-load check rejects unknown faction ids. +1 test (94 total).
- 2026-07-21 · subsystems + coupling · subsystem↔event coupling: outcome `subsystem_deltas` (signed condition/knowledge changes, clamped) wired into `apply_outcome`, and a `knowledge_below` crisis gate so events fire as a module's know-how decays. Authored 4 events (the-last-engineer teaching succession, coolant-loop rupture damaging the eng bay, grow-deck blight denting agriculture + food, forgotten-medicine crisis). Events 87→91. Data-load check rejects unknown subsystem ids. +1 test (95 total). Event gates now: consequence, charter-tag, dominant-faction, faction-aboard, knowledge-below.
- 2026-07-21 · campaign skeleton · made the seeded-beat pools data-driven — lifted the hardcoded Rust phase→family tables into a `campaign_skeleton` config block (honoring the data-driven hard rule) — and added **era layering**: beats in the first 20% of the voyage also draw a founding-era pool (exploration/anomaly/comedy), beats in the last 20% a homecoming-era pool (legacy_drift/ethics). `generate_beats` now takes the config; all 3 call sites updated. Data-load check rejects pool families with no events. +2 tests (96 total). No new events this pass (structural).
- 2026-07-21 · provisioning + coupling · provisioning↔event coupling: `food_below`/`fuel_below`/`spare_parts_below` shortage gates on events, wired into `passes_gate`. Retro-fitted the existing opportunity events so they now fire *because* you're short (garden_world food≤4000, fuel_skim fuel≤0.45, resupply_cache food≤5000) rather than at random. Authored 3 new shortage beats (the-dry-tank fuel crisis strips the ship for mass, the-empty-lockers parts foundry, the-laden-return cargo/escort risk on the Return leg). Events 91→94. +1 test (97 total).
- 2026-07-21 · voice · killed the game's worst repetition tell: the obituary / succession / coming-of-age lines fire every generation (12+×/voyage) and were 3 hardcoded Rust strings. Lifted them into a data-driven `flavor` config block with 6/6/5 authored variants, indexed by generation (deterministic, no RNG perturbation) so a seed still replays exactly. Placeholders `{name}`/`{generation}`/`{births}` substituted. Data-load check + helper unit test (98 total). This completes one full rotation through all 7 axes.
- 2026-07-21 · event families (round 2) · brought **ethics** to parity (6→11, the thinnest family) leaning on round-one mechanics: the-stowaway census dilemma, the-mercy-dose (gated on low medical knowledge, dents the bay), the-archive-lie, a century-spanning consequence chain (the-mutineers-sentence exiles a faction → the-exiles-return generations later via `requires_consequence`), plus science_anomaly parity (the-second-star). Events 94→**100**. Added a data-load provenance check: every `requires_consequence` tag must be produced by some outcome (typo guard). No new test fn (content pass); 98 total. Refreshed baseline table.
- 2026-07-21 · charters (round 2) · added 2 charters with a genuinely new **long-station** shape — most of the voyage parked on-site, not in transit (the-deep-camp, mine a cinder vein for 300 of 480 yr; hearthfall-accord, an 8-generation embassy residency). Both carry a new `long_station` tag; authored 2 tag-keyed events (the-stationkeepers: a generation that knows only the worksite; the-idle-hull: a parked ship seizes up as no moving one does). Charters 10→12 (grid already scales), events 100→102. +1 soak test flying the 480-yr Deep Camp end to end (99 total).
- 2026-07-21 · factions (round 2) · gave signature events to the 3 factions that lacked them — Steel Covenant (workshops over classrooms: +engineering, −education), Meridian Accord (slow arbitration: +stability), Hearth Union (the long table: +morale/unity). Added 2 more inter-faction friction pairs (forge×hearth, accord×ascension). Deliberate new coupling: **faction→subsystem** via `subsystem_deltas` on faction outcomes. Events 102→107; all 6 factions now have signature beats + 3 friction pairs. Added a data-load coverage assertion (every faction has a `requires_dominant_faction` event); 99 total.
- 2026-07-21 · subsystems (round 2) · round one gave knowledge-crisis beats only to engineering + medical; this pass covers the other 4 — life_support (the-breath-keepers, scrubber chemistry lost), security (the-unlearned-watch, peace hollowed the corps), education (the-teachers-gap, schools teaching shape not substance), agriculture (the-lost-gardeners, craft not passed on). Plus the doc's example cross-coupling: the-hungry-wards (agriculture shortfall → malnutrition → medical-bay load, moving both subsystems). Events 107→**112**. Added a data-load coverage assertion: every subsystem has a `knowledge_below` crisis event; 99 total. Refreshed baseline.
- 2026-07-21 · campaign skeleton (round 2) · **drift-threshold beats**: new `drift_beats`/`drift_beat_family` config + a `drift_beats_fired` contract counter + a `fire_drift_beat` hook in the advance loop. The first month cultural_drift reaches each of 0.3/0.5/0.7/0.85, a legacy_drift beat is forced — so the signature Long-Term Expedition beats fire as *consequences of how far the voyage has changed the people*, not random rolls. Fires once per threshold, deterministic. Data-load check (ascending, in-range, family has events) + a firing unit test; the soaks now exercise it. No new events (structural). 100 tests.
- 2026-07-21 · provisioning (round 2) · completed the shortage-gate set with `energy_below` (food/fuel/parts/**energy**), wired into `passes_gate`. Authored 5 beats: the-dimming (energy crisis → brown out habitats or industry, couples energy→subsystem), the-ice-moon (stop to mine volatiles when fuel low), a food-shortage consequence chain (the-seed-corn eats the replant reserve → the-barren-decks a generation later via `requires_consequence`, both denting agriculture), and the-homeward-wreck (Return-leg salvage when parts-scarce, grants a component). Events 112→**117**. +1 test (101 total). Refreshed baseline.
- 2026-07-22 · voice (round 2) · **ambient flavor** for dead-air: a 10-line `ambient` pool + `ambient_gap_years` in the flavor config, emitted from the year-boundary tick once per gap-years of event-less quiet (indexed by year, deterministic, dated by the log, never resets the event ramp). Long centuries between decisions now read as lived-in — murals, drifted songs, a keeper unsure which parts of the founding log are still true. Data-load check (ambient non-empty when enabled) + a quiet-stretch unit test (102 total). Completes the **second full rotation** through all 7 axes.
- 2026-07-22 · event families (round 3) · pushed the thinnest family, exploration_first_contact, from 7 to the doc's 12+ bar with genuine **first-contact** scenarios the family had lacked (it was mostly navigation flavor): the-wayfarers (a fellow generation ship), the-wary-frontier (a burned-before civilization), the-young-world (prime-directive dilemma), the-silent-monuments (ruins of a vanished people), the-last-broadcast (an eon-old beacon awaiting a reply). Plus science_anomaly and comedy parity lifts (the-wandering-comet, the-great-bake-off). Events 117→**124**; every family now ≥9. Data-load category check caught a family-vs-category typo mid-pass. 102 tests.
- 2026-07-22 · charters (round 3) · added a third charter **shape**: the double-hop (twin_survey), a `[travel, operation, travel, operation, return]` topology — two anomalies surveyed in one voyage before turning home, objective accruing cumulatively across both Operation legs. Verified the phase engine already handles arbitrary topologies (phase_at walks segments; operation_months sums them). New `double_hop` tag + 2 keyed events (the-second-departure: turn deeper out or take partial pay and go home via `force_return`; the-far-deep: a stranger second anomaly). Charters 12→13, events 124→**126**. +1 soak test flying the 440-yr Twin Survey across both legs (103 total).
- 2026-07-22 · factions (round 3) · **faction-specific schism beats** with a new mechanic: outcome `faction_loss_id` + `apply_faction_loss_by_id` shed the *named* faction (not just the smallest), so a schism removes the group it's actually about. Authored 3 drift-gated schisms — the-ascension-exodus (the augmented depart to become what they're becoming), the-flame-orthodoxy (the faith hardens: bend the whole ship to its rites or let it secede), the-verdant-secession (the Kin plant a world and stay). Each gated on min_cultural_drift + faction aboard, coupling factions + drift + faction_loss. Events 126→**129**. Data-load check extended to `faction_loss_id`; +1 targeted-loss unit test (104 total).
- 2026-07-22 · subsystems (round 3) · **condition-breakdown gate**: generalized `KnowledgeGate`→`SubsystemGate` and added `condition_below` (the physical-failure parallel to knowledge_below), wired into `passes_gate`. Every subsystem had a knowledge crisis; now the modules can also fire *breakdown* beats as their condition rots — the-failing-air (life-support failing in earnest: all hands or seal the dying decks and lose people), the-open-locks (security decayed to uselessness), the-seized-works (engineering bay so worn it chokes the ship's whole capacity to mend). Events 129→**132**. Data-load subsystem-ref check extended to `condition_below`; +1 gate unit test (105 total).
- 2026-07-22 · campaign skeleton (round 3) · **adaptation-threshold beats** — the physiological parallel to round 2's drift beats, completing the doc's "cultural_drift/adaptation crossings." New `adaptation_beats`/`adaptation_beat_family` config + `adaptation_beats_fired` counter + `fire_adaptation_beat` (shared `force_family_beat` helper extracted). As the people's `adaptation` crosses 0.35/0.6/0.8, a biology_medical beat fires — the descendants growing shipborn. Authored 3 adaptation-themed events (the-shipborn-body, the-recycled-palate, the-quiet-lungs: fitness for the ship becomes fragility to its failures). Events 132→**135**. Data-load beat check generalized to both stat kinds; +1 firing test; 2 timeline tests clear both beat lists (106 total).
- 2026-07-22 · provisioning (round 3) · pure content pass on the mature shortage-gate set: the-long-winter (a **compounding crisis** gated on low food AND low energy at once — hunger and cold together), a rationing-discipline consequence chain (the-lean-pact swears equal shares → the-pact-remembered generations on, when the pact has hardened into identity and merit-reward lands like heresy), and Return-leg texture (the-trade-convoy converts cargo→provisions; the-drifting-tanker, a derelict fuel bounty when the tank is low). Events 135→**140**. +1 test locking the multi-shortage AND semantics (107 total). Refreshed baseline.
- 2026-07-22 · voice (round 3) · **occurrence-aware phase-transition flavor**: the phase lines were hardcoded, so the double-hop charter reprinted the same "Departure burn complete"/"makes station" text on its *second* Travel/Operation — a real repetition tell. Added a data-driven `phase_lines` pool (keyed by phase) + `ActiveContract::phase_occurrence`, indexed so the second departure/arrival reads differently; missing pool falls back to the built-in line so the log never blanks. `phase_transition_line` now takes the flavor config. Data-load key check + a double-hop variety unit test (108 total). No new events (voice pass). Completes the **third full rotation** through all 7 axes.
- 2026-07-22 · charters (round 4) · a fourth charter **shape**: the return-dominant tow (the_long_tow), `[travel 70, operation 60, return 320]` of 450 yr — the only charter whose hard stretch is the *haul home*, not the outbound or the on-station work. Grapple a dead titan-ship at a near waypoint (objective accrues in the short Operation), then nurse an overloaded ship and its parasitic tow across three centuries of deep dark; success weighted onto PopulationSurvival + ResourceEfficiency (enduring the return) over MissionCompletion. Verified the engine needs no change — objective banks in Operation, the return grades on survival/efficiency/cohesion across all months. New `long_return` tag + 3 Return-phase-keyed events giving the haul its signature texture: the-towline (re-rig the failing anchor for parts vs shed part of the prize to lighten ship — engineering condition/spare parts vs fuel/hull), the-overloaded-decks (bear the tow's draw and strain life-support vs let it go cold and lose freezable salvage — life_support coupling), the-endless-haul (a generation born to nothing but the tow: make the labor sacred → unity+drift vs point them at a home they'll never see → morale/stability but restless). Charters 13→**14**, events 146→**149**. +1 soak test flying the return-dominant voyage + asserting its topology (110 total).
- 2026-07-22 · event families (round 4) · closed the last parity gap: brought the two thinnest families, **mystery** (9→12) and **comedy** (9→12), to the doc's 12+ bar, so every family is now ≥10 and 8 of 10 at ≥12. All 6 are century-aware (gated on generation/drift/knowledge decay) with genuine two-way choices and system couplings, not flavor one-offs: the-sealed-deck (breach a founder-welded deck for salvage vs keep it a shared mystery — engineering condition + drift), the-second-log (publish a contradicting founding record vs bury it — education knowledge + drift, seeds a `buried_second_log` consequence for a future chain), the-wandering-mind (obey the old nav archive and lose understanding vs rebuild it by hand — knowledge-crisis gate, opposite subsystem-knowledge swings), the-festival-war (rival festivals vs a fused invented holiday), the-reconstructed-feast (canonize a delicious fake vs honor the archive — education knowledge), the-office-of-lost-things (fund an absurd bureau that actually mends the shops vs disband it — engineering condition + spare parts). Events 140→**146**. +1 test locking the wandering-mind divergent-choice coupling (109 total).
