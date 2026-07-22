# Event-System Design Notes

*Living content-authoring reference (W6). Distilled from `plan.md` and the raw
brainstorm in `event_notes.md`. `event_notes.md` remains the unstructured idea
catalogue; **this** file is the working map from those ideas onto the shipped
`family × phase × gate` system. All event content lives under `assets/events/`,
**one file per family** (`assets/events/<family>.json`, e.g. `comedy.json`),
merged into one registry at load with a duplicate-id guard; add a new event to
its family's file (add a new family = a new file + one line in `EVENT_FILES` in
`src/data.rs`). Only the phase-pool table (mechanics) lives in code.*

## Principle: generate scenarios from **families × complications × outcomes**, not one-offs

An event = a **family** + optional **complication/twist** + **outcome branches**.
A handful of authored families cover a centuries-long voyage without visible
repetition — critical now that a single mission spans 300–600 years and dozens of
decisions. `EventCategory` (`ImmediateCrisis` / `GenerationalChallenge` /
`MissionMilestone` / `LegacyMoment`) stays the **scoring/weighting axis**; the
`family` tag is the **content-organisation + gating axis**.

## The ten families (canonical strings)

| Family | Maps to category (guidance) | Buffering subsystem (W5) | Notes |
| --- | --- | --- | --- |
| `exploration_first_contact` | LegacyMoment / MissionMilestone | — | Mostly Travel; big branching decisions |
| `diplomacy` | GenerationalChallenge | `security` | Travel & on-station; influence/piracy ties |
| `engineering` | ImmediateCrisis | `engineering_bay` | Any phase; core ship faults |
| `biology_medical` | ImmediateCrisis | `medical_bay` | The medical-bay upgrade loop |
| `science_anomaly` | LegacyMoment / MissionMilestone | — | Travel; risk/reward; can shift voyage length |
| `survival` | ImmediateCrisis | `agriculture` | Pressure-tests provisioning (food/fuel) |
| `mystery` | MissionMilestone | — | Ghost signals, derelicts, artifacts; salvage hooks |
| `comedy` | LegacyMoment / MissionMilestone | — | Tension-breakers, low stakes ("Lobites") |
| `ethics` | GenerationalChallenge | — | The "soul" — moral dilemmas |
| `legacy_drift` | GenerationalChallenge | `education_culture` | **Headline family** — only makes sense at century scale |

## The Long-Term Expedition family (`legacy_drift`) is the signature of this redesign

The century-scale beats a 300-year no-cryo voyage unlocks and short missions cannot,
**gated on year / generation / voyage-drift** so they read as *consequences* of the
long voyage rather than random rolls:

- `home_silence` (`min_year: 100`) — the home civilization stops answering; the
  mission stops being an errand for someone else.
- `the_faith` (`min_generation: 5`, `min_cultural_drift: 0.5`) — the mission becomes
  a religion; the charter is read as scripture.
- `the_schism_deepens` (`min_cultural_drift: 0.6`) — the ship splits into two peoples;
  the "let them part" branch carries `faction_loss: departed`.
- `cultural_schism` (W7) — the earlier, ungated schism; also `faction_loss: departed`.
- `returning_signal` (`min_year: 100`) — a changed voice from a changed home.

## Phase-aware weighting — the campaign skeleton (W6)

At LAUNCH, `event_resolver::skeleton::generate_beats` lays out one major beat per full
20 years of mission duration (skipping the first 5 years), each placed randomly within
its own 20-year window and drawn from the phase-appropriate pool for the month it
falls in. Same seed ⇒ same schedule. The monthly loop fires each beat when its month
arrives; a beat **replaces** that month's reactive/filler roll, and falls through to a
normal roll if its family is over-gated.

Phase pools (mechanics — the code table in `skeleton.rs`; the families are content):

- **Travel:** `exploration_first_contact`, `science_anomaly`, `diplomacy`, `mystery`, `engineering`
- **Operation:** `survival`, `diplomacy`, `engineering`, `mystery`
- **Return:** `legacy_drift`, `ethics`, `mystery`
- **Any phase (always added):** `biology_medical`, `comedy`

## Force-return & faction-loss beats

- Catastrophic `force_return` (Operation-gated): `crop_blight`; plus W2's `reactor_scram`.
- Fortunate `force_return` (Travel-gated windfall): `the_lodestar`; plus W2's `resupply_cache`.
- `faction_loss: settled`: `garden_world` (the canonical garden-world stop), `berth_lottery` (W7).
- `faction_loss: departed`: `cultural_schism` (W7), `the_schism_deepens`.
- **House rule:** `force_return` / `faction_loss` are never the first outcome (index 0), so the
  autoplay's dumb first-choice policy keeps the mission on-course; they are the deliberate,
  dramatic branch.

## Per-family inventory (live)

`total: 72 templates` — every family at or above the 6-template floor.

| Family | Templates | Gated / phased beats |
| --- | --- | --- |
| `exploration_first_contact` | 7 | the_lodestar (Travel) |
| `diplomacy` | 6 | — |
| `engineering` | 9 | — |
| `biology_medical` | 6 | — |
| `science_anomaly` | 6 | singularity_whisper (drift 0.3) |
| `survival` | 6 | garden_world, fuel_skim, crop_blight (phased) |
| `mystery` | 6 | — |
| `comedy` | 6 | — |
| `ethics` | 6 | — |
| `legacy_drift` | 14 | returning_signal, last_photograph, the_faith, home_silence, the_schism_deepens |

## Placeholders / follow-ups

- Flavor placeholders renamed to in-universe names as authored (Tribbles → **Lobites**,
  `lobites`). No Tribbles flavor remains.
- Families are at the structural floor (≥6); deepening any family toward the 10-template cap,
  and adding more Operation-phase objective-specific beats (mining hazards, colony setup),
  is future content work, not structure.
- An onboard-AI keeper-of-memory `legacy_drift` arc is sketched in `event_notes.md` and not yet
  authored — a natural next beat.
