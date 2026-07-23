//! Ship-loadout effects (GDD §5.1, PLAN item 3).
//!
//! The installed hull + engine + weapon each carry [`ComponentStats`]; this
//! module aggregates them and turns them into the yearly deltas the tick
//! applies. Deterministic: it only reads the loadout ids and sums catalog
//! stats — no RNG.

use crate::data::ship_components::{ComponentKind, ComponentStats};
use crate::data::{GameConfig, GameData, PopulationDelta, ResourceDelta};
use crate::state::sim::SimState;

/// Whether a salvaged part can be installed right now, and if not, why
/// (PLAN M4.4). At port anything installs; underway it's gated by the part,
/// the crew, and consumables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallEligibility {
    Ready,
    NeedsDrydock,
    NeedsEngineer,
    NeedsConsumables,
    NotSalvaged,
}

/// The post id whose holder can fit salvaged parts in the field.
const FIELD_ENGINEER_POST: &str = "engineer";

fn has_field_engineer(sim: &SimState, skill_required: u32) -> bool {
    sim.crew
        .iter()
        .any(|c| c.archetype_id == FIELD_ENGINEER_POST && c.skill >= skill_required)
}

/// Can this salvaged part be fitted right now (PLAN M4.4)? Single source of
/// truth shared by `install_salvage` and the Ship screen's install button.
pub fn install_eligibility(sim: &SimState, data: &GameData, id: &str) -> InstallEligibility {
    if !sim.ship.salvage.iter().any(|s| s == id) {
        return InstallEligibility::NotSalvaged;
    }
    let Some((_, component)) = data.ship_components.find_any(id) else {
        return InstallEligibility::NotSalvaged;
    };
    // In port, the drydock fits anything for free.
    if sim.contract.is_none() {
        return InstallEligibility::Ready;
    }
    let cfg = &data.config.field_install;
    if !component.field_installable {
        return InstallEligibility::NeedsDrydock;
    }
    if !has_field_engineer(sim, cfg.skill_required) {
        return InstallEligibility::NeedsEngineer;
    }
    let minerals = ResourceDelta {
        minerals: -cfg.minerals_cost,
        ..Default::default()
    };
    if sim.ship.spare_parts < cfg.parts_cost || !sim.resources.can_afford(&minerals) {
        return InstallEligibility::NeedsConsumables;
    }
    InstallEligibility::Ready
}

/// Install a salvaged part into its slot, dropping it from the hold (PLAN M4.4).
/// Underway this charges spare parts + minerals; in port it's free. Refuses with
/// a reason if the part isn't installable in the current situation.
pub fn install_salvage(sim: &mut SimState, data: &GameData, id: &str) -> Result<(), String> {
    match install_eligibility(sim, data, id) {
        InstallEligibility::Ready => {}
        InstallEligibility::NotSalvaged => {
            return Err("That part isn't in the salvage hold.".to_owned())
        }
        InstallEligibility::NeedsDrydock => {
            return Err("Too big to fit in the field — it needs a drydock.".to_owned())
        }
        InstallEligibility::NeedsEngineer => {
            return Err("No engineer skilled enough to fit it underway.".to_owned())
        }
        InstallEligibility::NeedsConsumables => {
            return Err("Not enough spare parts or minerals to fit it.".to_owned())
        }
    }
    let (kind, name) = {
        let (kind, component) = data
            .ship_components
            .find_any(id)
            .expect("eligibility checked");
        (kind, component.name.clone())
    };
    // Underway installs consume the field kit; a drydock install is free.
    if sim.contract.is_some() {
        let cfg = &data.config.field_install;
        sim.resources.apply(&ResourceDelta {
            minerals: -cfg.minerals_cost,
            ..Default::default()
        });
        sim.ship.spare_parts -= cfg.parts_cost;
    }
    match kind {
        ComponentKind::Hull => sim.ship.hull = id.to_owned(),
        ComponentKind::Engine => sim.ship.engine = id.to_owned(),
        ComponentKind::Weapon => sim.ship.weapon = Some(id.to_owned()),
    }
    if let Some(pos) = sim.ship.salvage.iter().position(|s| s == id) {
        sim.ship.salvage.remove(pos);
    }
    sim.push_log(format!("{name} fitted from the salvage hold."));
    Ok(())
}

/// Which ship subsystem a repair verb targets (PLAN M4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    Hull,
    LifeSupport,
}

impl RepairKind {
    fn label(self) -> &'static str {
        match self {
            RepairKind::Hull => "Hull",
            RepairKind::LifeSupport => "Life support",
        }
    }
}

/// Field repair (underway, PLAN M4.3): patch a subsystem from carried
/// consumables — spare parts + minerals — by `field_gain`, but only up to
/// `field_ceiling` (a ship can never be made pristine in the black; that is
/// what port is for). Returns an error message on refusal.
pub fn field_repair(
    sim: &mut SimState,
    config: &GameConfig,
    kind: RepairKind,
) -> Result<(), String> {
    let cfg = &config.repair;
    let current = match kind {
        RepairKind::Hull => sim.ship.hull_integrity,
        RepairKind::LifeSupport => sim.ship.life_support,
    };
    if current >= cfg.field_ceiling {
        return Err("Field repairs won't hold past a point — that needs a drydock.".to_owned());
    }
    if sim.ship.spare_parts < cfg.field_parts_cost {
        return Err("Not enough spare parts for a field repair.".to_owned());
    }
    let minerals = ResourceDelta {
        minerals: -cfg.field_minerals_cost,
        ..Default::default()
    };
    if !sim.resources.can_afford(&minerals) {
        return Err("Not enough minerals for a field repair.".to_owned());
    }
    sim.resources.apply(&minerals);
    sim.ship.spare_parts -= cfg.field_parts_cost;
    match kind {
        RepairKind::Hull => {
            sim.ship.hull_integrity =
                (sim.ship.hull_integrity + cfg.field_gain).min(cfg.field_ceiling)
        }
        RepairKind::LifeSupport => {
            sim.ship.life_support = (sim.ship.life_support + cfg.field_gain).min(cfg.field_ceiling)
        }
    }
    sim.push_log(format!(
        "{} patched in the field — it will hold, for now.",
        kind.label()
    ));
    Ok(())
}

/// Commission a new ship (port-only, PLAN M4.5): swap to a new hull, fully
/// refit the vessel, and lift the crew's morale — a fresh hull renews hope. The
/// people carry across unchanged (drift/adaptation are NOT reset). Costs the
/// hull's catalog price plus a commission premium.
pub fn commission_ship(sim: &mut SimState, data: &GameData, hull_id: &str) -> Result<(), String> {
    if sim.contract.is_some() {
        return Err("A new ship can only be commissioned in port.".to_owned());
    }
    let Some(hull) = data.ship_components.find(ComponentKind::Hull, hull_id) else {
        return Err("Unknown hull.".to_owned());
    };
    let cm = &data.config.commission;
    let cost = ResourceDelta {
        credits: -(hull.cost.credits + cm.premium_credits),
        minerals: -(hull.cost.minerals + cm.premium_minerals),
        energy: -hull.cost.energy,
        ..Default::default()
    };
    if !sim.resources.can_afford(&cost) {
        return Err("The treasury cannot cover a new ship.".to_owned());
    }
    let name = hull.name.clone();
    sim.resources.apply(&cost);
    sim.ship.hull = hull_id.to_owned();
    sim.ship.hull_integrity = 1.0;
    sim.ship.life_support = 1.0;
    sim.ship.fuel = 1.0;
    if sim.ship.spare_parts < data.config.repair.full_parts_restock {
        sim.ship.spare_parts = data.config.repair.full_parts_restock;
    }
    // A fresh hull renews hope — but the people are who they've become.
    sim.population.apply(&PopulationDelta {
        morale: cm.hope_morale,
        unity: cm.hope_unity,
        ..Default::default()
    });
    sim.push_log(format!(
        "A new ship christened: the {name}. Hope runs high again."
    ));
    Ok(())
}

/// Refuel to a full tank in drydock (port-only, W4). Costs
/// `fuel_cost_credits_per_point` credits per whole fuel point restored (the
/// missing fraction × 100 integer credits). Underway the only fuel is the slow
/// engine regen — a dry tank between systems is exactly the peril W4 adds.
pub fn refuel(sim: &mut SimState, config: &GameConfig) -> Result<(), String> {
    if sim.contract.is_some() {
        return Err("Refuelling is a drydock job, between missions.".to_owned());
    }
    let missing = 1.0 - sim.ship.fuel;
    if missing <= 0.0 {
        return Err("The tanks are already full.".to_owned());
    }
    let cost =
        (config.provisioning.fuel_cost_credits_per_point as f32 * missing * 100.0).ceil() as i64;
    if sim.resources.credits < cost {
        return Err(format!("Refuelling the tanks needs {cost} credits."));
    }
    sim.resources.credits -= cost;
    sim.ship.fuel = 1.0;
    sim.push_log("Tanks topped off in drydock — full and cold and ready.");
    Ok(())
}

/// Stock spare parts in drydock (port-only, W4 provisioning): buy `amount`
/// parts at the configured credit price. Underway the stores only drain — the
/// black sells nothing.
pub fn buy_parts(sim: &mut SimState, config: &GameConfig, amount: i64) -> Result<(), String> {
    if sim.contract.is_some() {
        return Err("Spare parts are stocked in port, between missions.".to_owned());
    }
    if amount <= 0 {
        return Err("The stores are already stocked.".to_owned());
    }
    let cost = amount * config.provisioning.part_cost_credits;
    if sim.resources.credits < cost {
        return Err(format!("Stocking {amount} parts needs {cost} credits."));
    }
    sim.resources.credits -= cost;
    sim.ship.spare_parts += amount;
    sim.push_log(format!(
        "{amount} spare parts craned aboard and racked for the voyage."
    ));
    Ok(())
}

/// Full refit (port-only, PLAN M4.3): restore hull, life support, and fuel to
/// whole and top the spare-parts stores back up, for credits + minerals. Only
/// available between missions (`contract == None`) — the drydock the field kit
/// can't stand in for.
pub fn full_repair(sim: &mut SimState, config: &GameConfig) -> Result<(), String> {
    if sim.contract.is_some() {
        return Err("A full refit can only be done in port, between missions.".to_owned());
    }
    let cfg = &config.repair;
    let cost = ResourceDelta {
        credits: -cfg.full_credits_cost,
        minerals: -cfg.full_minerals_cost,
        ..Default::default()
    };
    if !sim.resources.can_afford(&cost) {
        return Err("The treasury cannot cover a full refit.".to_owned());
    }
    sim.resources.apply(&cost);
    sim.ship.hull_integrity = 1.0;
    sim.ship.life_support = 1.0;
    sim.ship.fuel = 1.0;
    if sim.ship.spare_parts < cfg.full_parts_restock {
        sim.ship.spare_parts = cfg.full_parts_restock;
    }
    sim.push_log("Full refit complete in drydock — the ship is whole again.");
    Ok(())
}

/// Sum the stats of every currently installed component.
pub fn loadout_stats(sim: &SimState, data: &GameData) -> ComponentStats {
    let picks = [
        (ComponentKind::Hull, Some(sim.ship.hull.as_str())),
        (ComponentKind::Engine, Some(sim.ship.engine.as_str())),
        (ComponentKind::Weapon, sim.ship.weapon.as_deref()),
    ];
    let mut total = ComponentStats::default();
    for (kind, id) in picks {
        let Some(component) = id.and_then(|id| data.ship_components.find(kind, id)) else {
            continue;
        };
        let s = &component.stats;
        total.cargo += s.cargo;
        total.crew_capacity += s.crew_capacity;
        total.speed += s.speed;
        total.combat += s.combat;
        total.fuel_regen += s.fuel_regen;
    }
    total
}

/// Apply the loadout's yearly effects: a production bonus (speed → credits from
/// faster trade runs, cargo → minerals from bigger holds) and fuel regeneration
/// (a scooping engine). Combat is surfaced in the UI; its wanderer-dilemma odds
/// hook is a later item.
pub fn apply_loadout_effects(sim: &mut SimState, data: &GameData) {
    let stats = loadout_stats(sim, data);
    let cfg = &data.config.ship;

    let bonus = ResourceDelta {
        credits: stats.speed as i64 * cfg.credits_per_speed,
        minerals: (stats.cargo as f32 * cfg.minerals_per_cargo).floor() as i64,
        ..Default::default()
    };
    sim.resources.apply(&bonus);

    if stats.fuel_regen > 0 {
        // Accrue only the fuel the tank actually took (the part above the 1.0 cap
        // is not "scooped"), so the periodic provisioning line reports a real haul
        // and stays silent while the tank simply sits full (real-time loop
        // follow-up: legible stat changes).
        let before = sim.ship.fuel;
        sim.ship.fuel = (before + stats.fuel_regen as f32 * cfg.fuel_regen_per_point).min(1.0);
        sim.fuel_scooped_accum += sim.ship.fuel - before;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loadout_sums_installed_component_stats() {
        let data = GameData::load().unwrap();
        let sim = SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        // Founding loadout: colony_barge hull + ion_drive engine, no weapon.
        let stats = loadout_stats(&sim, &data);
        assert_eq!(stats.cargo, 200); // colony_barge
        assert_eq!(stats.speed, 2); // ion_drive
        assert_eq!(stats.combat, 0); // no weapon
    }

    #[test]
    fn field_repair_patches_but_never_reaches_pristine() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.ship.hull_integrity = 0.3;
        sim.ship.spare_parts = 100;
        sim.resources.minerals = 100_000;

        for _ in 0..20 {
            let _ = field_repair(&mut sim, &data.config, RepairKind::Hull);
        }
        let ceiling = data.config.repair.field_ceiling;
        assert!(
            (sim.ship.hull_integrity - ceiling).abs() < 1e-4,
            "field repair tops out at the ceiling ({ceiling}), got {}",
            sim.ship.hull_integrity
        );
        assert!(sim.ship.hull_integrity < 1.0, "never pristine in the black");
        assert!(sim.ship.spare_parts < 100, "field repair spends parts");
    }

    #[test]
    fn field_repair_refused_without_parts() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.ship.hull_integrity = 0.3;
        sim.ship.spare_parts = 0;
        sim.resources.minerals = 100_000;
        assert!(field_repair(&mut sim, &data.config, RepairKind::Hull).is_err());
    }

    #[test]
    fn full_repair_is_port_only_and_restores_everything() {
        use crate::simulation::contract::start_contract;
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.ship.hull_integrity = 0.3;
        sim.ship.life_support = 0.4;
        sim.ship.fuel = 0.2;
        sim.ship.spare_parts = 0;
        sim.resources.credits = 100_000;
        sim.resources.minerals = 100_000;

        // Underway: refused.
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        assert!(
            full_repair(&mut sim, &data.config).is_err(),
            "no full refit underway"
        );

        // In port: restores the ship to whole and tops parts back up.
        sim.contract = None;
        full_repair(&mut sim, &data.config).unwrap();
        assert_eq!(sim.ship.hull_integrity, 1.0);
        assert_eq!(sim.ship.life_support, 1.0);
        assert_eq!(sim.ship.fuel, 1.0);
        assert!(sim.ship.spare_parts >= data.config.repair.full_parts_restock);
    }

    #[test]
    fn salvage_field_install_is_gated_by_crew_and_part() {
        use crate::simulation::contract::start_contract;
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.ship.spare_parts = 100;
        sim.resources.minerals = 100_000;
        // Underway.
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));

        // A field-installable weapon in the hold, with a skilled engineer aboard
        // (the founding crew includes an engineer), installs underway.
        let eng = sim
            .crew
            .iter_mut()
            .find(|c| c.archetype_id == "engineer")
            .unwrap();
        eng.skill = data.config.field_install.skill_required + 5;
        sim.ship.salvage.push("mass_driver".to_owned());
        assert_eq!(
            install_eligibility(&sim, &data, "mass_driver"),
            InstallEligibility::Ready
        );
        install_salvage(&mut sim, &data, "mass_driver").unwrap();
        assert_eq!(sim.ship.weapon.as_deref(), Some("mass_driver"));
        assert!(!sim.ship.salvage.iter().any(|s| s == "mass_driver"));

        // A hull is not field-installable — it must wait for a drydock.
        sim.ship.salvage.push("generation_ark".to_owned());
        assert_eq!(
            install_eligibility(&sim, &data, "generation_ark"),
            InstallEligibility::NeedsDrydock
        );
        assert!(install_salvage(&mut sim, &data, "generation_ark").is_err());

        // With no skilled engineer, even a modular part can't be fitted underway.
        sim.crew.retain(|c| c.archetype_id != "engineer");
        sim.ship.salvage.push("flak_screen".to_owned());
        assert_eq!(
            install_eligibility(&sim, &data, "flak_screen"),
            InstallEligibility::NeedsEngineer
        );
    }

    #[test]
    fn salvage_installs_freely_in_port() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.contract = None; // in port
        sim.ship.spare_parts = 0;
        sim.resources.minerals = 0;
        // Even a hull installs in the drydock, with no crew or consumables.
        sim.crew.clear();
        sim.ship.salvage.push("generation_ark".to_owned());
        assert_eq!(
            install_eligibility(&sim, &data, "generation_ark"),
            InstallEligibility::Ready
        );
        install_salvage(&mut sim, &data, "generation_ark").unwrap();
        assert_eq!(sim.ship.hull, "generation_ark");
        assert!(sim.ship.salvage.is_empty());
    }

    #[test]
    fn granted_component_lands_in_the_salvage_hold() {
        use crate::simulation::event_resolver::apply_outcome;
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "wanderers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let template = data.events.get("derelict_encounter").unwrap().clone();
        let idx = template
            .outcomes
            .iter()
            .position(|o| o.grant_component.is_some())
            .expect("derelict_encounter grants a salvage part");
        apply_outcome(&mut sim, &data, &template, idx);
        assert!(
            !sim.ship.salvage.is_empty(),
            "boarding a derelict fills the salvage hold"
        );
    }

    #[test]
    fn commission_refits_and_lifts_hope_but_keeps_the_people() {
        use crate::simulation::contract::start_contract;
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.credits = 100_000;
        sim.resources.minerals = 100_000;
        sim.ship.hull_integrity = 0.3;
        sim.ship.life_support = 0.4;
        sim.ship.spare_parts = 0;
        sim.population.morale = 0.4;
        let drift_before = sim.population.cultural_drift;

        // Underway: refused.
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        assert!(commission_ship(&mut sim, &data, "generation_ark").is_err());

        // In port: swaps hull, full refit, morale lift; the people don't reset.
        sim.contract = None;
        commission_ship(&mut sim, &data, "generation_ark").unwrap();
        assert_eq!(sim.ship.hull, "generation_ark");
        assert_eq!(sim.ship.hull_integrity, 1.0);
        assert_eq!(sim.ship.life_support, 1.0);
        assert!(sim.ship.spare_parts >= data.config.repair.full_parts_restock);
        assert!(sim.population.morale > 0.4, "a fresh hull lifts hope");
        assert_eq!(
            sim.population.cultural_drift, drift_before,
            "commissioning a ship never resets who the people have become"
        );
    }

    #[test]
    fn commission_needs_the_full_price() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.contract = None;
        sim.resources.credits = 0;
        sim.resources.minerals = 0;
        assert!(commission_ship(&mut sim, &data, "generation_ark").is_err());
    }

    #[test]
    fn loadout_effects_add_production_bonus() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let credits = sim.resources.credits;
        let minerals = sim.resources.minerals;

        apply_loadout_effects(&mut sim, &data);

        let stats = loadout_stats(&sim, &data);
        assert_eq!(
            sim.resources.credits,
            credits + stats.speed as i64 * data.config.ship.credits_per_speed
        );
        assert!(sim.resources.minerals > minerals, "cargo yields minerals");
    }

    #[test]
    fn buying_parts_is_port_only_and_charges_per_part() {
        use crate::simulation::contract::start_contract;
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.credits = 10_000;
        let parts_before = sim.ship.spare_parts;

        // Underway: refused.
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        assert!(buy_parts(&mut sim, &data.config, 10).is_err());

        // In port: parts land, credits leave at the configured price.
        sim.contract = None;
        buy_parts(&mut sim, &data.config, 10).unwrap();
        assert_eq!(sim.ship.spare_parts, parts_before + 10);
        assert_eq!(
            sim.resources.credits,
            10_000 - 10 * data.config.provisioning.part_cost_credits
        );

        // Zero or unaffordable orders are refused whole.
        assert!(buy_parts(&mut sim, &data.config, 0).is_err());
        sim.resources.credits = 5;
        assert!(buy_parts(&mut sim, &data.config, 10).is_err());
    }

    #[test]
    fn refuel_is_port_only_and_charges_by_the_missing_fraction() {
        use crate::simulation::contract::start_contract;
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.resources.credits = 100_000;
        sim.ship.fuel = 0.5;

        // Underway: refused.
        let template = data.contracts.get("deep_vein_survey").unwrap().clone();
        sim.contract = Some(start_contract(&template, &sim));
        assert!(
            refuel(&mut sim, &data.config).is_err(),
            "no refuel underway"
        );

        // In port: tops to full and charges missing × cost/point × 100.
        sim.contract = None;
        let before = sim.resources.credits;
        refuel(&mut sim, &data.config).unwrap();
        assert_eq!(sim.ship.fuel, 1.0);
        let expected = (data.config.provisioning.fuel_cost_credits_per_point as f32 * 0.5 * 100.0)
            .ceil() as i64;
        assert_eq!(before - sim.resources.credits, expected);

        // Already full: refused.
        assert!(refuel(&mut sim, &data.config).is_err());
    }
}
