//! Ship-loadout effects (GDD §5.1, PLAN item 3).
//!
//! The installed hull + engine + weapon each carry [`ComponentStats`]; this
//! module aggregates them and turns them into the yearly deltas the tick
//! applies. Deterministic: it only reads the loadout ids and sums catalog
//! stats — no RNG.

use crate::data::ship_components::{ComponentKind, ComponentStats};
use crate::data::{GameData, ResourceDelta};
use crate::state::sim::SimState;

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
