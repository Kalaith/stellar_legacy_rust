//! The economic year: what a ship produces and eats, and what a long lean
//! or a long plenty does to the crew's spirits and its politics.

use super::*;

#[test]
fn a_year_produces_resources_and_consumes_food() {
    // Events off so the year runs to its boundary without a decision stop.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        21,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let food_before = sim.resources.food;
    let credits_before = sim.resources.credits;

    let crew_mult = crate::simulation::crew::production_multipliers(&sim, &data);
    advance_year(&mut sim, &data);

    assert_eq!(sim.year(), 1);
    let upkeep = (sim.population.count as f32 * data.config.food_per_person_per_year).ceil() as i64;
    assert_eq!(
        sim.resources.food,
        food_before + (data.config.base_production.food * crew_mult.food).floor() as i64 - upkeep
    );
    assert!(crew_mult.food > 1.0, "founding agronomist boosts food");
    assert!(sim.resources.credits >= credits_before);
    assert!(sim.ship.hull_integrity < 1.0);
}

#[test]
fn over_deep_food_stores_spoil_toward_the_carrying_capacity() {
    // Content-depth provisioning round 24: food beyond what the ship can keep fresh
    // rots. A hoard above the carrying capacity loses a fraction of the excess each
    // year; a ship at sensible stores loses nothing.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let cap = data.config.food_carrying_capacity;
    assert!(
        cap > 0 && data.config.food_spoilage_fraction > 0.0,
        "this test needs the spoilage coupling enabled"
    );

    // A deep hoard: the excess above the cap erodes this year.
    let mut hoard = SimState::new_campaign(
        &data,
        "preservers",
        4,
        &crate::state::sim::founding_faction_ids(&data),
    );
    let start = cap + 40_000;
    hoard.resources.food = start;
    advance_year(&mut hoard, &data);
    assert!(
        hoard.resources.food < start,
        "an over-deep hoard loses stores to spoilage ({} -> {})",
        start,
        hoard.resources.food
    );
    assert!(
        hoard.resources.food > cap,
        "spoilage only erodes toward the cap, not below it in one year"
    );

    // A ship at sensible stores (below the cap): spoilage takes nothing (production and
    // upkeep move it a little, but no spoilage line fires and it is never clipped down).
    let mut modest = SimState::new_campaign(
        &data,
        "preservers",
        4,
        &crate::state::sim::founding_faction_ids(&data),
    );
    modest.resources.food = cap / 2;
    advance_year(&mut modest, &data);
    let spoil_lines = modest
        .log
        .iter()
        .filter(|l| data.config.flavor.food_spoilage.contains(&l.text))
        .count();
    assert_eq!(
        spoil_lines, 0,
        "a ship below its carrying capacity loses nothing to spoilage"
    );
}

#[test]
fn a_long_lean_wears_the_crews_spirits_down() {
    // Content-depth provisioning round 17: the axis's first *systemic* coupling. A
    // chronic hunger — years of a store below the lean line — drains morale each year
    // the lean holds, where a comfortably fed ship's spirits are untouched by the
    // larder. Isolated by matching two ships in all but their stores (production off,
    // a small crew so neither famines), so the only morale difference is the toll.
    let mut data = GameData::load().unwrap();
    assert!(
        data.config.chronic_hunger_morale_drain > 0.0 && data.config.chronic_hunger_years > 0,
        "this test needs the chronic-hunger coupling enabled"
    );
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.base_production.food = 0.0; // isolate the larder from fresh yield

    let make = |food: i64, lean_years: u32| -> SimState {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            17,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.count = 200; // a crew the stores easily feed (no famine)
        sim.resources.food = food;
        sim.lean_food_years = lean_years;
        sim
    };

    // A ship long lean (store below the lean line, years of it) vs one comfortably fed.
    let mut hungry = make(
        data.config.lean_food_threshold - 500,
        data.config.chronic_hunger_years,
    );
    let mut fed = make(data.config.fat_food_threshold + 5000, 0);
    assert_eq!(
        hungry.population.morale, fed.population.morale,
        "the two ships launch in the same spirits"
    );

    advance_year(&mut hungry, &data);
    advance_year(&mut fed, &data);

    assert!(
        hungry.population.morale < fed.population.morale,
        "a chronic hunger wears the crew's spirits down where a full larder does not \
         (hungry {} vs fed {})",
        hungry.population.morale,
        fed.population.morale
    );
    // All else matched, the gap is exactly the year's chronic-hunger toll.
    let gap = fed.population.morale - hungry.population.morale;
    assert!(
        (gap - data.config.chronic_hunger_morale_drain).abs() < 1e-4,
        "the morale gap is the chronic-hunger drain ({gap})"
    );
}

#[test]
fn a_long_plenty_lifts_the_crews_spirits() {
    // Content-depth provisioning round 20: a fat spell held past the sustained
    // threshold eases morale each year — the mirror of the chronic-hunger drain.
    let (data, base) = provisioned(5, 1.0);
    // Next month crosses a year boundary; everything else identical between the two.
    let setup = |food: i64, fat_years: u32| {
        let mut s = base.clone();
        s.month_clock = 11;
        s.resources.food = food;
        s.fat_food_years = fat_years;
        s.lean_food_years = 0;
        s.population.morale = 0.5;
        s.pending_event = None;
        s.pending_dilemma = None;
        s
    };
    let mut fat = setup(100_000, data.config.chronic_hunger_years.max(1));
    let mut plain = setup(8_000, 0);

    advance_months(&mut fat, &data, 1);
    advance_months(&mut plain, &data, 1);
    assert!(
        fat.population.morale > plain.population.morale,
        "a well-fed generation is a happier one (fat {} vs plain {})",
        fat.population.morale,
        plain.population.morale
    );
}

#[test]
fn a_long_hunger_turns_the_peoples_against_the_council() {
    // Content-depth provisioning round 28: the political toll of a chronic hunger. A ship long
    // lean sours its aboard peoples — a people that keeps going hungry stops trusting the council
    // that rations it — where a comfortably fed ship's factions are untouched by the larder.
    // Isolated like the morale test (production off, a small crew so neither famines, no events),
    // so the only approval difference is the year's hunger penalty on a single tracked faction.
    let mut data = GameData::load().unwrap();
    assert!(
        data.config.chronic_hunger_faction_penalty > 0.0 && data.config.chronic_hunger_years > 0,
        "this test needs the chronic-hunger faction coupling enabled"
    );
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.base_production.food = 0.0;

    let make = |food: i64, lean_years: u32| -> SimState {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            17,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.count = 200; // a crew the stores easily feed (no famine)
        sim.resources.food = food;
        sim.lean_food_years = lean_years;
        sim
    };
    // A tracked aboard faction's approval; demographic drift moves members, not approval, and at
    // neutral reputation/sound subsystems no other coupling touches it — so it isolates the toll.
    let approval = |sim: &SimState| {
        sim.factions
            .iter()
            .find(|f| f.is_aboard())
            .unwrap()
            .approval
    };

    let mut hungry = make(
        data.config.lean_food_threshold - 500,
        data.config.chronic_hunger_years,
    );
    let mut fed = make(data.config.fat_food_threshold + 5000, 0);
    assert_eq!(
        approval(&hungry),
        approval(&fed),
        "the two ships launch with the same faction goodwill"
    );

    advance_year(&mut hungry, &data);
    advance_year(&mut fed, &data);

    assert!(
        approval(&hungry) < approval(&fed),
        "a chronic hunger sours the peoples where a full larder does not (hungry {} vs fed {})",
        approval(&hungry),
        approval(&fed)
    );
    let gap = approval(&fed) - approval(&hungry);
    assert!(
        (gap - data.config.chronic_hunger_faction_penalty).abs() < 1e-4,
        "the approval gap is exactly the year's chronic-hunger faction penalty ({gap})"
    );
}

#[test]
fn a_long_plenty_warms_the_peoples_toward_the_council() {
    // Content-depth provisioning round 31: the political mirror of the it28 hunger souring — a ship
    // fed well and long warms its aboard peoples, who learn to trust the council that keeps their
    // holds full, where a larder only just gone fat (no standing plenty yet) leaves them untouched.
    // Both ships carry the same fat larder and small crew (no famine, no hunger penalty); the only
    // difference is the *streak* — one has held plenty past the sustained gate, the other has not —
    // so the sole approval gap is the year's plenty faction bonus on a single tracked faction.
    let mut data = GameData::load().unwrap();
    assert!(
        data.config.sustained_plenty_faction_bonus > 0.0 && data.config.chronic_hunger_years > 0,
        "this test needs the sustained-plenty faction coupling enabled"
    );
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.base_production.food = 0.0;

    let make = |fat_years: u32| -> SimState {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            17,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.population.count = 200; // a crew the stores easily feed (no famine)
        sim.resources.food = data.config.fat_food_threshold + 5000;
        sim.fat_food_years = fat_years;
        sim
    };
    let approval = |sim: &SimState| {
        sim.factions
            .iter()
            .find(|f| f.is_aboard())
            .unwrap()
            .approval
    };

    // The streak counter is incremented at the top of the year *before* the bonus gate, so a ship
    // launched at `chronic_hunger_years` clears the gate this year; one launched at 0 reaches only
    // 1 — a larder just gone fat, not a lifetime of plenty — and wins no goodwill yet.
    let mut plentiful = make(data.config.chronic_hunger_years);
    let mut new_plenty = make(0);
    assert_eq!(
        approval(&plentiful),
        approval(&new_plenty),
        "the two ships launch with the same faction goodwill"
    );

    advance_year(&mut plentiful, &data);
    advance_year(&mut new_plenty, &data);

    assert!(
        approval(&plentiful) > approval(&new_plenty),
        "a standing plenty warms the peoples where a larder just gone fat does not (long {} vs new {})",
        approval(&plentiful),
        approval(&new_plenty)
    );
    let gap = approval(&plentiful) - approval(&new_plenty);
    assert!(
        (gap - data.config.sustained_plenty_faction_bonus).abs() < 1e-4,
        "the approval gap is exactly the year's sustained-plenty faction bonus ({gap})"
    );
}

#[test]
fn a_chronic_becalming_wears_the_crews_spirits() {
    // Content-depth provisioning round 25: a ship stalled dry for years loses heart, the
    // fuel/mobility twin of the chronic-hunger morale drain. A ship that stays becalmed
    // this year ends it a shade grimmer than one that burns again, the gap the year's
    // becalmed drain exactly.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    let drain = data.config.becalmed_morale_drain;
    let years = data.config.chronic_hunger_years;
    assert!(
        drain > 0.0 && years > 0,
        "this test needs the becalming drain enabled"
    );

    // No contract, so no travel burn touches the stall flag — we set it directly.
    let run = |stalled_this_year: bool| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;
        sim.population.morale = 0.6;
        sim.fuel_stall_years = years; // already chronically becalmed at the year's start
        sim.fuel_stalled_this_year = stalled_this_year;
        advance_year(&mut sim, &data);
        sim.population.morale
    };
    let stays_becalmed = run(true); // still stalled → the drain bites
    let burns_again = run(false); // burns again → counter resets, no drain
    assert!(
        stays_becalmed < burns_again,
        "a ship still going nowhere loses heart where one that burns again does not"
    );
    assert!(
        (burns_again - stays_becalmed - drain).abs() < 1e-4,
        "the gap is exactly the year's becalmed morale drain ({} vs {burns_again})",
        stays_becalmed
    );
}

#[test]
fn a_ship_run_long_in_the_dark_loses_heart() {
    // Content-depth provisioning round 34: a ship whose grid runs dark for years loses heart, the
    // power twin of the chronic-hunger and becalming morale drains. A ship still below the low line
    // this year ends it a shade grimmer than one whose reactors recover, the gap the year's drain.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.base_production.energy = 0.0; // hold the energy store where we set it
    let drain = data.config.chronic_low_energy_morale_drain;
    let years = data.config.chronic_hunger_years;
    let low = data.config.low_energy_threshold;
    assert!(
        drain > 0.0 && years > 0 && low > 0,
        "this test needs the low-energy drain enabled"
    );

    let run = |energy: i64| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000;
        sim.population.morale = 0.6;
        sim.resources.energy = energy;
        sim.lean_energy_years = years; // already chronically dark at the year's start
        advance_year(&mut sim, &data);
        sim.population.morale
    };
    let stays_dark = run(low - 100); // still below the low line → the drain bites
    let reactors_recover = run(low + 5_000); // the grid recovers → the streak resets, no drain
    assert!(
        stays_dark < reactors_recover,
        "a ship still in the dark loses heart where one whose reactors recover does not"
    );
    assert!(
        (reactors_recover - stays_dark - drain).abs() < 1e-4,
        "the gap is exactly the year's low-energy morale drain ({stays_dark} vs {reactors_recover})"
    );
}

#[test]
fn a_chronic_disrepair_wears_the_crews_spirits() {
    // Content-depth provisioning round 27: a ship left unmended for years loses heart, the
    // toolroom twin of the chronic-hunger and becalming morale drains. A ship that stays short
    // of its maintenance stock this year ends it a shade grimmer than one that can cover it,
    // the gap the year's disrepair drain exactly.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    data.config.fabrication_parts_yield = 0; // no fabrication topping up the parts mid-year
    let drain = data.config.disrepair_morale_drain;
    let years = data.config.chronic_hunger_years;
    let upkeep = data.config.parts_upkeep_per_year;
    assert!(
        drain > 0.0 && years > 0,
        "this test needs the disrepair drain enabled"
    );

    let run = |unmended: bool| -> f32 {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            5,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000; // keep famine out of the way
        sim.population.morale = 0.6;
        sim.lean_parts_years = years; // already chronically unmended at the year's start
                                      // Short of upkeep → stays unmended; stocked → covers upkeep and the count resets.
        sim.ship.spare_parts = if unmended { 0 } else { upkeep + 100 };
        advance_year(&mut sim, &data);
        sim.population.morale
    };
    let stays_broken = run(true); // still unmended → the drain bites
    let mended = run(false); // stores cover upkeep → counter resets, no drain
    assert!(
        stays_broken < mended,
        "a ship still falling apart loses heart where one it can maintain does not"
    );
    assert!(
        (mended - stays_broken - drain).abs() < 1e-4,
        "the gap is exactly the year's disrepair morale drain ({stays_broken} vs {mended})"
    );
}

#[test]
fn a_power_rich_ship_fabricates_its_own_spare_parts() {
    // Content-depth provisioning round 21: idle reactor surplus — otherwise wasted —
    // is worked with raw ore into spare parts. A ship above the surplus line converts
    // each year (energy and minerals down, parts up); one below it does not.
    let mut data = GameData::load().unwrap();
    data.config.event_chance_base = 0.0;
    data.config.event_chance_cap = 0.0;
    data.config.dilemma_chance_per_generation = 0.0;
    // Isolate fabrication from the round-29 low-energy production shed: this test's poor run
    // (energy 0) would otherwise dent its own minerals production, confounding the comparison.
    data.config.low_energy_production_shed = 0.0;
    assert!(
        data.config.surplus_energy_threshold > 0 && data.config.fabrication_parts_yield > 0,
        "this test needs the fabrication mechanic enabled"
    );

    let run = |energy: i64| -> (i64, i64, i64) {
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            8,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.food = 1_000_000; // keep famine out of the way
        sim.resources.energy = energy;
        sim.resources.minerals = 5_000;
        let (parts0, min0) = (sim.ship.spare_parts, sim.resources.minerals);
        advance_year(&mut sim, &data);
        (
            sim.ship.spare_parts - parts0,
            min0 - sim.resources.minerals,
            sim.resources.energy,
        )
    };

    // Above the surplus line vs a power-starved ship (energy 0). Both runs share the
    // seed, so their yearly production is identical — the difference isolates the
    // fabrication: net minerals spent differs by exactly the ore feedstock, and the
    // surplus run banks parts the starved one never gets.
    let (parts_rich, min_spent_rich, _e) = run(data.config.surplus_energy_threshold + 4_000);
    let (parts_poor, min_spent_poor, _e) = run(0);
    assert_eq!(
        min_spent_rich - min_spent_poor,
        data.config.fabrication_minerals_cost,
        "the surplus run spends exactly its ore feedstock more than the starved run"
    );
    assert!(
        parts_rich > parts_poor,
        "the surplus buys parts the starved ship never gets ({parts_rich} vs {parts_poor})"
    );
}

#[test]
fn a_power_starved_ship_runs_its_industry_cold() {
    // Content-depth provisioning round 29: power runs the factories, so a ship below its
    // low-energy line sheds industrial output — full at the line, less as the tanks empty, but
    // never to zero (the shed is a fraction).
    let data = GameData::load().unwrap();
    let shed = data.config.low_energy_production_shed;
    let threshold = data.config.low_energy_threshold;
    assert!(
        shed > 0.0 && threshold > 0,
        "this test needs the low-energy production coupling enabled"
    );
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        4,
        &crate::state::sim::founding_faction_ids(&data),
    );

    // At and above the line: full industry.
    sim.resources.energy = threshold;
    assert_eq!(energy_production_factor(&sim, &data.config), 1.0);
    sim.resources.energy = threshold * 4;
    assert_eq!(energy_production_factor(&sim, &data.config), 1.0);

    // Empty tanks: shed to exactly (1 - shed), and never below.
    sim.resources.energy = 0;
    let empty = energy_production_factor(&sim, &data.config);
    assert!(
        (empty - (1.0 - shed)).abs() < 1e-6,
        "empty tanks shed exactly the configured fraction ({empty})"
    );
    assert!(
        empty > 0.0,
        "even a dead reactor keeps some industry running"
    );

    // Half-starved: between the empty floor and full.
    sim.resources.energy = threshold / 2;
    let half = energy_production_factor(&sim, &data.config);
    assert!(
        half > empty && half < 1.0,
        "a half-starved reactor sheds some but not all ({half})"
    );
}

#[test]
fn a_governed_ship_mints_full_influence_and_a_collapsing_one_earns_less() {
    // Content-depth provisioning round 26: influence is political capital, only as real as
    // the institutions that mint it. A ship at or above the governance line earns full
    // income (factor 1.0); below it the factor falls proportionally toward the floor at
    // zero stability — but never to zero, and never above 1.0.
    let data = GameData::load().unwrap();
    let threshold = data.config.influence_governance_threshold;
    let floor = data.config.influence_governance_floor;
    assert!(threshold > 0.0, "this test needs the coupling enabled");
    let mut sim = SimState::new_campaign(
        &data,
        "preservers",
        6,
        &crate::state::sim::founding_faction_ids(&data),
    );

    // At and above the line: full income.
    sim.population.stability = threshold;
    assert_eq!(influence_governance_factor(&sim, &data.config), 1.0);
    sim.population.stability = 1.0;
    assert_eq!(influence_governance_factor(&sim, &data.config), 1.0);

    // Below the line: less than full, and monotonically lower as governance slips.
    sim.population.stability = threshold * 0.5;
    let mid = influence_governance_factor(&sim, &data.config);
    assert!(
        mid < 1.0 && mid > floor,
        "a slipping government earns less ({mid})"
    );

    // Total collapse: the floor exactly, never zero.
    sim.population.stability = 0.0;
    let collapsed = influence_governance_factor(&sim, &data.config);
    assert!(
        (collapsed - floor).abs() < 1e-6,
        "an ungoverned ship mints only the floor ({collapsed} vs {floor})"
    );
    assert!(
        collapsed > 0.0,
        "even a collapsed government mints something"
    );
}
