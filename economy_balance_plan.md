# Stellar Legacy — Economy Balancing Plan

*Companion to `economy_report.md`. Goal: make the charter fee the dominant credit line (T1), pace the full best-kit at 5–6 successful missions (T2), keep the founding stake tight (T3), and leave the mineral/energy/food/market loops untouched (T4). All changes are data-only (`assets/*.json`) plus invariant tests — no new Rust constants.*

---

## The shape of the fix

Two moves, in tension by design:

1. **Raise charter credit fees ~3–6×** so a mission out-earns the passive drip and dwarfs its own provisioning bill.
2. **Raise catalog prices ~2.5×** so the richer fees still take 5–6 successful missions to buy everything.

Passive production (60 cr/yr) is deliberately **not** cut in the first pass — it's the safety net that keeps a failed mission from bankrupting a dynasty, and cutting it would also starve the treasury-voice bands. It becomes a phase-3 lever only if playtests show fees still failing T1.

### The budget check (why these multipliers)

Per successful mid-tier (~400 yr, renown-100) mission after repricing:

| | Credits |
|---|---|
| Charter fee (new) | ~+22,000 |
| Passive drip (~×1.2 crew) | ~+29,000 |
| Milestones (new) | ~+2,000 |
| Provisioning + refit | ~−7,000 |
| Crew/faction upkeep decisions | ~−3,000 |
| **Net per successful mission** | **~+43,000 — fee ≈ 50–60% of gross income, ≥3× the voyage bill ✓** |

Full kit at ×2.5 ≈ **195,000 cr** ÷ ~43,000 ≈ **4.5–5.5 missions**, landing right on the renown ladder's own 4+ mission gate to the top charters. A scraped-through mission (40% objective) now forfeits ~13,000 cr, not ~3,600 — failure finally has a bill. ✓

---

## Phase 1 — Reprice charter fees (`assets/contracts.json`)

Formula: `credits ≈ duration_years × tier_rate`, where the rate climbs with the renown gate — prestige charters pay better *per year*, not just bigger:

| Renown gate | Rate (cr/voyage-yr) |
|---|---|
| 0–40 | 40–45 |
| 100 | 55 |
| 200–250 | 65–70 |
| 400 | 90 |

Concrete new fees (rounded to feel authored, not generated):

| Charter | Years | Old | **New** |
|---|---|---|---|
| Tarssen Relief | 300 | 4,000 | **12,000** |
| Hollow Fleet | 310 | 3,800 | **12,500** |
| Coronal Tap | 320 | 5,500 | **13,000** |
| Deep Vein Survey | 340 | 6,000 | **13,500** |
| Ark Run (renown 40) | 320 | 7,200 | **14,500** |
| Veiled Expanse | 360 | 5,000 | **20,000** |
| Hard Contract *(pays a hard-job premium)* | 360 | 9,500 | **25,000** |
| Warden Patrol | 380 | 6,000 | **21,000** |
| Sanctuary Run | 400 | 5,200 | **22,000** |
| Karst Works | 400 | 9,000 | **22,000** |
| Seedfall | 420 | 7,000 | **23,000** |
| Seedbearers' Writ | 420 | 7,200 | **23,000** |
| Twin Survey | 440 | 9,500 | **24,000** |
| Long Tow | 450 | 8,200 | **25,000** |
| Deep Camp | 480 | 9,000 | **26,500** |
| Sunset Relief (200) | 400 | 7,200 | **26,000** |
| Sunward Dive (250) | 350 | 8,600 | **24,500** |
| Starfall Beacon (250) | 400 | 5,200 | **28,000** |
| Founding Colony (250) | 450 | 8,000 | **31,500** |
| Hearthfall Accord (250) | 460 | 8,500 | **32,000** |
| Far Crossing (400) | 480 | 9,000 | **43,000** |
| Long Dark (400) | 600 | 9,000 | **54,000** |

Also in this phase:
- **Milestone credit rewards ×5** (400→2,000, 500→2,500, 600→3,000, 800→4,000, the Sanctuary/Fleet 450–500s likewise). Influence/minerals/energy milestone rewards stay as-is.
- Non-credit fee components (influence, minerals, energy, food) stay as-is — they are tuned against their own loops.
- Existing mechanisms (proration, `reward_reputation_scale`, abandonment penalties) are untouched; they now multiply numbers worth multiplying.

## Phase 2 — Reprice the catalog (`assets/ship_components.json`, `assets/subsystems.json`)

**×2.5 on every credit price. Mineral prices unchanged** (mineral production is ~35/yr; scaling its sink would starve the loop — T4).

Components:

| Item | Old cr | **New cr** |
|---|---|---|
| Light Corvette | 500 | 1,250 |
| Armored Prow | 1,500 | 3,750 |
| Habitat Ring | 2,000 | 5,000 |
| Generation Ark | 2,500 | 6,250 |
| Commission premium (`config.commission.premium_credits`) | 3,000 | 7,500 |
| Ion Drive | 800 | 2,000 |
| Solar Sail | 1,200 | 3,000 |
| Fusion Torch | 1,800 | 4,500 |
| Ramscoop Array | 3,000 | 7,500 |
| Warp Coil | 4,000 | 10,000 |
| Point Defense | 400 | 1,000 |
| Flak Screen | 500 | 1,250 |
| Pulse Cannon | 600 | 1,500 |
| Mass Driver | 1,400 | 3,500 |
| Spinal Railgun | 2,200 | 5,500 |

Subsystem ladders (per tier, ×2.5): tier 1 → 3,500–4,500 · tier 2 → 7,000–9,000 · tier 3 → 14,000–18,000. Full six ladders ≈ **166,000 cr**. Full kit ≈ **195,000 cr** ≈ 5–5.5 missions. Mission-reward relics (Derelict Titan, Voidfold Lattice, Singularity Lance, Nanolathe Forge, Voidsealed Biosphere) stay priceless — they are the reason to *fly*, not to *save*.

Early-game check (T3): stake 10,000 − one tier-1 upgrade (~4,000) − first-voyage parts/fuel (~1,500) leaves ~4,500 buffer. Tight but never bricked: the renown-0 affordability test below enforces it forever.

## Phase 3 — Sharpen the sinks (config, `assets/data/game_config.json`)

- `repair.full_credits_cost` 1,500 → **4,000**: a battered return should cost a visible slice of the fee.
- Leave `fuel_cost_credits_per_point` (30) and `part_cost_credits` (12): they price survival, not progression, and their current weight is right against the *new* fees.
- Leave `crew.recruit_cost_credits` (800), `train_cost_credits` (400/600), faction `recruit_group_cost_credits` (2,500): cheap people-spend is fine once gear is the expensive thing.
- **Held lever:** `base_production.credits` 60 → 45 *only if* playtests show fees still under 50% of gross income. Cutting it also shifts the treasury-voice flush/bare bands — retune those together if pulled.
- `distress_credit_floor` (2,000) and heritage tier grants (500–6,000) keep their absolute values reviewed against the new price level — heritage head starts may deserve ×2 in a follow-up so "Remembered" still feels remembered.

## Phase 4 — Invariant tests (so this never regresses silently)

Add to the existing data-validation tests in `src/data.rs` (house rule: fail fast, data-driven):

1. **Fee-per-year band:** every charter's `reward.credits / target_duration_years` ∈ [35, 100], and non-decreasing in `min_renown` tier.
2. **Fee beats the bill:** every charter's credit fee ≥ 3 × its estimated voyage bill (`duration × parts_upkeep × part_cost + one full tank + full refit`).
3. **Kit pacing:** sum of all purchasable catalog credit prices (components + commission premium + all subsystem tiers) ∈ [4.5, 6.5] × the mean charter fee — the "multiple missions for the best ship" contract, in a test.
4. **Founding affordability (extend existing):** the renown-0 charter check in `data.rs` also asserts stake ≥ cheapest tier-1 upgrade + its provisioning estimate.

Existing simulation tests that hardcode reward/cost magnitudes (`tick/tests.rs`, `crew.rs`, `ship.rs` tests) must be audited and updated in the same commit as each phase.

## Phase 5 — Verify in the loop

- `cargo test` + `cargo clippy` per house CI.
- Run the autoplay harness (`src/simulation/autoplay.rs`) across 3–4 back-to-back charters; record treasury at each docking. Expected curve: ~10k → ~50k → ~95k → ~140k with full kit landing mid-campaign 5–6. If autoplay banks materially faster, the leak is passive/market income → pull the Phase-3 held lever.
- One manual playthrough of a renown-0 charter to feel the early game: the first upgrade should be a *choice*, not a formality.

## Sequencing

Each phase is one focused, separately committed change (fees → catalog → sinks → tests ride along with each). Phases 1 and 2 must ship in adjacent commits — fees without prices makes the game trivially rich; prices without fees makes it a grind. Phase 3's held lever waits for Phase 5 evidence.
