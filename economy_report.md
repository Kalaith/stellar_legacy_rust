# Stellar Legacy — Game Economy Report

*Audit of the credit economy as of this commit. All numbers read from `assets/data/game_config.json`, `assets/contracts.json`, `assets/ship_components.json`, `assets/subsystems.json`, and the simulation code (`src/simulation/tick/economy.rs`, `src/simulation/ship.rs`, `src/simulation/market.rs`, `src/simulation/contract.rs`, `src/game/actions.rs`).*

---

## 1. The founding position

| Resource | Start |
|---|---|
| Credits | **10,000** |
| Energy | 5,000 |
| Minerals | 2,000 |
| Food | 12,000 |
| Influence | 100 |
| Spare parts | 300 |
| Fuel | full tank |
| Ship | Colony Barge (free) + Ion Drive, no weapon |
| Heritage head start | +500 … +6,000 cr for storied dynasties (renown 100–900) |

## 2. Credit sources

### 2.1 Passive production — the silent majority
`base_production.credits = 60/yr`, multiplied by crew skill (commander +0.004/skill, navigator +0.002/skill — a skill-60 commander and skill-50 navigator push it to ~×1.34) and the power factor. Time only advances while a charter runs, so passive income is *per voyage*:

| Voyage length | Passive credits (×1.0 … ×1.4 crew) |
|---|---|
| 300 yr | 18,000 … 25,200 |
| 400 yr | 24,000 … 33,600 |
| 600 yr | 36,000 … 50,400 |

Minerals (35/yr) and energy (140/yr) also accrue; energy self-throttles through the fabricators, and mineral surplus (~12,000 over 340 yr) is worth ~5 cr/unit on the market before price-impact clamps.

### 2.2 Charter fees — the headline that isn't
Completion pays `reward.credits × objective_fraction × reputation multiplier (0.5–1.3)`:

| Charter | Renown gate | Years | Credits | **Cr / voyage-year** |
|---|---|---|---|---|
| Hollow Fleet | 0 | 310 | 3,800 | 12.3 |
| Tarssen Relief | 0 | 300 | 4,000 | 13.3 |
| Veiled Expanse | 100 | 360 | 5,000 | 13.9 |
| Sanctuary Run | 100 | 400 | 5,200 | 13.0 |
| Starfall Beacon | 250 | 400 | 5,200 | 13.0 |
| Coronal Tap | 0 | 320 | 5,500 | 17.2 |
| Deep Vein Survey | 0 | 340 | 6,000 | 17.6 |
| Warden Patrol | 100 | 380 | 6,000 | 15.8 |
| Seedfall | 100 | 420 | 7,000 | 16.7 |
| Sunset Relief | 200 | 400 | 7,200 | 18.0 |
| Seedbearers' Writ | 100 | 420 | 7,200 | 17.1 |
| Ark Run | 40 | 320 | 7,200 | 22.5 |
| Founding Colony | 250 | 450 | 8,000 | 17.8 |
| Long Tow | 100 | 450 | 8,200 | 18.2 |
| Hearthfall Accord | 250 | 460 | 8,500 | 18.5 |
| Sunward Dive | 250 | 350 | 8,600 | 24.6 |
| Long Dark | 400 | 600 | 9,000 | 15.0 |
| Far Crossing | 400 | 480 | 9,000 | 18.75 |
| Deep Camp | 100 | 480 | 9,000 | 18.75 |
| Karst Works | 100 | 400 | 9,000 | 22.5 |
| Hard Contract | 100 | 360 | 9,500 | 26.4 |
| Twin Survey | 100 | 440 | 9,500 | 21.6 |

**Every charter pays 12–26 credits per voyage-year. Passive production pays 60–85.** The fee for six centuries in the Long Dark is a ~25% tip on money the ship would have minted anyway.

### 2.3 Milestones
80–800 cr per charter, i.e. noise against a 10,000 founding stake.

## 3. Credit sinks

### 3.1 The voyage itself (per mission)
| Item | Cost |
|---|---|
| Spare parts shortfall (1/yr upkeep, start 300) | 340 yr → ~480 cr · 600 yr → ~3,600 cr @ 12 cr/part |
| Fuel top-up (30 cr/point, 100 pts/tank) | up to 3,000 cr |
| Food | usually ~0 — farms (45/yr) ≈ upkeep (40/yr @ 1,000 pop) |
| Full refit on return | 1,500 cr + 500 minerals |
| **Typical voyage bill** | **~2,000–8,000 cr** |

**The provisioning bill for a charter is the same order as its fee.** Net mission-specific profit ≈ 0; the Long Dark nets about +900 cr on its own line items for 600 years of risk.

### 3.2 People
Recruit officer 800 · train officer 400 · subsystem training 600 · recruit a faction group 2,500.

### 3.3 The "best ship" bill
One slot each of hull / engine / weapon; subsystem tiers are bought sequentially (1→2→3; tier 4s are mission-reward relics with no price).

| Purchase | Credits | Minerals |
|---|---|---|
| Generation Ark (2,500) + commission premium (3,000) | 5,500 | 1,600 |
| Warp Coil | 4,000 | 600 (+800 energy) |
| Spinal Railgun | 2,200 | 500 |
| Agriculture ladder (1,400+2,800+5,600) | 9,800 | 2,100 |
| Education ladder (1,600+3,200+6,400) | 11,200 | 1,400 |
| Engineering ladder (1,800+3,600+7,200) | 12,600 | 2,800 |
| Life-support ladder (1,700+3,400+6,800) | 11,900 | 2,450 |
| Medical ladder (1,500+3,000+6,000) | 10,500 | 2,100 |
| Security ladder (1,500+3,000+6,000) | 10,500 | 2,100 |
| **Full kit** | **~78,200** | **~15,650** |

## 4. Diagnosis

1. **Missions don't pay.** A charter's credit fee (12–26 cr/yr) is a fraction of the passive drip (60–85 cr/yr) and roughly cancels against its own provisioning bill. The player's wealth curve is driven by *time under sail*, not by *what the ship accomplishes*. Scraping through at 40% objective costs almost nothing financially, because proration only touches the small line.
2. **The kit is cheap relative to the drip.** ~78k full kit ÷ ~26–35k credit income per mission cycle ≈ **2.5–3 missions to own everything**, success optional. The renown ladder (400 renown = 4 flawless missions for the Long Dark / Far Crossing) already implies a 4–6 mission arc; the credit economy finishes long before the prestige economy does.
3. **Price signals are inverted.** A full fuel tank (3,000) costs half a typical mission fee; a whole founding people (2,500) costs less than a mid farm tier (2,800); an officer's lifetime bounty (800) is a rounding error. Survival is priced like it matters — accomplishment isn't.
4. **What's healthy and should not move:** the mineral economy (35/yr production vs upgrade/repair/fabrication demand is tight), the energy fabricator throttle, market price-impact clamps (0.5×–3× band prevents surplus-dumping exploits), the desperation/distress/reputation trade factors, and proration/reputation multipliers on fees (good mechanisms currently multiplying a too-small number).

## 5. Targets for the rebalance

- **T1 — The fee is the story.** A charter's credit fee should be ≥ 60% of the voyage's total credit income and ≥ 3× its provisioning bill. Completing well vs scraping through should swing the treasury by more than the founding stake.
- **T2 — The best ship is earned across a dynasty's dynasty.** Full kit ≈ **5–6 successful missions**, aligned with the renown ladder's 4-mission gate to the top charters.
- **T3 — Early game keeps its tension.** The 10,000 founding stake should still cover first provisioning plus one or two tier-1 upgrades, no more.
- **T4 — Don't collapse the other loops.** Minerals, energy, food, influence, and the market stay as-is in the first pass; they are separately tuned and mostly sound.

*The concrete repricing and phases live in `economy_balance_plan.md`.*
