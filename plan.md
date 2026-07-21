# Stellar Legacy — Generational-Voyage Redesign (v3)

## Context

`stellar_legacy` is a mature, content-complete generation-ship sim (all M1–M4 done per
`PLAN.md`). But two things diverge from the owner's actual intent, and the owner traces this
to under-planned documentation:

1. **Missions are far too short.** Charters run **22–60 in-game years** today. The owner wants
   **each mission to be at least ~300 years** — a true generational voyage where *multiple
   human lifetimes pass in transit, with no cryosleep*: the ship carries a whole living
   civilization, and the crew/family that comes home is measurably **not** the one that left.
2. **There is no explicit launch / the mission is where the depth should live.** The
   between-missions "drydock" Contract screen (pick a charter) exists, but accepting a charter
   *silently* starts it (`AcceptContract` in `src/game/actions.rs:154`). There is no felt
   "prepare, then **LAUNCH**" moment, and the long voyage that follows is thin.

The owner wants to **plan this before any development**. This document is that plan plus the
requested **event-design notes** distilled from `event_notes.md`. (The prior M1–M4 framework
handoff that lived in this file is preserved in `PLAN.md`.)

### The owner's vision (from clarification, 2026-07-21)

- Player **picks a mission**; the ship departs on it. Missions are **≥300 years**, **no
  cryosleep**, **multi-hour** to play.
- A civilization lives and turns over aboard during the trip; **the people are changed by the
  voyage** (this already exists in miniature as M4.1 "voyage drift").
- **The ship returns** ("assuming the player makes it back") — this is a **round-trip
  expedition**, NOT one-way colonization. So the existing **voyage-and-return refit loop is the
  right frame** — it just needs to be scaled up and given real depth.
- Missions are **internally phased**, often: *travel ~100 yr → work on-station ~100 yr (e.g.
  mine a site) → travel back ~100 yr*. Some missions may stay in a location for a long stretch.
- Goal is still **do the mission and earn lots of money.**
- On return, the reward buys **reactive upgrades** — e.g. after a major shipboard illness, you
  return and **strengthen the medical bays** so the next voyage weathers it better.
- **The player makes the large decisions and sees the effects** ripple across generations.

### What already exists that we build ON (not a rewrite)

- **Phase backbone is already modeled:** `ContractPhase::{Preparation, Travel, Operation,
  Return, Completion}` (`src/data/contracts.rs:29`) maps *exactly* to travel→work→return. Today
  it's **cosmetic** — derived from the progress fraction (`for_progress`, 0–0.2 Travel /
  0.2–0.8 Operation / 0.8–1.0 Return). We make it **mechanically real**.
- **Persistent ship + drydock refit loop** (M4) already carries the `SimState` across missions,
  with full repair / full loadout / commission gated to port (`contract == None`).
- **Voyage drift** (M4.1, `simulation/tick.rs::apply_voyage_drift`): the people already diverge
  from the founders year over year, legacy-scaled.
- **Generational succession** every 25 yr (`generation_interval_years`), retirement at 70,
  extinction = game-over.
- **Data-driven content** in `assets/*.json`; deterministic seeded RNG; UI is a pure view layer
  pushing `UiAction` (dispatch in `game.rs`); events roll/score/resolve in
  `simulation/event_resolver.rs`.

### The core gaps to close

| Gap | Today | Target |
| --- | --- | --- |
| Mission length | 22–60 yr | ≥300 yr, phased travel→operation→return |
| Pacing | 1 year per manual Space-press (≈300 clicks/mission) | month→10yr speed selector, dated events |
| Launch | charter click = silent start | explicit **pre-launch prep → LAUNCH** commit |
| Phase mechanics | cosmetic % label | real phases with phase-specific events & actions |
| Reactive upgrades | hull/engine/weapon slots only | ship **subsystems/modules** (medical, etc.) that buffer event families |
| Event content | 46 events, generic | families from `event_notes.md`, phase- & length-aware |

---

## Event-System Design Notes (distilled from `event_notes.md`)

*Requested deliverable. `event_notes.md` is a brainstorm catalog inspired by Star Trek's
"catalog of things that happen to a ship." It is **not final** — specific content is
placeholder (e.g. "Tribbles" → rename to an in-universe "Lobites" or cut). These notes convert
it into a system.*

### Principle: generate scenarios from **families × complications × outcomes**, not one-offs

`event_notes.md` itself recommends this (its closing table). An event = a **family** +
optional **complication/twist** + **outcome branches**. This lets a handful of authored
families cover a centuries-long voyage without visible repetition — critical now that a single
mission spans 300+ years and dozens of decisions.

### Event families (map onto / extend the current `EventCategory`)

Current categories: `ImmediateCrisis`, `GenerationalChallenge`, `MissionMilestone`,
`LegacyMoment` (`src/data/events.rs`). The catalog's families are richer; proposal is to keep
the 4 categories as the **scoring/weighting axis** and add a **`family` tag** on each event
template for content organization + phase gating:

| Family | Source sections in notes | Maps to category | Notes |
| --- | --- | --- | --- |
| **Exploration / First Contact** | Friendly explorers, suspicious civs, primitive worlds, ruins, anomalies | LegacyMoment / MissionMilestone | Mostly **Travel**-phase; big branching decisions |
| **Diplomacy** | Trade, war, refugees, negotiations, tribute demands | GenerationalChallenge | Travel & on-station; ties to influence/piracy_reputation |
| **Engineering** | Warp/reactor faults, coolant, hull stress, gravity failure | ImmediateCrisis | Any phase; **buffered by ship subsystems** (below) |
| **Biology / Medical** | Disease, sleep epidemic, mutation, parasite, plague | ImmediateCrisis / GenerationalChallenge | **The medical-bay upgrade loop lives here** |
| **Science / Anomaly** | Temporal rifts, wormholes, spatial anomalies, new particles | LegacyMoment | Travel-phase; risk/reward, can shift voyage length |
| **Survival** | Scarcity, morale crises, environmental hazards (radiation, ion storm) | ImmediateCrisis | Pressure-tests provisioning |
| **Mystery** | Ghost signals, derelicts, impossible artifacts | MissionMilestone | Feeds salvage/found-parts (existing M4.4 hook) |
| **Comedy** | Bureaucrats, pranksters, infestations, translation mishaps | LegacyMoment | Tension-breakers; low stakes; **"Lobites" placeholder** |
| **Ethics / Moral dilemma** | Save crew vs colony, honor treaty, answer distress call | GenerationalChallenge | The "soul" — reuse the existing **legacy dilemma** modal |
| **Legacy / Generational drift** | Mission forgotten, factions split, mission becomes religion, descendant cultures | GenerationalChallenge | **Only makes sense at century scale** — the headline new family |

### The "Long-Term Expedition" family is the signature of this redesign

`event_notes.md` §"Long-Term Expedition Events" lists exactly the beats a 300-year no-cryo
voyage unlocks and short missions cannot:

- A generation awakens to find the home civilization changed/vanished; the mission is now
  obsolete or reinterpreted.
- Successive generations turn the mission into a **religion**; the ship **splits into factions**;
  **multiple descendant cultures** emerge aboard.
- The onboard AI becomes the keeper (or subtle rewriter) of centuries of memory.
- Descendants choose to settle a world and **leave the mission understaffed** (population drain).

These should be **gated by `year`/generation count and by voyage-drift state** (fire when
`cultural_drift`/`adaptation` cross thresholds), so they read as *consequences of the long
voyage*, not random rolls. They hook naturally into the existing `LegacyTrack` counters and the
M4.1 drift stats.

### Phase-aware event weighting (new)

Because missions become phased (travel→operation→return), each event template should declare
**which phases it can fire in** so content fits context:

- **Travel:** Exploration, First Contact, Anomaly, Diplomacy, deep-space Engineering.
- **Operation (on-station):** objective-specific work events (mining hazards, colony setup,
  survey finds, rescue complications), local Diplomacy, Survival.
- **Return:** homecoming beats, "the mission is now obsolete," reintegration, cargo/escort risk.
- **Any:** Biology/Medical, core Engineering, Legacy/Generational drift.

### Content authoring model (implementation-time)

- Keep everything in `assets/events.json` + a new brainstorm/notes file `event_design_notes.md`
  (this section, committed to the repo, so the catalog + placeholders live alongside the game).
- Add optional fields to the event template schema (all `#[serde(default)]`, back-compatible):
  `family: String`, `phases: Vec<ContractPhase>` (empty = any), `min_year`/`min_generation`
  gates, and a `subsystem` tag (which ship module buffers it).
- Rename/replace placeholder flavor (Tribbles → in-universe name or cut) as content lands.

---

## Workstreams

*Ordered; each is a shippable increment following the repo's build/verify loop. Numbers are
first-pass and belong in `assets/data/game_config.json`, not Rust.*

### W1 — Rescale missions to generational length (data-only first)
- Raise `target_duration_years` on charters to **300–600 yr** bands in `assets/contracts.json`.
- Re-tune `voyage_drift`, `hull_decay_per_year`, `life_support_decay_per_year`, parts upkeep,
  and provisioning so a 300-yr voyage is *survivable but demanding* (today's decay numbers were
  tuned for ~55 yr and would destroy a 300-yr ship). Extend the soak test to ~400 yr.
- Verify the succession/extinction math holds over 12+ generations.

### W2 — Real mission phases (travel → operation → return) with early truncation
- Promote `ContractPhase` from a cosmetic %-derived label to **authored phase segments** on the
  template: e.g. `phases: [{kind: Travel, years: 100}, {kind: Operation, years: 100}, {kind:
  Return, years: 100}]`. Progress/metrics/events become phase-aware.
- On-station (Operation) is where the objective work happens (mining yield, colony build,
  survey) — gate objective progress + reward accrual to that phase.
- **Mission length is fixed by the charter** (authored phase segments — the player shops
  between charters, they don't tune phase lengths at prep), but the voyage can **end early**
  (owner decision, 2026-07-21):
  - **Bad-event truncation** — the resource turns out to be unavailable, a disaster cripples
    the ship, the mine collapses: Operation is cut short, the ship jumps straight to Return.
  - **Fortunate truncation** — an event surfaces something *more valuable* than the charter
    objective; taking it can also send the ship home early.
  - **Player [ ABORT / TURN BACK ] verb** — the player may turn back at any time.
  - **Pay is strictly proportional to measured objective progress** — mined X of Y, built X of
    Y, explored X of Y. **No progress = no pay.** This requires every charter objective to be
    a **quantified counter** that accrues during Operation, not a timer.
  - **Total loss (extinction) remains possible.** Failure is a spectrum: full success →
    partial (early return, prorated pay) → total loss.

### W3 — Month-resolution clock + speed selector (dated events)
*Owner decision: a **speed selector spanning 1 month (slowest) → 10 years (fastest)**, and
**every event carries the specific date it triggered.** This requires sub-year time.*
- **Date/time model:** replace the bare `sim.year: u32` with a **month clock** (e.g.
  `month_clock: u32`, `year = month_clock / 12`, `month = month_clock % 12`). Display a mission
  clock in the CRT header (e.g. `Y143 · M07`). `LogEntry`/`PendingEvent` gain a `month` (or a
  packed date) so every event/log line is dated. **Existing saves are abandoned** (owner
  decision, 2026-07-21) — no migration or serde back-compat shims for this redesign.
- **Speed selector** with steps roughly `[1 mo] [1 yr] [5 yr] [10 yr]`; pressing Advance
  fast-forwards up to that span but **hard-stops the instant a decision fires** (event,
  dilemma, generation turnover, phase boundary, crisis threshold), stamping the exact date.
  Pillar 4 holds — time only moves on an explicit Advance; one press can now cover many months.
- **Tick granularity (owner-ratified 2026-07-21: the game advances in months, player can
  speed up):** the advance loop steps **month by month**; roll & **date events at month resolution**, but apply the existing
  **economic tick (production, decay, drift, aging, succession) on year boundaries** so the
  tested `advance_year` math and the 25-yr `generation_interval_years` stay intact. This gives
  month-precise event dates with minimal churn to the soak-tested tick. (Alternative: fully
  per-month effects — more faithful but re-tunes every rate and the whole test suite.)

### W4 — Pre-launch prep phase + explicit LAUNCH
- Reframe the drydock/Contract screen into an explicit **PREP** state ending in a committing
  **[ LAUNCH ]** button that replaces the silent `AcceptContract` (`game/actions.rs:154`).
  Departure is the felt moment; LAUNCH locks the loadout for the voyage.
- **Prep depth (owner decision — build these):**
  - **Provisioning** — stockpile food / spare parts / fuel sized against the chosen voyage
    length (extends today's `spare_parts`/upkeep model to food & fuel stores). Owner-specified
    failure model: **food runs out → starvation deaths**; **fuel runs out → subsystems get shut
    down and the ship may fail to reach its destination**. Shortages also **generate
    opportunity events**, not just attrition — e.g. running low on food surfaces a garden-world
    stop: resupply food, but a segment of the population settles there and leaves the mission
    (ties into the Long-Term Expedition family and faction loss, W7). Skipping the stop means
    weathering the starvation losses instead.
  - **Ship subsystems** — outfit the module family from W5 before departure.
  - **Destination & charter** — pick destination (distance ⇒ voyage length); the on-station
    stay is **fixed by the charter's authored phases** (W2), not a prep-time slider. The
    time/risk-vs-reward trade lives in the choice *between* charters.
  - **People — founding factions (owner decision, 2026-07-21: Frostpunk-style).** The game
    ships **6 authored factions** (`assets/factions.json`) spread across an ideological
    spectrum — tech-embracing ↔ tech-averse and points between. On a **new game** the player
    **picks 3 of the 6** as the founding population (replacing the pure-RNG `founding_dynasty`
    roll in `state/sim.rs:440`). Factions can be **lost during a voyage**; if the ship returns
    with only 2, drydock offers **recruiting another group** from the remaining pool. On a
    continuing mission the people persist; **Recruit** adds crew/population (reusing
    `simulation/crew.rs`). Full faction system: **W7**.

### W5 — Ship subsystems / reactive upgrade loop (full module family)
*Owner decision: **full module family**, not a lean set.*
- Add a **module/subsystem** layer beyond hull/engine/weapon, each with upgrade tiers:
  **medical bay, life-support/habitat, agriculture/food, security, education/culture,
  engineering bay**. Each **buffers a specific event family** — medical bay ↓ severity/odds of
  Biology events, security ↓ Diplomacy/boarding/mutiny, agriculture ↓ Survival/famine,
  education/culture ↓ Legacy/cultural-drift crises, engineering ↓ Engineering faults,
  life-support/habitat ↑ population capacity & ↓ life-support decay.
- Wire each subsystem's tier into the relevant roll/severity in `event_resolver.rs` and the
  tick (mirrors how M4.1/M4.2 already scale effects by config).
- **Subsystems decay like everything else on the ship** (owner decision, 2026-07-21): modules
  take wear/damage over the voyage and need repairs en route — the upgrade loop is not
  ratchet-only. **Repair capability is gated by living expertise**: knowledge lives in people,
  and **if everyone who understands a system dies, it cannot be repaired** until the knowledge
  is rebuilt. **Knowledge model (owner decision): a per-subsystem institutional-knowledge
  stat** carried by the population — raised by education/training (the education/culture
  subsystem transmits it across generations), decaying when experts die untrained; repair
  capability gates on it.
- On return, reward is spent in drydock to **upgrade the subsystem that hurt you** — closing the
  "illness → strengthen medical bays" loop. Reuses the port-only purchase gate + Ship Builder UI
  pattern. New `assets/*.json` subsystem catalog + a `subsystems` config block.

### W6 — Event content build-out from the families above
- Author the family catalog + phase gates + Long-Term Expedition beats into `assets/events.json`
  and `event_design_notes.md`. Replace placeholder flavor.
- **Volume target (owner, 2026-07-21): 6–10 events per family is enough** — where a family
  already has ≥10, keep them. **Structure before content depth**: build the family/phase/gating
  schema and generation machinery first; the catalog can grow later.
- Each mission should play like a **campaign generated over the course of the journey** — the
  family × complication × outcome generator plus year/drift-gated Long-Term beats produce a
  narrative arc per voyage, not a flat random-event stream.
- **Generation timing (owner decision): seeded skeleton at LAUNCH.** The mission's major beats
  are laid out deterministically from the mission seed when the player launches;
  reactive/filler events roll as time advances. Same seed ⇒ same campaign, keeping the
  automated playthrough harness reproducible.

### W7 — Founding factions (Frostpunk-style population groups)
*Owner decision (2026-07-21): factions replace "pick individual founders."*
- **6 authored factions** in `assets/factions.json` — identity, position on an ideological
  spectrum (tech-embracing ↔ tech-averse and everything between), biases, flavor. Data-driven
  like all other content.
- **Pick 3 of 6 at new game** as the founding population; population segments belong to
  factions.
- **Factions can be lost mid-voyage** via any of (owner-confirmed): population wiped out
  (starvation/disease/disaster), **settles off-ship** (garden-world stop, descendant-settlement
  beats), **schism/departure** (ideological split or mutiny), or **soft assimilation** (drift
  merges a small faction into a dominant one over generations). **Returning with fewer than 3**
  lets the player recruit **another group from the remaining pool** in drydock.
- Factions feed the Legacy/Generational-drift event family (faction splits, descendant
  cultures, mission-becomes-religion) and event reactions.
- **v1 depth (owner decision): structure first** — factions as data, population membership,
  loss conditions, and event/drift coloring. No approval meters or stat modifiers in v1; the
  schema should leave room for them to layer on later.

---

## Resolved Decisions (owner, 2026-07-21)

1. **Core loop** — keep the **return-refit loop**, but each mission is **≥300 yr, multi-hour**,
   internally **phased** (travel → operation → return); the ship **comes home** and refits.
2. **Pacing** — **speed selector, 1 month (slowest) → 10 years (fastest)**; **every event is
   stamped with its exact date** ⇒ month-resolution clock (W3).
3. **Prep depth** — build **provisioning + ship subsystems + destination/charter**; **pick
   founders on a new game**, **recruit** on a continuing mission (people persist).
4. **Subsystems** — **full module family** (medical, life-support/habitat, agriculture, security,
   education/culture, engineering), each buffering an event family.
5. **Failure** — **graduated**: a big setback can **truncate the on-station phase and return
   early for reduced reward** (mine-collapse example); **total loss (extinction) still possible**.

### Round 2 (owner, 2026-07-21)

6. **Mission length** — fixed per charter; the voyage can end early via bad events, fortunate
   finds, or a player abort. **Pay is strictly proportional to quantified objective progress**
   (mined/built/explored X of Y); no progress = no pay.
7. **Tick granularity** — ratified: the game **advances in months**, with the speed selector on
   top (W3's month-events / year-economics hybrid accepted).
8. **Saves** — **ignore existing saves**; no migration or back-compat work.
9. **Build order** — interim partial playability is fine; **most playtesting happens after it
   can be automated** ⇒ an automated full-mission playthrough harness is a first-class
   deliverable, not an afterthought.
10. **Founders** — Frostpunk-style **factions**: 6 authored groups across a tech↔tradition
    spectrum, player picks 3; return with only 2 → recruit another group (W7).
11. **Provisioning** — no food → starvation deaths; no fuel → systems shut down / may not reach
    the destination; shortages also generate opportunity events (garden-world stop: food gained,
    settlers lost).
12. **Decay** — the whole ship decays and needs repair; **repair requires living expertise** —
    if everyone who knows a system dies, it can't be fixed until knowledge is rebuilt.
13. **Pacing** — each mission plays like a **campaign generated over the course of the journey**.
14. **Content volume** — 6–10 events per family suffices (keep existing where already ≥10);
    **structure first, content depth later**.
15. **HARD RULE — data-driven design holds everywhere.** Missions, phases, factions,
    subsystems, provisioning rates, and events are all authored in `assets/*.json`.
    **Missions must never be hardcoded in Rust.**

### Round 3 (owner, 2026-07-21)

16. **Faction depth (v1)** — **structure first**: factions as data + population segments +
    loss/recruit, coloring events and legacy drift. No approval meters or stat modifiers yet;
    those layer on later.
17. **Faction loss** — **all paths in play**: population wiped out, settles off-ship,
    schism/departure, and soft assimilation via generational drift.
18. **Knowledge model** — **per-subsystem institutional-knowledge stat** carried by the
    population: raised by education/training, decays when experts die untrained; repair
    capability gates on it.
19. **Campaign generation** — **seeded skeleton at LAUNCH**: major beats laid out
    deterministically from the mission seed, reactive/filler events rolled as time advances.
    Same seed ⇒ same campaign (keeps the automated harness reproducible).

### Implementation briefs

Self-contained, execution-ready briefs for each workstream live in **`tasks/`**
(`tasks/README.md` for order and ground rules). They merge the decisions above into
per-workstream instructions with exact files, schemas, and acceptance criteria — an
implementing agent should work from a brief, not from this document.

### Suggested build order

W1 (rescale, data-only, de-risks everything) → W3 (month clock + speed, since dated events
touch core state) → W2 (real phases + early truncation) → W7 (factions, so prep has groups to
pick) → W4 (prep + LAUNCH) → W5 (subsystem family + reactive upgrades) → W6 (event content
from the families). W1 and the W6 event catalog can proceed in parallel with the middle work.
Stand up the **automated full-mission playthrough harness** alongside W1/W3 — the owner
expects most playtesting to run automated, so it should mature with the systems it tests.

## Verification (for the eventual build)

- `cargo test -p stellar_legacy` (extend the long-campaign soak to ~400 yr; keep it green).
- **Automated mission playthrough**: a headless harness that plays an entire ≥300-yr charter
  end to end (prep → launch → phases → return/refit) making policy-driven choices and asserting
  survival/economy/pacing invariants — the owner's primary playtest channel before human passes.
- `cargo clippy --all-targets --all-features -- -D warnings`; `cargo fmt -p stellar_legacy -- --check`.
- `cargo check --release --target wasm32-unknown-unknown` (WASM).
- `.\scripts\capture_ui.ps1 -Scenes menu,prep,gameplay,contract_active,drydock` for visual checks.
- `.\publish.ps1` then verify at `http://127.0.0.1/stellar_legacy/`.
- **Human playtest** to confirm a full mission paces to multiple hours and the generational
  arc *feels* like lifetimes passing.
