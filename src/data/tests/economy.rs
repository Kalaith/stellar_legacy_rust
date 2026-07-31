//! Provisioning, trade and the shape of a solvent campaign.

use super::*;

#[test]
fn a_new_ship_sails_provisioned_for_a_starter_charter() {
    // A new player should be able to fly a renown-0 charter without
    // shopping first: the founding stores cover the shortest one whole.
    let data = GameData::load().unwrap();
    let config = &data.config;
    let starter_years = data
        .contracts
        .iter()
        .filter(|(_, c)| c.min_renown == 0)
        .map(|(_, c)| c.target_duration_years)
        .min()
        .expect("at least one renown-0 charter");
    let food_need = (config.starting_population as f32
        * config.food_per_person_per_year
        * starter_years as f32)
        .ceil() as i64;
    assert!(
        config.starting_resources.food >= food_need,
        "founding food {} must cover a {starter_years}-yr starter charter ({food_need})",
        config.starting_resources.food
    );
    assert!(
        config.starting_spare_parts >= config.parts_upkeep_per_year * starter_years as i64,
        "founding parts {} must cover {starter_years} years of upkeep",
        config.starting_spare_parts
    );

    // Economy rebalance (phase 3, target T3): after
    // the phase-2 price hike the founding stake must still put a first
    // tier-1 upgrade within reach of a new captain — the early game keeps
    // its tension, but the first improvement is a choice on turn one, not a
    // wall to save toward. The stake covers the cheapest tier-1 subsystem
    // plus whatever launching the shortest starter charter costs in credits
    // (parts beyond the founding stock; the tank starts full).
    let cheapest_tier1 = data
        .subsystems
        .iter()
        .filter_map(|(_, s)| s.tiers.first())
        .map(|t| t.cost.credits)
        .min()
        .expect("subsystems have at least one purchasable tier");
    let parts_shortfall =
        (config.parts_upkeep_per_year * starter_years as i64 - config.starting_spare_parts).max(0);
    let launch_bill = parts_shortfall * config.provisioning.part_cost_credits;
    assert!(
        config.starting_resources.credits >= cheapest_tier1 + launch_bill,
        "founding stake {} must cover the cheapest tier-1 upgrade ({cheapest_tier1}) plus \
         the shortest starter charter's launch bill ({launch_bill})",
        config.starting_resources.credits
    );
}

#[test]
fn a_charter_fee_is_worth_the_voyage() {
    // Economy rebalance (phase 1): the fee is the
    // story. Every charter's credit fee sits in an authored band per
    // voyage-year — above the passive drip's shadow, below a blank check —
    // and the ladder climbs with the renown gate: founding writs pay modestly,
    // the storied ones pay like the legends they are.
    let data = GameData::load().unwrap();
    for (id, c) in data.contracts.iter() {
        let per_year = c.reward.credits as f32 / c.target_duration_years as f32;
        assert!(
            (35.0..=100.0).contains(&per_year),
            "charter '{id}' pays {per_year:.1} cr/voyage-year; the authored band is 35-100"
        );
        if c.min_renown == 0 {
            assert!(
                per_year <= 50.0,
                "founding charter '{id}' pays {per_year:.1} cr/yr; renown-0 writs stay at or under 50"
            );
        }
        if c.min_renown >= 400 {
            assert!(
                per_year >= 80.0,
                "storied charter '{id}' pays {per_year:.1} cr/yr; renown-400 writs pay 80+"
            );
        }
    }
}

#[test]
fn a_charter_fee_clears_its_provisioning_bill() {
    // Economy rebalance (phase 1): a writ must pay
    // for the sailing several times over. The bill estimated here is what
    // the voyage itself costs the treasury — the spare parts consumed beyond
    // the founding stock, and a full tank — so a mission is never a wash.
    let data = GameData::load().unwrap();
    let config = &data.config;
    for (id, c) in data.contracts.iter() {
        let parts_needed = config.parts_upkeep_per_year * c.target_duration_years as i64;
        let parts_shortfall = (parts_needed - config.starting_spare_parts).max(0);
        let bill = parts_shortfall * config.provisioning.part_cost_credits
            + 100 * config.provisioning.fuel_cost_credits_per_point;
        assert!(
            c.reward.credits >= 3 * bill,
            "charter '{id}' fee {} must be at least 3x its provisioning bill {bill}",
            c.reward.credits
        );
    }
}

#[test]
fn the_best_ship_is_earned_across_many_voyages() {
    // Economy rebalance (phase 2): the best ship and
    // its full kit should cost several successful missions, not one lucky
    // payday. This pins the whole-catalog credit cost against what a voyage
    // actually banks — fee, milestones, and the passive drip the crossing
    // mints, less what the sailing costs — so fees (phase 1) and prices
    // (phase 2) can never drift apart into trivial wealth or endless grind.
    let data = GameData::load().unwrap();
    let config = &data.config;
    use ship_components::ComponentKind;

    // The full best-buyable kit: the dearest hull (plus the commission
    // premium a new hull costs), the dearest engine, the dearest weapon,
    // and every subsystem tier bought up the ladder. Mission-reward relics
    // carry no price and never enter the reckoning.
    let dearest = |kind: ComponentKind| {
        data.ship_components
            .list(kind)
            .iter()
            .map(|c| c.cost.credits)
            .max()
            .unwrap_or(0)
    };
    let subsystem_ladders: i64 = data
        .subsystems
        .iter()
        .flat_map(|(_, s)| s.tiers.iter())
        .map(|t| t.cost.credits)
        .sum();
    let kit_cost = dearest(ComponentKind::Hull)
        + config.commission.premium_credits
        + dearest(ComponentKind::Engine)
        + dearest(ComponentKind::Weapon)
        + subsystem_ladders;

    // What a successful charter banks: its fee, its milestone credits, and
    // the base passive production over the whole crossing, less the voyage's
    // own provisioning bill (parts beyond the founding stock, plus a tank).
    let net_incomes: Vec<i64> = data
        .contracts
        .iter()
        .map(|(_, c)| {
            let milestones: i64 = c.milestones.iter().map(|m| m.reward.credits).sum();
            let drip = (config.base_production.credits * c.target_duration_years as f32) as i64;
            let parts_needed = config.parts_upkeep_per_year * c.target_duration_years as i64;
            let parts_shortfall = (parts_needed - config.starting_spare_parts).max(0);
            let bill = parts_shortfall * config.provisioning.part_cost_credits
                + 100 * config.provisioning.fuel_cost_credits_per_point;
            c.reward.credits + milestones + drip - bill
        })
        .collect();
    let mean_income = net_incomes.iter().sum::<i64>() / net_incomes.len() as i64;

    let missions = kit_cost as f32 / mean_income as f32;
    assert!(
        (4.0..=7.0).contains(&missions),
        "the full kit costs {kit_cost} cr = {missions:.1} mean-mission incomes ({mean_income} each); \
         the authored pacing is 4-7 successful voyages"
    );
}

#[test]
fn a_full_refit_is_a_visible_slice_of_a_fee_but_never_a_wall() {
    // Economy rebalance (phase 3): a battered return
    // should cost real coin — a full refit is the sink that makes thrashing
    // the ship matter — but never so much that even the leanest fee cannot
    // cover the way home. Pinned as a band against the cheapest charter fee.
    let data = GameData::load().unwrap();
    let refit = data.config.repair.full_credits_cost;
    let cheapest_fee = data
        .contracts
        .iter()
        .map(|(_, c)| c.reward.credits)
        .min()
        .expect("at least one charter");
    let slice = refit as f32 / cheapest_fee as f32;
    assert!(
        (0.10..=0.50).contains(&slice),
        "a full refit ({refit} cr) is {:.0}% of the leanest fee ({cheapest_fee}); \
         the sink should be a felt 10-50%, visible but never a wall",
        slice * 100.0
    );
}

#[test]
fn a_heritage_head_start_is_a_boost_not_a_replacement() {
    // Economy rebalance (phase 3 heritage review): the
    // rebalance left the founding stake untouched, so a storied dynasty's
    // credit head start stays anchored to it — a real leg up (the top tier is
    // a large fraction of the stake) that never eclipses a fresh captain's own
    // footing. This holds the "boost, not replacement" line against a future
    // heritage bump or a stake cut, and is why the grants were kept as-is
    // rather than scaled with the catalog: they ride the (unchanged) stake,
    // not the (raised) prices.
    let data = GameData::load().unwrap();
    let stake = data.config.starting_resources.credits;
    let top_grant = data
        .config
        .heritage
        .iter()
        .map(|h| h.credits)
        .max()
        .expect("heritage tiers exist");
    assert!(
        top_grant > 0 && top_grant < stake,
        "the richest heritage grant ({top_grant} cr) must be a boost — nonzero, but under the \
         founding stake ({stake}) so meta-progression never dominates the founding position"
    );
}

/// Market bends, morale drains and production sheds are all fractions.
#[test]
fn the_provisioning_couplings_are_gentle_and_bounded() {
    let data = GameData::load().unwrap();
    let fl = &data.config.flavor;
    // Content-depth provisioning round 21: the fabrication narration carries its
    // {parts} slot, and if the mechanic is on its costs/yield are sane (a positive
    // yield, and a mineral gate so it never runs a poor ship's ore dry).
    assert!(
        fl.fabrication.is_empty() || fl.fabrication.iter().all(|s| s.contains("{parts}")),
        "every fabrication flavor line needs its {{parts}} slot"
    );
    if data.config.surplus_energy_threshold > 0 {
        assert!(
            data.config.fabrication_parts_yield > 0
                && data.config.fabrication_minerals_cost > 0
                && data.config.fabrication_energy_cost > 0,
            "the fabrication mechanic is on but its costs/yield are not all positive"
        );
    }
    // Content-depth provisioning round 22: the market impact is a gentle per-unit
    // nudge — a bulk trade moves a thin market, but a single unit barely stirs it,
    // and the clamp plus the yearly drift keep even a whale ship from breaking it.
    assert!(
        (0.0..=0.01).contains(&data.config.market_impact_per_unit),
        "market_impact_per_unit {} out of the gentle range [0, 0.01]",
        data.config.market_impact_per_unit
    );
    // Content-depth provisioning round 30: the reputation trade scale is a gentle bend on
    // prices — a strong name shades the terms a captain's way but never makes trade free or
    // ruinous (kept below 1 so even a spotless or infamous name only tilts, never inverts).
    assert!(
        (0.0..1.0).contains(&data.config.trade_reputation_scale),
        "trade_reputation_scale {} must be a gentle fraction in [0, 1)",
        data.config.trade_reputation_scale
    );
    // Content-depth provisioning round 32: the desperation premium is a gentle markup in
    // [0, 1) — a crisis-buyer pays more for the good it cannot do without, but a waystation
    // never doubles the price on need alone.
    assert!(
        (0.0..1.0).contains(&data.config.market_desperation_premium),
        "market_desperation_premium {} must be a gentle markup in [0, 1)",
        data.config.market_desperation_premium
    );
    // Content-depth provisioning round 33: the distress discount is a fraction in [0, 1) — a
    // fire sale pays less, but a broke ship's stores are never taken for nothing.
    assert!(
        (0.0..1.0).contains(&data.config.market_distress_discount),
        "market_distress_discount {} must be a fraction in [0, 1)",
        data.config.market_distress_discount
    );
    // Content-depth provisioning round 25: the becalmed morale drain is a gentle
    // yearly attrition, like the chronic-hunger one it mirrors — the slow despair of a
    // voyage that will not move, not a single hard blow.
    assert!(
        (0.0..=0.05).contains(&data.config.becalmed_morale_drain),
        "becalmed_morale_drain {} must be a gentle yearly attrition [0, 0.05]",
        data.config.becalmed_morale_drain
    );
    // Content-depth provisioning round 27: the disrepair morale drain is a gentle yearly
    // attrition too, the third of the sustained-privation costs — the slow demoralization
    // of a home coming apart, not a single hard blow.
    assert!(
        (0.0..=0.05).contains(&data.config.disrepair_morale_drain),
        "disrepair_morale_drain {} must be a gentle yearly attrition [0, 0.05]",
        data.config.disrepair_morale_drain
    );
    // Content-depth provisioning round 34: the chronic-low-energy morale drain is a gentle
    // yearly attrition too, the fourth of the sustained-privation costs — the slow wearing of a
    // crew living in the dark, not a single hard blow.
    assert!(
        (0.0..=0.05).contains(&data.config.chronic_low_energy_morale_drain),
        "chronic_low_energy_morale_drain {} must be a gentle yearly attrition [0, 0.05]",
        data.config.chronic_low_energy_morale_drain
    );
    // Content-depth provisioning round 28: the chronic-hunger faction penalty is a gentle
    // yearly souring — the slow political erosion of a people that keeps going hungry, not a
    // single rupture (the acute famine events carry the sharp breaks).
    assert!(
        (0.0..=0.05).contains(&data.config.chronic_hunger_faction_penalty),
        "chronic_hunger_faction_penalty {} must be a gentle yearly souring [0, 0.05]",
        data.config.chronic_hunger_faction_penalty
    );
    // Content-depth provisioning round 31: the sustained-plenty faction bonus is the positive
    // mirror of that souring — a gentle yearly warming as a well-fed people learns to trust its
    // council — so it lives in the same [0, 0.05] band, never a single rupture of goodwill.
    assert!(
        (0.0..=0.05).contains(&data.config.sustained_plenty_faction_bonus),
        "sustained_plenty_faction_bonus {} must be a gentle yearly warming [0, 0.05]",
        data.config.sustained_plenty_faction_bonus
    );
    // Content-depth provisioning round 29: the low-energy production shed is a fraction in
    // [0, 1) — a power crisis dents industrial output but, kept below 1, never wholly stops
    // the factories, so a starved reactor slows the ship's earnings without freezing them.
    assert!(
        (0.0..1.0).contains(&data.config.low_energy_production_shed),
        "low_energy_production_shed {} must be in [0, 1) so power scarcity never zeroes production",
        data.config.low_energy_production_shed
    );
    // Content-depth provisioning round 24: the food carrying capacity, if set, must
    // sit above the fat line (a prudent reserve should still read as plenty, not spoil
    // the ship out of its own abundance), its spoilage a gentle fraction, and its
    // narration carry the {spoiled} slot.
    if data.config.food_carrying_capacity > 0 {
        assert!(
            data.config.food_carrying_capacity > data.config.fat_food_threshold,
            "food_carrying_capacity {} must sit above the fat line {} so plenty still reads",
            data.config.food_carrying_capacity,
            data.config.fat_food_threshold
        );
        assert!(
            data.config.food_spoilage_fraction > 0.0 && data.config.food_spoilage_fraction <= 0.5,
            "food_spoilage_fraction {} must be a gentle positive fraction",
            data.config.food_spoilage_fraction
        );
        assert!(
            data.config.flavor.food_spoilage.is_empty()
                || data
                    .config
                    .flavor
                    .food_spoilage
                    .iter()
                    .all(|s| s.contains("{spoiled}")),
            "every food_spoilage line needs its {{spoiled}} slot"
        );
    }
    // Content-depth provisioning round 26: the influence→governance income coupling. The
    // line is a fraction in (0,1) when enabled, and the floor a fraction in [0,1) strictly
    // below it (even a collapsed government mints *some* influence, but a healthy one must
    // out-earn it) — so the factor is continuous and never inverts.
    if data.config.influence_governance_threshold > 0.0 {
        assert!(
            data.config.influence_governance_threshold < 1.0,
            "influence_governance_threshold {} must be a fraction inside (0, 1)",
            data.config.influence_governance_threshold
        );
        assert!(
            (0.0..1.0).contains(&data.config.influence_governance_floor)
                && data.config.influence_governance_floor
                    < data.config.influence_governance_threshold,
            "influence_governance_floor {} must be in [0, 1) and below the threshold {}",
            data.config.influence_governance_floor,
            data.config.influence_governance_threshold
        );
    }
    // Content-depth charters round 22: the crew-morale accrual swing is gentle —
    // a devoted crew works meaningfully but not miraculously faster, and even a
    // broken one is floored above a stall at runtime.
    assert!(
        (0.0..=1.0).contains(&data.config.ship.morale_objective_swing),
        "morale_objective_swing {} out of the gentle range [0, 1]",
        data.config.ship.morale_objective_swing
    );
    // Content-depth charters round 34: the crew-unity accrual swing, the same gentle shape as
    // the morale one — a cohesive crew works meaningfully but not miraculously faster, floored
    // above a stall at runtime.
    assert!(
        (0.0..=1.0).contains(&data.config.ship.unity_objective_swing),
        "unity_objective_swing {} out of the gentle range [0, 1]",
        data.config.ship.unity_objective_swing
    );
    // Content-depth charters round 27: each point of combat deters route hazard by a
    // gentle fraction — a moderately-armed ship should meaningfully quiet a lawless route,
    // not make a single gun cancel the worst hazard outright.
    assert!(
        (0.0..=0.2).contains(&data.config.ship.hazard_combat_mitigation),
        "hazard_combat_mitigation {} out of the gentle range [0, 0.2]",
        data.config.ship.hazard_combat_mitigation
    );
    // Content-depth charters round 28: each berth eases preserve attrition by a gentle
    // fraction — a roomy hull should meaningfully outperform a cramped one, but not make a
    // single point of crew_capacity nearly cancel the whole attrition (the in-code floor of
    // 0.2 also caps the total relief regardless).
    assert!(
        (0.0..=0.05).contains(&data.config.ship.preserve_berth_relief),
        "preserve_berth_relief {} out of the gentle range [0, 0.05]",
        data.config.ship.preserve_berth_relief
    );
    // Content-depth charters round 31: the mission-outcome morale scale is a gentle one-time
    // shift — a clean run lifts spirits and a botched one dents them, but a single mission's
    // outcome should not, by itself, swing the whole crew's morale.
    assert!(
        (0.0..=0.5).contains(&data.config.ship.mission_outcome_morale_scale),
        "mission_outcome_morale_scale {} out of the gentle range [0, 0.5]",
        data.config.ship.mission_outcome_morale_scale
    );
}
