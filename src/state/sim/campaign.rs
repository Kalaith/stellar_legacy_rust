//! Founding a campaign: the one long constructor that turns a legacy, a
//! seed and a roster of peoples into a ship ready to launch.

use crate::data::GameData;
use macroquad_toolkit::rng::SeededRng;
use std::collections::HashMap;

use super::*;

impl SimState {
    /// Build a fresh campaign for the chosen legacy and founding factions.
    /// Deterministic for a given (data, legacy, seed, faction set) — all
    /// randomness flows through the stored seeded RNG (GDD §5.6). The caller
    /// guarantees `faction_ids` holds exactly `config.factions.starting_count`
    /// entries (the picker / `founding_faction_ids` enforce it).
    pub fn new_campaign(
        data: &GameData,
        legacy_id: &str,
        seed: u64,
        faction_ids: &[String],
    ) -> Self {
        let config = &data.config;
        let mut rng = SeededRng::new(seed);
        let dynasty = founding_dynasty(data, legacy_id, &mut rng);

        let market = MarketState {
            entries: TradeResource::ALL
                .iter()
                .map(|&resource| MarketEntry {
                    resource,
                    price: base_price(resource),
                    trend: 0.0,
                })
                .collect(),
            impact_per_unit: config.market_impact_per_unit,
            trade_reputation_scale: config.trade_reputation_scale,
            desperation_premium: config.market_desperation_premium,
            desperation_food_floor: config.low_food_threshold,
            desperation_energy_floor: config.low_energy_threshold,
            distress_discount: config.market_distress_discount,
            distress_credit_floor: config.distress_credit_floor,
        };

        let mut sim = Self {
            seed,
            rng,
            month_clock: 0,
            last_event_month_clock: 0,
            speed: GameSpeed::default(),
            resources: ResourcePool::from_delta(config.starting_resources),
            production: config.base_production,
            ship: ShipState {
                hull_integrity: 1.0,
                life_support: 1.0,
                fuel: 1.0,
                spare_parts: config.starting_spare_parts,
                hull: "colony_barge".to_owned(),
                engine: "ion_drive".to_owned(),
                weapon: None,
                salvage: Vec::new(),
                unlocked_fittings: Vec::new(),
            },
            population: PopulationState {
                count: config.starting_population,
                morale: 0.7,
                unity: 0.7,
                stability: 0.7,
                legacy_loyalty: 0.6,
                adaptation: 0.3,
                cultural_drift: 0.1,
            },
            dynasty,
            crew: Vec::new(),
            next_crew_id: 0,
            legacy: LegacyTrack {
                legacy_id: legacy_id.to_owned(),
                tradition_points: 50,
                body_horror_events: 0,
                existential_dread: 0.0,
                piracy_reputation: 0.0,
            },
            contract: None,
            selected_charter: None,
            stalled_months: 0,
            fuel_stalled_this_year: false,
            fuel_stall_years: 0,
            becalmed_beat_band: 0,
            adaptation_divergence_band: 0,
            cultural_divergence_band: 0,
            fuel_scooped_accum: 0.0,
            tutorial_dismissed: false,
            market,
            delegation: DelegationSettings::default(),
            pending_event: None,
            pending_dilemma: None,
            consequences: Vec::new(),
            reputation: HashMap::new(),
            scheduled_events: Vec::new(),
            event_fire_counts: HashMap::new(),
            last_dominant_faction: String::new(),
            morale_band: 0,
            polity_mood_band: 0,
            reputation_beat_band: 0,
            reputation_voice_band: 0,
            wonder_voice_band: 0,
            resolve_voice_band: 0,
            stability_voice_band: 0,
            loyalty_voice_band: 0,
            adaptation_voice_band: 0,
            drift_voice_band: 0,
            unity_voice_band: 0,
            hull_voice_band: 0,
            air_voice_band: 0,
            fuel_voice_band: 0,
            crew_size_voice_band: 0,
            treasury_voice_band: 0,
            power_voice_band: 0,
            ruling_people_voice: None,
            hull_beat_band: 0,
            air_beat_band: 0,
            depopulation_beats_fired: 0,
            subsystem_beats_fired: Vec::new(),
            founding_beat_fired: false,
            lean_food_years: 0,
            lean_parts_years: 0,
            lean_energy_years: 0,
            fat_food_years: 0,
            factions: factions::build_founding_factions(faction_ids, config.starting_population),
            subsystems: subsystems::build_founding_subsystems(data),
            log: Vec::new(),
        };
        // Record the launch morale's band so the ship's hopeful starting spirits
        // read as the baseline, not a "lift" the collective-mood voice announces
        // (content-depth voice round 11).
        sim.morale_band = factions::mood_band_for(sim.population.morale);
        // Likewise record the launch band of the ship's institutional order so a
        // founding ship's sound government reads as the baseline, not a "firming" the
        // governance voice announces (content-depth voice round 17).
        sim.stability_voice_band = factions::stability_voice_band_for(
            sim.population.stability,
            config.flavor.stability_voice_high,
            config.flavor.stability_voice_low,
        );
        // Likewise record the launch band of the crew's devotion to the founders'
        // mission, so a founding crew's high loyalty reads as the baseline, not a
        // "brightening" the loyalty voice announces (content-depth voice round 20).
        sim.loyalty_voice_band = factions::stability_voice_band_for(
            sim.population.legacy_loyalty,
            config.flavor.loyalty_voice_high,
            config.flavor.loyalty_voice_low,
        );
        // Likewise record the launch band of the crew's physiological identity, so a
        // founding crew's baseline-human bodies read as the baseline, not a "shipborn"
        // the adaptation voice announces (content-depth voice round 25).
        sim.adaptation_voice_band = factions::stability_voice_band_for(
            sim.population.adaptation,
            config.flavor.adaptation_voice_high,
            config.flavor.adaptation_voice_low,
        );
        // Likewise record the launch band of the crew's cultural identity, so a founding
        // crew's founders-kept ways read as the baseline, not a "new people" the drift voice
        // announces (content-depth voice round 26).
        sim.drift_voice_band = factions::stability_voice_band_for(
            sim.population.cultural_drift,
            config.flavor.drift_voice_high,
            config.flavor.drift_voice_low,
        );
        // Likewise record the launch band of the crew's cohesion, so a founding crew's
        // one-people unity reads as the baseline, not a "cohering" the unity voice
        // announces (content-depth voice round 21).
        sim.unity_voice_band = factions::stability_voice_band_for(
            sim.population.unity,
            config.flavor.unity_voice_high,
            config.flavor.unity_voice_low,
        );
        // Likewise record the launch band of the ship's hull, so a new-built vessel's
        // sound body reads as the baseline, not a "riding true" the hull voice announces
        // (content-depth voice round 22).
        sim.hull_voice_band = factions::stability_voice_band_for(
            sim.ship.hull_integrity,
            config.flavor.hull_voice_high,
            config.flavor.hull_voice_low,
        );
        // Likewise record the launch band of the ship's air, so a new ship's clean
        // atmosphere reads as the baseline, not a "breathing easy" the air voice
        // announces (content-depth voice round 23).
        sim.air_voice_band = factions::stability_voice_band_for(
            sim.ship.life_support,
            config.flavor.air_voice_high,
            config.flavor.air_voice_low,
        );
        // Likewise record the launch band of the ship's drive, so a new ship's full tanks read
        // as the baseline, not a "flying free" the drive voice announces (content-depth voice
        // round 27).
        sim.fuel_voice_band = factions::stability_voice_band_for(
            sim.ship.fuel,
            config.flavor.fuel_voice_high,
            config.flavor.fuel_voice_low,
        );
        // Likewise record the people who run the ship at launch, so the founding majority reads as
        // the baseline, not a "changing of the guard" the ruling-people voice announces — only a
        // *later* shift in who is dominant speaks (content-depth voice round 31).
        sim.ruling_people_voice = sim.dominant_faction_id().map(str::to_owned);
        // Founding senior staff fill the configured starting posts.
        for archetype_id in &config.crew.starting_posts {
            let age_span = config.crew.recruit_age_max - config.crew.recruit_age_min + 1;
            let age = config.crew.recruit_age_min + sim.rng.below(age_span as usize) as u32;
            if let Some(member) = generate_crew_member(
                data,
                legacy_id,
                archetype_id,
                age,
                &mut sim.rng,
                &mut sim.next_crew_id,
            ) {
                sim.crew.push(member);
            }
        }
        // Name the peoples who board together (W7).
        let names: Vec<String> = faction_ids
            .iter()
            .map(|id| factions::log_name(&data.factions, id))
            .collect();
        if !names.is_empty() {
            sim.push_log(format!(
                "{} board together for the voyage.",
                join_names(&names)
            ));
        }
        sim.push_log("The founding council convenes. The voyage begins with a choice of contract.");
        sim
    }
}

/// The default founding faction set (W7): the first `starting_count` faction
/// ids in sorted order. Used by the game's real entry point and by tests that
/// don't drive the picker. Reads only from data — no faction names in Rust.
pub fn founding_faction_ids(data: &GameData) -> Vec<String> {
    let mut ids = GameData::sorted_ids(&data.factions);
    ids.truncate(data.config.factions.starting_count as usize);
    ids
}
