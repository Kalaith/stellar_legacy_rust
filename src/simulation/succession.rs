//! Dynasty generational renewal and leader succession (GDD §5.3).
//!
//! Aging and death are now continuous (see [`crate::simulation::mortality`]):
//! everyone ages a year on Founding Day and faces a monthly death roll. What
//! remains generational is *renewal* — every `generation_interval_years` a new
//! cohort of young members joins ([`process_generation`]) — and *succession*,
//! which the mortality tick drives through [`install_successor`] whenever the
//! leader's seat falls empty or a retirement-aged leader has an heir ready.

use crate::data::{GameConfig, GameData};
use crate::state::sim::{Dynasty, DynastyMember, SimState};

/// True while at least one non-leader member sits in the eligible heir band.
pub fn eligible_heir_exists(dynasty: &Dynasty, config: &GameConfig) -> bool {
    dynasty
        .members
        .iter()
        .any(|m| !m.is_leader && m.age >= config.heir_min_age && m.age <= config.heir_max_age)
}

/// Install a new leader (GDD §4 Select Heir): clear the current leader, then take
/// the council-designated heir if one is living and age-eligible, otherwise the
/// highest-leadership member in the ideal heir band, and failing that the best of
/// whoever remains — a ship is never left without a captain while anyone lives.
/// Returns the new leader's name (if any member remained) and whether the dynasty
/// is now extinct (no members at all).
pub fn install_successor(dynasty: &mut Dynasty, config: &GameConfig) -> (Option<String>, bool) {
    for member in &mut dynasty.members {
        member.is_leader = false;
    }
    let eligible = |m: &DynastyMember| m.age >= config.heir_min_age && m.age <= config.heir_max_age;
    let best_in_band = || {
        dynasty
            .members
            .iter()
            .enumerate()
            .filter(|(_, m)| eligible(m))
            .max_by_key(|(_, m)| m.leadership)
            .map(|(i, _)| i)
    };
    // Fallback when no one sits in the ideal band: the strongest survivor still
    // leads — an unusually young or old captain, but a captain. Ties break on id
    // so a given roster is deterministic.
    let best_any = || {
        dynasty
            .members
            .iter()
            .enumerate()
            .max_by_key(|(_, m)| (m.leadership, m.id))
            .map(|(i, _)| i)
    };
    let heir_index = dynasty
        .designated_heir
        .and_then(|id| {
            dynasty
                .members
                .iter()
                .position(|m| m.id == id && eligible(m))
        })
        .or_else(best_in_band)
        .or_else(best_any);
    dynasty.designated_heir = None;
    match heir_index {
        Some(i) => {
            dynasty.members[i].is_leader = true;
            (Some(dynasty.members[i].name.clone()), false)
        }
        None => {
            dynasty.extinct = dynasty.members.is_empty();
            (None, dynasty.extinct)
        }
    }
}

/// Mark a new generation (GDD §5.3): advance the generation counter and return
/// the young adults who have come of age since the last one (births are yearly
/// now — see `mortality::annual_aging`; this only closes the generational ledger
/// and reports its tally for the coming-of-age line).
pub fn process_generation(sim: &mut SimState, _data: &GameData) -> u32 {
    sim.dynasty.generation += 1;
    sim.dynasty.years_since_generation = 0;
    let born = sim.dynasty.births_this_generation;
    sim.dynasty.births_this_generation = 0;
    born
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::sim::SimState;

    #[test]
    fn a_vacated_seat_hands_off_to_the_best_eligible_heir() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );

        // The leader dies: clear the flag, then install a successor.
        for member in &mut sim.dynasty.members {
            member.is_leader = false;
        }
        let (new_leader, extinct) = install_successor(&mut sim.dynasty, &data.config);
        assert!(!extinct, "the founding dynasty is not extinct");
        assert!(new_leader.is_some(), "a founding heir stands ready");
        let leader = sim.dynasty.leader().expect("a leader was installed");
        assert!(leader.age >= data.config.heir_min_age && leader.age <= data.config.heir_max_age);
    }

    #[test]
    fn designated_heir_takes_precedence_over_best_leadership() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            12,
            &crate::state::sim::founding_faction_ids(&data),
        );

        // Designate the weakest eligible member instead of the strongest.
        let eligible: Vec<(u32, u32)> = sim
            .dynasty
            .members
            .iter()
            .filter(|m| {
                !m.is_leader
                    && m.age >= data.config.heir_min_age
                    && m.age <= data.config.heir_max_age
            })
            .map(|m| (m.id, m.leadership))
            .collect();
        let weakest = eligible
            .iter()
            .min_by_key(|(_, leadership)| *leadership)
            .map(|(id, _)| *id)
            .expect("founding dynasty has eligible members");
        sim.dynasty.designated_heir = Some(weakest);

        let (new_leader, _) = install_successor(&mut sim.dynasty, &data.config);
        assert!(new_leader.is_some());
        assert_eq!(
            sim.dynasty.leader().map(|l| l.id),
            Some(weakest),
            "the designated heir must inherit even with lower leadership"
        );
        assert!(sim.dynasty.designated_heir.is_none(), "consumed on use");
    }

    #[test]
    fn marking_a_generation_advances_the_counter_and_reports_its_births() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "adaptors",
            9,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let before = sim.dynasty.members.len();
        // Births are yearly now; the generation mark only closes the ledger.
        sim.dynasty.births_this_generation = 4;

        let reported = process_generation(&mut sim, &data);

        assert_eq!(
            reported, 4,
            "the accumulated coming-of-age tally is reported"
        );
        assert_eq!(sim.dynasty.births_this_generation, 0, "the tally resets");
        assert_eq!(
            sim.dynasty.members.len(),
            before,
            "the mark itself adds no one"
        );
        assert_eq!(sim.dynasty.generation, 2);
    }
}
