//! The subsystem verbs, dispatched from `game/actions.rs`: repair, upgrade,
//! install a fitting, and train the crew that keeps the module running.

use crate::data::{GameData, ResourceDelta};
use crate::state::sim::SimState;

use super::effects::education_training_factor;

/// Repair a subsystem (W5), underway or in port. Requires living expertise —
/// knowledge >= the subsystem's threshold — then spends parts + minerals to
/// restore condition (field ceiling underway, whole in port).
pub fn repair_subsystem(sim: &mut SimState, data: &GameData, id: &str) -> Result<(), String> {
    let Some(def) = data.subsystems.get(id) else {
        return Err("Unknown subsystem.".to_owned());
    };
    let knowledge = sim.subsystems.get(id).map(|s| s.knowledge).unwrap_or(0.0);
    if knowledge < def.repair_knowledge_required {
        return Err(format!(
            "No one aboard remembers how to mend the {}.",
            def.name
        ));
    }
    let current = sim.subsystems.get(id).map(|s| s.condition).unwrap_or(0.0);
    let in_port = sim.contract.is_none();
    let ceiling = if in_port {
        1.0
    } else {
        data.config.repair.field_ceiling
    };
    if current >= ceiling {
        return Err("It is already as sound as this can make it here.".to_owned());
    }
    let minerals = ResourceDelta {
        minerals: -def.repair_minerals_cost,
        ..Default::default()
    };
    if sim.ship.spare_parts < def.repair_parts_cost || !sim.resources.can_afford(&minerals) {
        return Err("Not enough spare parts or minerals to mend it.".to_owned());
    }
    sim.resources.apply(&minerals);
    sim.ship.spare_parts -= def.repair_parts_cost;
    let restored = if in_port {
        1.0
    } else {
        // A field repair is only as good as the bay it is made with (content-depth subsystems round
        // 34): the engineering bay's condition scales the underway gain, so a failing bay can only
        // patch. Dock repairs (above) go whole regardless — full facilities in port.
        let eng = sim
            .subsystems
            .get("engineering_bay")
            .map_or(1.0, |s| s.condition);
        let effectiveness =
            (1.0 - data.config.subsystems.engineering_field_repair_penalty * (1.0 - eng)).max(0.0);
        (current + data.config.repair.field_gain * effectiveness).min(ceiling)
    };
    let name = def.name.clone();
    if let Some(state) = sim.subsystems.get_mut(id) {
        state.condition = restored;
    }
    // Data-driven so a voyage's many field repairs do not reprint one line
    // (content-depth voice round 9); indexed by the month clock, built-in fallback.
    let line = crate::data::FlavorConfig::line_with_name(
        &data.config.flavor.subsystem_repair,
        sim.month_clock as usize,
        &name,
    )
    .unwrap_or_else(|| format!("The {name} is patched back toward working order."));
    sim.push_log(line);
    Ok(())
}

/// Upgrade a subsystem one tier (W5), port only. Pays the next tier's cost.
/// Tiers cap at 3.
pub fn upgrade_subsystem(sim: &mut SimState, data: &GameData, id: &str) -> Result<(), String> {
    if sim.contract.is_some() {
        return Err("Subsystems are rebuilt in drydock, between missions.".to_owned());
    }
    let Some(def) = data.subsystems.get(id) else {
        return Err("Unknown subsystem.".to_owned());
    };
    let name = def.name.clone();
    let tier = sim.subsystems.get(id).map(|s| s.tier).unwrap_or(0);
    let Some(next) = def.tiers.get(tier as usize) else {
        return Err(format!("The {name} is already at its highest tier."));
    };
    // A mission-reward version is never for sale — it is fitted via `install_fitting`
    // once a voyage has unlocked it, not bought here.
    if next.acquisition.is_mission_only() {
        return Err(format!(
            "The {} can only be recovered from a mission, not bought.",
            next.name
        ));
    }
    let cost = ResourceDelta {
        credits: -next.cost.credits,
        energy: -next.cost.energy,
        minerals: -next.cost.minerals,
        food: -next.cost.food,
        influence: -next.cost.influence,
    };
    if !sim.resources.can_afford(&cost) {
        return Err("The treasury cannot cover that upgrade.".to_owned());
    }
    sim.resources.apply(&cost);
    if let Some(state) = sim.subsystems.get_mut(id) {
        state.tier += 1;
    }
    // Tier-specific flavor (content-depth round 5): each module's rebuild reads
    // in its own voice; an unauthored tier falls back to the generic line so the
    // log never blanks.
    let line = if next.flavor.is_empty() {
        format!("The {name} is rebuilt stronger.")
    } else {
        next.flavor.clone()
    };
    sim.push_log(line);
    Ok(())
}

/// Fit the next subsystem version when it is a mission reward the ship has already
/// unlocked (2c). Free (the mission was the price), drydock-only, and refuses
/// unless the next version is mission-reward and its id is in
/// `ship.unlocked_fittings`. Consumes the unlock so a granted version fits once.
pub fn install_fitting(sim: &mut SimState, data: &GameData, id: &str) -> Result<(), String> {
    if sim.contract.is_some() {
        return Err("Subsystems are rebuilt in drydock, between missions.".to_owned());
    }
    let Some(def) = data.subsystems.get(id) else {
        return Err("Unknown subsystem.".to_owned());
    };
    let name = def.name.clone();
    let tier = sim.subsystems.get(id).map(|s| s.tier).unwrap_or(0);
    let Some(next) = def.tiers.get(tier as usize) else {
        return Err(format!("The {name} is already at its highest version."));
    };
    if !next.acquisition.is_mission_only() {
        return Err(format!(
            "The {} is bought in the yard, not unlocked.",
            next.name
        ));
    }
    if !sim.ship.unlocked_fittings.iter().any(|f| f == &next.id) {
        return Err(format!("No mission has recovered the {} yet.", next.name));
    }
    if let Some(state) = sim.subsystems.get_mut(id) {
        state.tier += 1;
    }
    // A granted version fits once; spend the unlock so it isn't re-usable.
    sim.ship.unlocked_fittings.retain(|f| f != &next.id);
    let line = if next.flavor.is_empty() {
        format!("The {name} is rebuilt to a design no yard could sell.")
    } else {
        next.flavor.clone()
    };
    sim.push_log(line);
    Ok(())
}

/// Train institutional knowledge for a subsystem (W5), anytime — the mid-voyage
/// recovery path when the experts have died out.
pub fn train_subsystem_knowledge(
    sim: &mut SimState,
    data: &GameData,
    id: &str,
) -> Result<(), String> {
    let Some(def) = data.subsystems.get(id) else {
        return Err("Unknown subsystem.".to_owned());
    };
    let cfg = &data.config.subsystems;
    let cost = ResourceDelta {
        credits: -cfg.train_cost_credits,
        ..Default::default()
    };
    if !sim.resources.can_afford(&cost) {
        return Err(format!(
            "Training a new cohort needs {} credits.",
            cfg.train_cost_credits
        ));
    }
    sim.resources.apply(&cost);
    // A cohort learns only as well as the ship's academy can teach (content-depth subsystems
    // round 27): the education/culture module's condition scales the knowledge a training run
    // imparts, so a crew that lets its schools rot trains new hands to a fainter craft.
    let academy = education_training_factor(sim, data);
    let name = def.name.clone();
    if let Some(state) = sim.subsystems.get_mut(id) {
        state.knowledge = (state.knowledge + cfg.train_knowledge_gain * academy).min(1.0);
    }
    let line = crate::data::FlavorConfig::line_with_name(
        &data.config.flavor.subsystem_training,
        sim.month_clock as usize,
        &name,
    )
    .unwrap_or_else(|| format!("A new cohort trains up on the {name}."));
    sim.push_log(line);
    Ok(())
}
