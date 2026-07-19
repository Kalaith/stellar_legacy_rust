//! Ship-loadout effects (GDD §5.1, PLAN item 3).
//!
//! The installed hull + engine + weapon each carry [`ComponentStats`]; this
//! module aggregates them and turns them into the yearly deltas the tick
//! applies. Deterministic: it only reads the loadout ids and sums catalog
//! stats — no RNG.

use crate::data::ship_components::{ComponentKind, ComponentStats};
use crate::data::{GameConfig, GameData, ResourceDelta};
use crate::state::sim::SimState;

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
        sim.ship.fuel =
            (sim.ship.fuel + stats.fuel_regen as f32 * cfg.fuel_regen_per_point).min(1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loadout_sums_installed_component_stats() {
        let data = GameData::load().unwrap();
        let sim = SimState::new_campaign(&data, "preservers", 3);
        // Founding loadout: colony_barge hull + ion_drive engine, no weapon.
        let stats = loadout_stats(&sim, &data);
        assert_eq!(stats.cargo, 200); // colony_barge
        assert_eq!(stats.speed, 2); // ion_drive
        assert_eq!(stats.combat, 0); // no weapon
    }

    #[test]
    fn field_repair_patches_but_never_reaches_pristine() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 1);
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
        let mut sim = SimState::new_campaign(&data, "preservers", 1);
        sim.ship.hull_integrity = 0.3;
        sim.ship.spare_parts = 0;
        sim.resources.minerals = 100_000;
        assert!(field_repair(&mut sim, &data.config, RepairKind::Hull).is_err());
    }

    #[test]
    fn full_repair_is_port_only_and_restores_everything() {
        use crate::simulation::contract::start_contract;
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 1);
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
    fn loadout_effects_add_production_bonus() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 3);
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
}
