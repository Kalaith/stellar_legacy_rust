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

## Baseline (as of 2026-07-21)

| Axis | Current |
| --- | --- |
| Events | **72** total — legacy_drift 14, engineering 9, exploration_first_contact 7, survival/ethics/mystery/biology_medical/diplomacy/comedy/science_anomaly 6 each |
| Charters | **10** in `assets/contracts.json` |
| Factions | **6** authored (pick 3), structure-first v1 (no approval meters yet) |
| Subsystems | **6** (medical, life-support, agriculture, security, education, engineering) with decay + knowledge-gated repair |
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
