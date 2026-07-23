# Stellar Legacy — Game Design Document

*Draft v0.1 — living document.*

> A generational starship strategy game about sending a living ship-civilization on
> missions that take decades or centuries to complete. You don't command a disposable
> crew — you build a legacy. Captains age out, children inherit roles, promises outlive
> the people who made them, and every voyage reshapes the society aboard your vessel.

Sources: `game_apps/stellar_legacy/` (React/PHP original), `RustGames/migration_candidates.md`,
`RustGames/standing.md`, `RustGames/docs/GAME_DEVELOPMENT_GUIDE.md`,
`RustGames/docs/CODE_STANDARDS.md`, `RustGames/docs/MACROQUAD_TOOLKIT.md`.

---

## 0. Migration Snapshot

- **Old game:** `game_apps/stellar_legacy/` — a React 19 + Zustand frontend with a thin
  PHP/Slim backend that does nothing but persist an opaque JSON save blob behind the
  shared WebHatchery login. All game rules live client-side in `frontend/src/services/*`.
- **Why it was picked:** `migration_candidates.md` Tier 3 — "generational dynasty-management
  strategy; conceptually close to `apartment`'s succession/portfolio mechanic, different
  setting." Not a zero-art-liability slam dunk like the Tier 1 picks, but it turns out to
  have **no art liability at all** (see below) and fills a genre the Rust catalog doesn't
  have: nothing else in `standing.md` is a multi-generation succession sim built around a
  single ship-civilization rather than a multi-building/multi-settlement portfolio.

- **Art-liability audit.** This is the unusual case the template asks to call out
  explicitly: the old game commissioned **zero art**. A full source grep for
  `.png/.jpg/.jpeg/.svg/.gif/.webp` under `frontend/src` returns nothing, there's no
  `assets/`/`public/` art directory, and no canvas/WebGL/icon library is even a
  dependency. The entire UI is a deliberate **text/CSS "terminal" aesthetic** — monospace
  font, amber/green/red phosphor palette (`--terminal-primary:#FFB000`,
  `--terminal-success:#00FF66`, ASCII box-drawing borders). That look ports almost
  verbatim:

  | Old asset (web) | Art cost | Rust replacement |
  | --- | --- | --- |
  | Terminal chrome (`TerminalWindow`, ASCII borders, phosphor palette) | None (CSS only) | `ui` `SurfaceStyle`/`TextStyle` + a monospace bitmap font; reuse the same hex palette via `colors` |
  | Ship/crew/dynasty/planet iconography | None (text labels only) | Keep as text/short glyphs; no icon commissioning needed |
  | Galaxy map | None (a list, not a rendered map) | Toolkit `math`/simple 2D node layout if a literal starmap view is kept (see §7); otherwise stays a list panel |

  Nothing here requires an artist. The entire "port" is a systems/content problem, not an
  art problem — which also means there's no cost saved by *cutting* systems for art
  reasons; every cut below is a scope/complexity call, not an art-avoidance call.

- **Mechanic carry-over table.**

  | Old mechanic | Disposition | Notes |
  | --- | --- | --- |
  | Resources/ship/crew/market "Phase 0" loop | Keep, redesign | Real-time `setInterval` tick doesn't fit a deterministic turn-based Rust sim (per `CODE_STANDARDS.md` §5); becomes an explicit "advance time" step. Ship/crew/market data moves to `assets/*.json`. |
  | Star system explore/colonize ("Galaxy Map", up to 50 systems + trade routes) | Redesign, scope down | The multi-system empire-building framing overlaps `frontier`/`realmseed`/`empire_builder`'s genre slot and was never implemented past a system list + one boost stat. Ported as: the ship's current mission has a handful of relevant systems (origin, waypoints, destination), not a sprawling colonial empire. |
  | 4-resource market buy/sell w/ price trends | Keep as-is | Already fully implemented, simple, fine. |
  | Generational Mission sim (population, resources, phases, milestones, success metrics) | Keep — this is the core system | This *is* the game. Ported wholesale but content-expanded (only 4 event templates existed — see §8) and with the dead colonization-boost code path (§5.1) actually fixed. |
  | Dynasty succession (age, leader handoff at 70, 25-year generation tick) | Keep as-is | Solid, simple, working formula (see §5.3). |
  | 3 Legacy factions (Preservers/Adaptors/Wanderers) + per-legacy dilemmas | Keep, fix | Dilemma resolution math is real and good; several inputs (`traditionPoints`, `bodyHorrorEvents`, `existentialDread`, `piracyReputation`) were hardcoded placeholders never actually tracked — port makes these real tracked state. |
  | Dynasty Hall / Legacy Relations / Cultural Evolution actions (~15 buttons, ~13 of them cosmetic no-ops) | Redesign | Collapse to ~5–6 actions that actually move real `Population`/`Dynasty` stats (unity, culturalDrift, legacyLoyalty, influence already exist in the data model — just weren't wired to most buttons). No flavor-text-only actions in the port. |
  | Legacy Deck (narrative card meta-game) | Cut for v1 | Fully scaffolded in types/services but **unreachable in play** — every card-content generator except one returns `null`/stub, and no `LegacyCard` is ever created in a real playthrough. Revisit only as a post-v1 stretch (§12). |
  | Chronicle + Heritage modifiers (cross-playthrough meta-progression) | Redesign, scope down | Concept kept (a lightweight persistent log that grants small modifiers to a new dynasty), but the original's analytics (`engagement`, `AI-performance metrics`, artifact/discovery generation) are almost entirely stub methods returning empty arrays — port replaces them with a small, honest, fully-implemented summary instead of scaffolding that looks deep but isn't. |
  | Pacing/"AI director" (time acceleration, engagement scoring, emergent event scheduling) | Redesign, scope down | The "engagement score" heuristic isn't derived from real telemetry and most of its methods are stubs. Replaced with explicit player-facing time controls (step 1 year / step to next decision) and a simple rule: always pause on a decision-required event. |
  | Automation/Council delegation (auto-resolve events via advisor AI) | Keep, simplified | The outcome-scoring auto-resolver (`evaluateOutcomeScore`) is real and reasonable; keep as an optional per-domain delegation so late-game "century-scale" ticking doesn't require manual resolution of every event. |
  | Phase1SimulationService ("vertical slice" demo tab) | Cut as a separate screen; keep its design | Isolated from the rest of the game in the original. Its shape — monthly tick, seeded LCG RNG, contract-due-date scoring, banded event outcomes — is the cleanest, most tractable model in the whole codebase and becomes the blueprint for the unified simulation tick (§5), not a standalone demo mode. |
  | PHP backend, WebHatchery shared login, server-side save slots | Cut | Confirmed to contain zero game rules — it's a JSON-blob store behind auth. A standalone Rust/Steam/itch game doesn't want an online account system; local save slots via `macroquad-toolkit::persistence` replace it entirely (see §7). |

- **Explicitly out of scope for the port:** multiplayer/netcode, real-money mechanics
  (none existed), the Legacy Deck card meta-game (v1), any server-authoritative or
  account-gated save system, literal galaxy-spanning multi-system empire management
  (that's `frontier`/`realmseed`/`empire_builder`'s job).

---

## 1. High Concept

- **Pitch:** You are the standing council of a generation ship — a vessel that is also a
  city, a company, and a dynasty. Every choice you make will be inherited by people who
  aren't born yet; every promise your ship makes to the galaxy will still be owed a
  century later, by someone else's grandchildren.
- **Genre:** Generational strategy / succession sim. Distinct from the catalog's existing
  kingdom-builders (`realmseed`, `frontier`) and city-management sims (`apartment`) —
  those manage *places*; this manages *one vessel's bloodline* across unbroken decades.
- **Perspective & presentation:** UI-only, no world/map rendering in the traditional
  sense — a text/log-driven "terminal" interface (retained wholesale from the original,
  see §0) with panels, meters, and an event log. No `camera`/`sprite` work needed; this
  is almost entirely the toolkit's `ui` module.
- **Tone:** Dry, procedural, slightly cold — a ship's log and a corporate ledger that
  happen to contain human tragedy and triumph. Understatement over melodrama; let the
  numbers (a population that "diminished," a dynasty that "went extinct") carry weight.
- **Comparables:** *Reigns* (succession-by-decision at a remove from any individual
  character), *FTL* (long-haul crisis events under resource pressure), *Star Traders:
  Frontiers* (text-forward sci-fi systems depth without character art).
- **Audience:** Strategy/sim players who want systems depth and long-arc consequence over
  spectacle — the same audience `realmseed`'s chronicle-driven campaign appeals to, but
  wanting a tighter single-vessel focus instead of a multi-settlement realm.
- **Scope:** Full game (not a vertical slice) — see §13 for staged milestones.
- **Platforms:** itch.io + Steam via WebGL and native Windows (standard for this catalog).

---

## 2. Design Pillars

1. **Generations, not runs.** A single playthrough is meant to span multiple leaders,
   multiple completed contracts, multiple event chains — not one mission and done. A
   system that only matters for one leader's lifetime is a candidate for cutting.
2. **Every decision is a debt someone else pays.** Resource choices, dilemma resolutions,
   and delegation settings should have consequences that surface turns or generations
   later (unpaid maintenance, a resentful cohort, a legacy grudge) — this is the whole
   point of "Legacy and Consequence" from the original pitch, and it must be backed by
   real state changes, not the old build's flavor-text no-ops.
3. **The log is honest.** If a system isn't deep enough to show real numbers changing,
   it doesn't get a panel that implies otherwise. The original's Legacy Deck and Chronicle
   analytics looked rich in the type system and were empty in play — the port doesn't
   repeat that mistake (§0 redesigns exist specifically to close this gap).
4. **Centuries move at the speed of decisions, not a clock.** ~~No background real-time
   ticking (the original's `setInterval` resource loop). Time advances only when the
   player asks it to, so a "century-scale mission" is legible and deterministic, not a
   race against a wall-clock timer.~~
   **Superseded (real-time loop).** Time now auto-advances while a mission is under way
   (~1 month per 5 s real time, `real_time.seconds_per_month`), controllable with a
   Pause / 1× / 2× / 3× selector; **docked, time is frozen** so refit and charter choice
   are unhurried. A blocked council decision holds the clock and **auto-resolves to a
   random option after 30 s** (`real_time.decision_timeout_secs`). The *sim internals*
   stay seeded (event rolls, ranged impacts, and timeout picks all draw from `sim.rng`),
   so a manual `advance_*` still replays deterministically — but the live wall-clock pace
   and player-timed choices mean a played session is no longer a strict seed replay.
   The decision still *matters*; it just no longer waits forever.
5. **Succession is a mechanic, not a screen transition.** Leader death/retirement,
   heir selection, and generational aging (§5.3) drive real gameplay stakes — new leaders
   bring different skills, and the player should feel the loss of a specialist captain.

---

## 3. Core Loop

**Session loop** (one "turn" = one in-game year, occasionally finer during an active
event):

1. Review the ship/colony dashboard — resources, population, hull/life-support health,
   any pending decision-required events.
2. Allocate the turn: assign crew/cohort focus, spend resources on ship components,
   crew training, or colony development; adjust delegation settings.
3. Advance time (explicit player action). The simulation tick applies production,
   population change, dynasty aging, and rolls for a new event.
4. Resolve any event that requires a decision (or let a delegated advisor auto-resolve
   it, per §5.4). Auto-resolved events still log their outcome.
5. Repeat until the active mission/contract reaches its target duration or fails outright.

**Campaign loop** (spans the whole playthrough, mirrors `realmseed/gdd.md`'s
prototype/full split):

1. Accept a long-term contract (mission objective + target system + duration).
2. Prepare the ship, crew, and finances for it (§3 session loop, steps 1–2).
3. Survive the contract's duration through the session loop, generation after
   generation.
4. Resolve the contract (complete / partial / pyrrhic / failure — §5.2) and record it to
   the Chronicle.
5. Use the outcome — resources, reputation, a surviving dynasty — to prepare the next
   contract or expand the ship itself.
6. Continue until the ship's dynasty goes extinct or the player chooses to retire the
   save; the Chronicle then offers a small set of Heritage modifiers for a new
   playthrough (§7).

### 3.1 The Voyage-and-Return Refit Loop (v2 direction, 2026-07-19)

*Owner-directed refinement of the campaign loop above. The intent: make one **mission
run** the unit of play, felt as a departure-and-homecoming arc, with a between-missions
refit economy that gives past success somewhere to go.*

- **A run = one mission (charter) flown by a persistent ship.** The ship is a single
  continuous `SimState` across missions — it is not rebuilt between charters. Accepting a
  charter is *casting off*; the contract's `return → completion` phases are the ship
  *arriving back*; the moment `contract == None` again is *in drydock*.
- **The arc within a run — hope in, wear out.** A ship leaves port fresh and whole
  (pristine hull/life-support, high morale and unity, a population still close to the
  founders). Over the mission's decades the hull decays, spare parts are spent, and — new
  in v2 — the *people themselves drift* year over year: adaptation and cultural drift rise,
  legacy loyalty shifts toward the faction's pull, and the strain of a long voyage erodes
  morale and unity. The crew that comes home is measurably not the crew that left; the ship
  comes home "held together on hope and prayers." Legacy flavors the rate (Adaptors embrace
  the drift and change fastest; Preservers resist it; Wanderers wear it as identity).
- **Underway vs. in port — two toolsets, and the split is the point.** *During a mission*
  (`contract == Some`) the ship can only be kept limping: **field repairs** patch hull and
  life support using what the ship *carries* (spare parts, minerals) and never restore it to
  pristine, and a component **found on the voyage** (salvaged from a derelict or an event)
  *may* be field-installed — but only if the crew and the part allow it: a capable engineer
  aboard, a part that tolerates a field swap, and the spare parts to do it. There is no
  catalog shopping in the black. A long mission is therefore a slow problem of keeping a
  decaying ship alive on what you brought and what you find. *In port* everything opens up
  (next bullet).
- **Homecoming → drydock → cast off again.** On arrival the run resolves to a *Homecoming*
  summary (years elapsed, hull and population change since departure, reward banked). Only in
  **drydock** (`contract == None`) can the ship be fully set right, spending the reward on:
  - **Full repair** — restore hull integrity, life support, fuel, and spare parts to whole
    (port-only; the field kit underway can't reach pristine).
  - **Full loadout** — install any hull/engine/weapon from the whole component catalog
    (the existing Ship Builder), plus freely fit any parts salvaged underway. Port-only.
  - **Commission a new ship** — a large purchase that swaps the hull and returns the vessel
    to pristine condition with a one-time morale/hope lift, for when repairs can no longer
    keep the old hull flying. Port-only. A new *ship*, never new *people*: cultural drift does
    not reset — the dynasty and its changed population always carry forward.
- **What persists, what ends — carry on success, reset only on game-over.** When a run
  ends in *success* (the mission completes), the ship, its loadout, resources, and above all
  the **dynasty and its drifted population carry across** to the next mission — the people
  continue to build the legacy (they already live in the one saved `SimState`; unrepaired
  wear compounds run over run). The *only* thing that resets is **game-over: dynasty
  extinction**, which ends that run early and starts the player over with a **new ship and
  new people** (a fresh `SimState`). Renown still accrues to the Chronicle across all of it
  and can gate larger, richer charters for later runs (escalation, not reset).
- **Pacing — ~1 hour soft cap, ~30 min floor.** One successful mission run should pace to
  roughly **30–60 minutes** of real play: ~1 hour is a soft cap, and a run should *not* come
  in **under 30 minutes** — the only way a run ends sooner is game-over (extinction). Reached
  by tuning mission length (in-game years) against decision density (events, dilemmas) plus
  the drydock phase, **and** the real-time auto-advance cadence (`real_time.seconds_per_month`
  × the 1×/2×/3× selector — Pillar 4, superseded). A cosmetic wall-clock run timer surfaces so
  the floor/cap can be measured and felt; the mission must be sized so even brisk play at 3×
  cannot clear it in under ~30 minutes.

See PLAN.md "M4 — The Voyage-and-Return Refit Loop" for the code-grounded build order.

---

## 4. Player Role & Verbs

- **The player is:** the standing council of the ship — not any single character. No
  avatar, no player-character portrait or death.
- **The player directly controls:** resource allocation, ship component purchases, crew
  training/recruitment/heir designation, contract acceptance, event decisions, delegation
  settings, time advancement pace.
- **The player does NOT control:** individual non-leader crew/cohort members'
  day-to-day behavior (population and cohorts are simulated in aggregate), the exact
  timing or content of random events (rolled, weighted by ship/population state), an
  advisor's specific choice once a domain is delegated (only that a domain *is*
  delegated).
- **Core verb list:** *Allocate* (resources/crew), *Advance* (time), *Decide* (resolve an
  event/dilemma), *Delegate* (hand a domain to an advisor), *Build* (ship components),
  *Recruit/Train* (crew), *Select Heir*, *Accept/Abandon* (a contract), *Trade* (market).

---

## 5. Systems & Mechanics

### 5.1 Resource Economy

| Resource | Meaning | Notes |
| --- | --- | --- |
| Credits | General currency | Spent on components, crew, contracts |
| Energy | Ship power | Consumed by systems, gates exploration actions |
| Minerals | Raw material | Spent on components, colony development |
| Food | Population upkeep | Population penalty below a threshold |
| Influence | Political/reputation currency | Gates diplomacy/contract options |
| Population, Unity, Stability, Legacy Loyalty, Adaptation, Cultural Drift | Colony-scale simulation stats (0–1 or count) | Drive event weighting and dilemma risk (§5.3–5.4) |
| Hull Integrity, Life Support, Fuel, Spare Parts | Ship-condition stats (0–1 or count) | Below-threshold values increase crisis-event weight |

```text
per_turn_generated[resource] = floor(production_rate[resource] * years_elapsed)
production_rate starts from ship/component base rates + colony development bonuses
```

Carried over from the original, **with the fix applied**: colonizing a planet whose
resource list matches a tracked resource actually increases that resource's
`production_rate` (the original computed this bonus against a fresh object where the key
was never pre-populated, so the boost silently never applied — a genuine dead-code bug,
not a design choice, and the port initializes the rate table with all tracked keys up
front so the bonus lands).

### 5.2 Contract (Mission) Progression

| Field | Meaning |
| --- | --- |
| `objective` | mining / colonization / exploration / rescue |
| `target_duration` | years to reach 100% phase progress |
| `current_phase` | preparation → travel → operation → return → completion |
| `success_metrics` | weighted 0–1 targets (population survival, mission completion, resource efficiency, social cohesion) |

```text
success_score = Σ( min(1, metric.current / metric.target) * metric.weight )
score >= 0.9 -> complete
score >= 0.7 -> partial
score >= 0.4 -> pyrrhic
else         -> failure
```

Kept as-is from the original — this formula is sound and doesn't need redesign, only
more contract *content* (objective-specific milestones/risks — currently 2 base +
1 objective-specific milestone, 4 base + 1 objective-specific success metric, 3 base +
1 legacy-specific failure risk; see §8 for expansion targets).

### 5.3 Dynasty & Succession

```text
every 25 years:
  each dynasty member ages += 25
  if leader.age > 70:
      successor = eligible member (age 30-50) with highest leadership skill
      leader = successor
  add 1-3 new young members to the dynasty
```

Kept as-is — simple, legible, and already produces the "leaders retire, heirs inherit"
pillar from the original pitch. This is the mechanical heart of "Generational Command."

### 5.4 Event System

```text
event_chance = min(0.8, 0.3 + years_elapsed * 0.1 + (current_year / target_duration) * 0.2)

category weights:
  immediate_crisis      base, scales up when food<500, energy<1000,
                         hull_integrity<0.7, life_support<0.8,
                         morale<0.5, unity<0.4
  generational_challenge flat 0.3
  mission_milestone      0.4 near phase boundaries (progress>80% or <20%), else 0.15
  legacy_moment           min(0.1 + floor(year/25)*0.05, 0.3)
```

If an event doesn't require a player decision (or a domain has been delegated, §4), it
auto-resolves: each outcome is scored and the highest-scoring one is applied.

```text
outcome_score = food_weight(x2 if food<500) + hull/life_support penalty (x1000 if below threshold)
              + credits*0.1 + energy*0.2 + minerals*0.3 + morale*500 + unity*600
              - 100 per long-term negative consequence
              + legacy_specific_modifier * 200
```

**Content gap to close:** the original ships **4 hardcoded event templates** total
(`system_failure`, `population_growth`, `arrival_at_target`, `cultural_schism`) for a
system meant to carry a century-scale campaign. See §8 for the port's target count.

### 5.5 Legacy Factions & Dilemmas

Three player-selected legacies, each with a distinct dilemma set and failure condition:

| Legacy | Dilemma flavor | Failure risk name |
| --- | --- | --- |
| Preservers | Tradition vs. adaptation pressure | `cultural_collapse` |
| Adaptors | Genetic/cybernetic/biological modification (weighted success rolls, e.g. 70%/75%/60%) | `humanity_loss` |
| Wanderers | Raiding/piracy (payout scales with a severity roll: `1000 + severity*2000` success vs. `-500` failure) | `fleet_dissolution` |

```text
risk += 30 if culturalDrift > 0.7
risk += 25 if unity < 0.3
risk += 35 if tradition_points < 20
at-risk if total_risk > 50
```

Kept — the math is real and good. **Fixed in the port:** the original hardcoded several
of these inputs (`traditionPoints`, `bodyHorrorEvents`, `existentialDread`,
`piracyReputation`) as constants that were never actually updated by play (e.g.
`traditionPoints` was always 50). The port tracks these as real per-dynasty/per-legacy
state, updated by the relevant dilemmas and events, so the failure-risk system means
something.

### 5.6 Randomness & Determinism

- **What's randomized:** event category/outcome selection, dilemma success rolls, random
  crew/dynasty-member generation, planet generation.
- **What must stay deterministic:** the whole simulation tick, for saves/replay/tests.
  The original's `Phase1SimulationService` already modeled this correctly with a seeded
  LCG (`state = (1664525*state + 1013904223) >>> 0`) — the port standardizes on the
  toolkit's `rng` module seeded the same way, isolated from `macroquad::rand`, per
  `CODE_STANDARDS.md` §5.

---

## 6. Data Model (`assets/*.json`)

```json
// assets/ship_components.json — hulls/engines/weapons and their cost/stat deltas
{
  "hulls": [{ "id": "light_corvette", "cost": { "credits": 500 }, "stats": { "cargo": 100, "crew_capacity": 6 } }],
  "engines": [{ "id": "ion_drive", "cost": { "credits": 800, "minerals": 50 }, "stats": { "speed": 2 } }],
  "weapons": [{ "id": "pulse_cannon", "cost": { "credits": 600 }, "stats": { "combat": 3 } }]
}
```

```json
// assets/events.json — weighted event templates by category, with per-outcome effects
{
  "id": "reactor_breach",
  "category": "immediate_crisis",
  "legacy_modifiers": { "adaptors": 1.2 },
  "outcomes": [
    { "id": "emergency_vent", "resource_delta": { "energy": -200 }, "population_effect": { "morale": -5 } }
  ]
}
```

```json
// assets/legacies.json — the 3 legacy factions, their dilemmas and failure-risk weights
{ "id": "preservers", "dilemmas": ["...":"..."], "failure_risk": "cultural_collapse" }
```

| File | Defines | Loaded via |
| --- | --- | --- |
| `assets/ship_components.json` | Hulls/engines/weapons + cost/stat deltas | `data_loader::load_json_file` |
| `assets/events.json` | Event templates, categories, outcomes, legacy modifiers | `DataRegistry` |
| `assets/legacies.json` | 3 legacy factions, dilemma sets, failure-risk weights | `DataRegistry` |
| `assets/dynasty_names.json` | Name pools, specializations, traits per legacy | `data_loader::load_json_file` |
| `assets/contracts.json` | Objective templates: milestones, success metrics, failure risks | `DataRegistry` |
| `assets/crew_archetypes.json` | Roles, background strings, starting skill ranges | `data_loader::load_json_file` |
| `assets/data/texture_manifest.json` | (empty/minimal — no sprite art; kept for toolkit consistency) | `AssetManager` |
| `assets/data/game_config.json` | Tunable constants: tick costs, thresholds, notification limits | `data_loader::load_json_file` |

Native builds read `assets/` from disk with an `include_str!` fallback for WASM — this
game has no hot-reload requirement beyond normal balance iteration, so embed-only
(like `template/`) is sufficient; no need for live disk reload during play.

---

## 7. World & Progression Structure

- **World layout:** no rendered map/tilemap. A short list of systems relevant to the
  active contract (origin, waypoints, destination) presented as a panel, not a
  `FlatGrid`/camera-driven view. If a literal starmap ever gets added it's a simple
  static node layout via `math`, not a scrolling `camera`-driven world.
- **Session/campaign length:** one contract spans decades to a century of in-game years;
  a full playthrough spans multiple contracts and multiple dynasty generations, ending
  only at dynasty extinction or player retirement.
- **Progression stages:**

  | Stage | Trigger | What changes |
  | --- | --- | --- |
  | Preparation | Contract accepted | Resource/crew allocation phase, no time pressure |
  | Travel → Operation → Return | Time advances | Event rolls active, milestones progress |
  | Completion | `current_year >= target_duration` | Success level computed (§5.2), Chronicle entry recorded |
  | Dynasty extinction / retirement | No eligible heir, or player choice | Playthrough ends, Heritage modifiers offered for a new save |

- **Save/persistence model:** local save slots via `macroquad_toolkit::persistence`
  (`save_to_slot_with_version` / `load_from_slot_with_migration`) — this fully replaces
  the original's server-side JSON-blob store; there is no server-authoritative state to
  preserve (§0). Chronicle history (for Heritage modifiers) persists alongside the save
  slots as its own versioned file, since it must survive across separate playthroughs/
  save slots rather than living inside one slot.

---

## 8. Content Inventory

| Content type | Prototype target | Full target |
| --- | ---: | ---: |
| Event templates (across 4 categories) | 12 | 30+ |
| Ship hulls / engines / weapons | 3 / 3 / 3 (as original) | 5 / 5 / 5 |
| Contract objective templates (mining/colonization/exploration/rescue) | 4 (as original) | 6–8 |
| Legacy factions | 3 (fixed — Preservers/Adaptors/Wanderers) | 3 |
| Per-legacy dilemmas | 3–4 each (as original) | 6 each |
| Dynasty name/specialization/trait pools | as original (10 names/legacy, 10 specializations, 5 traits/legacy) | roughly doubled |
| Crew roles | 7 (as original) | 7 |
| Heritage modifier tiers | 4 (minor/moderate/major/legendary, as original) | 4 |

The original's content is thin almost everywhere except the mechanics that consume it
(4 event templates trying to carry a century-scale campaign is the clearest gap) — the
full targets above are sized to make repeat playthroughs not feel like the same 4 events
on loop, not to invent new systems.

---

## 9. UI/UX & Screen Flow

| Screen | Purpose | Toolkit pieces |
| --- | --- | --- |
| Main Menu | New/continue/load slot, settings | `VirtualUi`, `SurfaceStyle`, buttons |
| Dashboard | Resources, ship status, crew summary, pending-event banner | `GridLayout`, meters, badges, `NotificationManager` |
| Ship Builder | Hull/engine/weapon purchase + current loadout | `GridLayout`, `TextStyle`, tooltips |
| Crew & Dynasty | Roster, train/recruit/heir actions, dynasty detail (merges the original's separate Crew Quarters + Dynasty Hall — dynasty actions are now real, not cosmetic, see §0) | `ScrollTabs`, meters |
| Contract & Systems | Active contract progress/milestones, relevant systems for the current journey (replaces the original's sprawling 50-system galaxy map, §0) | `GridLayout`, progress meters |
| Market | Buy/sell the 4 tradeable resources with price trend | `TextStyle`, simple table layout |
| Event/Decision modal | Resolve a triggered event or dilemma; shows outcome preview | `NotificationManager` / modal surface |
| Chronicle & Heritage | End-of-playthrough summary, Heritage modifier selection for a new save | `ScrollTabs`, `TextStyle` |
| Pause/Settings | Time-advance pace, delegation toggles, save/load | `VirtualUi` |

Interaction flow (mirrors `realmseed/gdd.md`'s numbered turn structure):

1. Player reviews Dashboard; if a decision-required event is pending, it interrupts here.
2. Player allocates the turn (Ship Builder / Crew & Dynasty / Market as needed).
3. Player presses Advance Time; the tick runs (§5), producing resource/population deltas
   and possibly a new event.
4. If the event needs a decision, the modal opens and blocks further advancement until
   resolved (or handled by a delegated advisor, which logs the outcome without
   blocking).
5. On contract completion, flow moves to Chronicle for the summary, then back to
   Dashboard for the next contract.

---

## 10. Toolkit Mapping

| Need | Toolkit module | Using it? | Notes |
| --- | --- | --- | --- |
| Input handling | `input` | Yes | Menu/button interaction only — no world input |
| Widgets/layout/text | `ui` (`VirtualUi`, `GridLayout`, `SurfaceStyle`, `TextStyle`, meters, badges, tabs, scroll) | Yes | Carries nearly the entire UI; this is a text/panel game |
| Textures/manifest | `assets` (`AssetManager`) | Minimal | No sprite art (§0); manifest kept minimal/empty for consistency |
| Camera/pan/zoom | `camera` | No | No rendered world/map (§7) |
| Cross-system messaging | `events` (`EventBus<UiAction>`) | Yes | Event/dilemma resolution, delegation toggles |
| Palette | `colors` | Yes | Reuse the original's amber/green/red terminal palette values |
| Vector/grid math | `math` | Maybe | Only if a literal starmap node layout is added |
| Frame timing | `timing` | Yes | Drives the real-time month auto-advance while under way (Pillar 4, superseded); frozen while docked |
| Particles/juice | `fx` | Minimal | A terminal-log game has little use for particles; maybe subtle text-flicker/typewriter effects |
| User settings | `settings` | Yes | Time-advance pace, delegation defaults |
| Unlocks/achievements | `achievements` | Maybe | Could back Chronicle milestones |
| Dev overlay | `debug` | Yes | Standard |
| Deterministic randomness | `rng` | Yes | Seeded LCG per §5.6 |
| Sprite animation | `sprite` | No | No sprites |
| Procedural images | `raster` | No | Not needed given the text-only presentation |
| Headless screenshot capture | `capture` | Yes (required for every game) | See `docs/screenshot_capture_harness_guide.md` |
| Save/load | `persistence` (`save_to_slot_with_version`, etc.) | Yes | Replaces the original's server-side save entirely (§7) |
| Tile grid / fog / pathing | `FlatGrid`, `FogState`, line-of-sight, flood-fill | No | No spatial world to model |

The near-total lean on `ui` with almost nothing from `camera`/`sprite`/`raster`/`FlatGrid`
is a direct consequence of the original having no world-rendering surface at all — this
is the leanest toolkit footprint of any port in this catalog, not a gap to fill.

---

## 11. Architecture Skeleton

```
src/
├── main.rs
├── game.rs             # Game struct, update()/draw() loop
├── state.rs             # GameState enum + re-exports
├── state/
│   ├── menu.rs
│   └── gameplay.rs       # active save: dashboard/contract/dynasty/event-modal sub-state
├── data.rs               # data module root
├── data/
│   ├── ship_components.rs
│   ├── events.rs
│   ├── legacies.rs
│   ├── contracts.rs
│   └── crew.rs
├── simulation.rs         # stateless services root
├── simulation/
│   ├── tick.rs           # per-year resource/population/dynasty advance
│   ├── succession.rs     # dynasty aging + heir selection (§5.3)
│   ├── event_resolver.rs # event roll, outcome scoring, dilemma resolution (§5.4-5.5)
│   ├── contract.rs       # success-metric scoring (§5.2)
│   └── market.rs         # trade validation
├── chronicle.rs          # cross-playthrough summary + heritage modifiers (§7)
├── ui.rs
├── ui/
│   ├── dashboard.rs
│   ├── ship_builder.rs
│   ├── crew_dynasty.rs
│   ├── contract_systems.rs
│   ├── market.rs
│   ├── event_modal.rs
│   └── chronicle.rs
└── save.rs
```

- **`GameState` variants:** `Menu`, `Gameplay` (with an internal screen enum matching
  §9's tab list), `EventModal` (or nested inside `Gameplay` as a blocking sub-state).
- **Key stateless services (`simulation`):** `tick::advance_year`,
  `succession::process_generation`, `event_resolver::roll_and_resolve`,
  `contract::score_success`.
- **State that must persist across frames but never leak into UI:** the RNG stream
  state, the active contract's full `ExtendedResources`/`Population`/`Dynasty` data —
  UI panels read snapshots via accessors, never mutate directly (per
  `CODE_STANDARDS.md` §7's `UiAction` pattern).

---

## 12. Non-Goals / Open Questions

- **Explicitly not building (v1):** the Legacy Deck card meta-game, any server/account
  system, a literal scrolling galaxy map, real-time background simulation, per-character
  portraits or any commissioned art.
- **Differentiation note:** this sits closest to `apartment` (Second Story) in the
  catalog — both are succession/portfolio management sims. The differentiator is scope
  of "portfolio": `apartment` manages a growing set of *buildings* with independent
  tenants; Stellar Legacy manages *one vessel's bloodline* across centuries — no
  building-to-building comparison shopping, no multi-property expansion loop. If a
  system ever starts to feel like "manage N independent colonies," that's a signal it's
  drifted into `apartment`'s or `realmseed`'s territory and should be pulled back.
- **Open questions**, in priority order:

  1. Does a literal (even minimal) starmap node view earn its keep, or does the
     Contract & Systems panel stay a plain list permanently? Leaning list-only per
     Pillar 3 (don't build a panel that implies more depth than exists) unless multi-leg
     journeys become a real mechanic.
  2. Should the Legacy Deck concept return post-v1, or is its role (narrative color from
     past decisions) fully covered by the redesigned Chronicle/Heritage system? Leaning
     toward "Chronicle covers it" — revisit only if playtesting shows the Chronicle
     summary feels thin.
  3. Exact automation/delegation UI — how many domains can be delegated simultaneously,
     and does delegating everything trivialize the "century-scale" pacing goal?
  4. **(v2, §3.1) RESOLVED 2026-07-19 — owner confirmed the persistent-ship model.** A run
     is one mission on a persistent ship; on success the people/dynasty carry across and
     continue to build the legacy, and only game-over (extinction) resets to a new ship and
     new people. This needs no new persistence channel (the ship already lives in one saved
     `SimState`; "between missions" is `contract == None`). Also settled: ~1 hour is a soft
     cap and a run has a ~30-minute floor (sub-30 only via game-over); and the repair/loadout
     split — **underway** allows only field repairs (from carried consumables, never to
     pristine) and gated install of parts *found* on the voyage (if crew + part allow),
     while **full repair, full loadout, and commissioning a new ship are port-only**.

---

## 13. Milestones

| Milestone | Proves | Target content |
| --- | --- | --- |
| M1 — Mechanical proof | One contract playable start-to-finish: allocate → advance → event → succession → completion scoring, with placeholder data | 1 contract objective, 4 event templates (parity with original), 1 legacy |
| M2 — Playable prototype | Full turn loop with delegation, market, ship builder, dynasty actions all wired to real state (no cosmetic no-ops); a full playthrough can end in dynasty extinction and produce a Chronicle entry | Prototype content targets from §8 |
| M3 — Content-complete | All 3 legacies, full event/contract library, Heritage modifiers feeding a second playthrough, terminal UI/palette fully ported | Full targets from §8 |

After M1, follow the standard per-game loop: `cargo clippy --all-targets --all-features -- -D warnings`,
`cargo test`, then `.\publish.ps1` from the game directory to verify at the shared preview
root — same validation path as every other game in this repo, no exceptions for being new.
