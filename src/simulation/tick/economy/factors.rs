//! Small read-only factors and the drift the voyage works on its people.

use crate::data::{GameConfig, GameData, PopulationDelta};
use crate::state::sim::SimState;

/// The ambient "life aboard" pool a quiet year draws from. The ship's *condition*
/// takes precedence — a long hunger, then a hollowed-out crew, then a far-drifted
/// people, then a long-flush larder (grim notes loudest first) — and failing all
/// of them, the plain *ordinary* quiet is colored by *who runs the ship*: a clear
/// dominant people's own `ambient` lines (content-depth factions round 21), or the
/// generic ordinary pool when it has none. Returns an empty pool only if every
/// candidate is empty.
pub(crate) fn quiet_ambient_pool<'a>(sim: &SimState, data: &'a GameData) -> &'a Vec<String> {
    let fl = &data.config.flavor;
    if !fl.ambient_lean.is_empty()
        && fl.ambient_lean_years_threshold > 0
        && sim.lean_food_years >= fl.ambient_lean_years_threshold
    {
        return &fl.ambient_lean;
    }
    if !fl.ambient_hollow.is_empty() && sim.population.count <= fl.ambient_population_threshold {
        return &fl.ambient_hollow;
    }
    if !fl.ambient_drifted.is_empty() && sim.population.cultural_drift >= fl.ambient_drift_threshold
    {
        return &fl.ambient_drifted;
    }
    if !fl.ambient_fat.is_empty()
        && fl.ambient_fat_years_threshold > 0
        && sim.fat_food_years >= fl.ambient_fat_years_threshold
    {
        return &fl.ambient_fat;
    }
    // Ordinary quiet — colored by the largest aboard people if it has ambient lines.
    sim.dominant_faction_id()
        .and_then(|id| data.factions.get(id))
        .map(|f| &f.ambient)
        .filter(|a| !a.is_empty())
        .unwrap_or(&fl.ambient)
}

/// Apply one year of voyage drift to the population (PLAN M4.1). Identity terms
/// scale by the legacy's multiplier (Adaptors fastest, Preservers slowest); the
/// morale/unity strain is universal. Clamped to 0-1 by `PopulationState::apply`.
/// The fraction of its influence income a ship actually mints given its governance
/// (content-depth provisioning round 26). Influence is political capital, and a council
/// that cannot reach quorum cannot issue the authority its officers spend: at or above the
/// governance line the ship earns full income (factor 1.0); below it, the factor falls
/// linearly from 1.0 at the line toward `influence_governance_floor` at zero stability, so
/// even an ungoverned ship mints a little raw standing but never zero. Inert (1.0) when the
/// threshold is 0. Reads `stability` only — deterministic, no RNG.
pub(crate) fn influence_governance_factor(sim: &SimState, config: &GameConfig) -> f32 {
    let threshold = config.influence_governance_threshold;
    if threshold <= 0.0 {
        return 1.0;
    }
    let stability = sim.population.stability;
    if stability >= threshold {
        return 1.0;
    }
    let floor = config.influence_governance_floor;
    floor + (1.0 - floor) * (stability / threshold)
}

/// The fraction of its *industrial* production a ship keeps given its power reserve (content-depth
/// provisioning round 29). Power runs the factories and refineries, not only the life-support and
/// the fabricators — so while the energy store sits below `low_energy_threshold` the ship's
/// credits-and-minerals output is shed, scaled `1 - shed·(1 - energy/threshold)`: full at the
/// line, `1 - shed` at empty tanks. At or above the line, full output. Inert (1.0) when the shed
/// is 0 or the threshold unset. Reads energy only — deterministic, no RNG.
pub(crate) fn energy_production_factor(sim: &SimState, config: &GameConfig) -> f32 {
    let shed = config.low_energy_production_shed;
    let threshold = config.low_energy_threshold;
    if shed <= 0.0 || threshold <= 0 {
        return 1.0;
    }
    let energy = sim.resources.energy.max(0);
    if energy >= threshold {
        return 1.0;
    }
    1.0 - shed * (1.0 - energy as f32 / threshold as f32)
}

pub(crate) fn apply_voyage_drift(sim: &mut SimState, data: &GameData) {
    let vd = &data.config.voyage_drift;
    let legacy_mult = vd
        .legacy_multipliers
        .get(&sim.legacy.legacy_id)
        .copied()
        .unwrap_or(1.0);
    // Who runs the ship bends how fast the people drift from the founders
    // (content-depth factions round 9): the dominant faction's ideology finally
    // does something — a tech-embracing majority leans into becoming someone new,
    // a tradition-bound one holds the founders' line. Read before the mutable
    // apply; gentle enough that identity still moves the same way whoever leads.
    let ideology = sim
        .dominant_faction_id()
        .and_then(|id| data.factions.get(id))
        .map_or(0.0, |f| f.ideology);
    let identity_mult = legacy_mult * (1.0 + vd.dominant_ideology_scale * ideology).max(0.0);
    // A well-kept culture archive resists the people forgetting the founders
    // (content-depth subsystems round 10): the education/culture module's
    // *knowledge* — how much of the founding is still remembered — slows the
    // cultural drift and the loyalty fade, but not the body's physiological
    // adaptation to the ship, which happens whether or not the archive holds.
    let archive_knowledge = sim
        .subsystems
        .get("education_culture")
        .map_or(0.0, |s| s.knowledge);
    let culture_mult =
        identity_mult * (1.0 - vd.archive_drift_resistance * archive_knowledge).max(0.0);
    // A well-kept infirmary keeps the crew *physically* baseline (content-depth
    // subsystems round 25): the bodily twin of the archive's cultural resistance. The
    // medical bay's living craft (its knowledge) slows the shipborn adaptation the way
    // the archive slows the cultural drift, so a ship bound for a world can hold its crew
    // fit to live on one. Reads knowledge, not condition — it is the *craft* of managing
    // the body, like the archive is the *memory* of the founders.
    let medical_knowledge = sim
        .subsystems
        .get("medical_bay")
        .map_or(0.0, |s| s.knowledge);
    // …and a *living* biosphere holds them planet-like too (content-depth subsystems round 29):
    // the environmental twin of the infirmary's craft. Where the medical bay slows the shipborn
    // drift by the *knowledge* of managing the body, a lush agriculture slows it by *condition* —
    // a crew that grows and eats living food and walks among green decks stays a little more the
    // kind of creature that could live on a world, while a crew fed vat-paste in sterile holds
    // goes shipborn faster. Reads condition (the biosphere's living state), not knowledge; stacks
    // multiplicatively with the medical resistance, so infirmary *and* farm both hold the line.
    let agriculture_condition = sim
        .subsystems
        .get("agriculture")
        .map_or(0.0, |s| s.condition);
    let adaptation_mult = identity_mult
        * (1.0 - vd.medical_adaptation_resistance * medical_knowledge).max(0.0)
        * (1.0 - vd.agriculture_adaptation_resistance * agriculture_condition).max(0.0);
    sim.population.apply(&PopulationDelta {
        adaptation: vd.adaptation_per_year * adaptation_mult,
        cultural_drift: vd.cultural_drift_per_year * culture_mult,
        legacy_loyalty: vd.legacy_loyalty_per_year * culture_mult,
        morale: vd.morale_strain_per_year,
        unity: vd.unity_strain_per_year,
        ..Default::default()
    });
}
