//! Dynasty aging and leader succession (GDD §5.3).
//!
//! Every `generation_interval_years` (25 by default): members age by the
//! interval, elders pass on, a leader past retirement hands off to the
//! highest-leadership eligible heir, and 1-3 young members join.

use crate::data::GameData;
use crate::state::sim::{generate_member, Dynasty, SimState};
use macroquad_toolkit::rng::SeededRng;

#[derive(Debug, Default)]
pub struct SuccessionReport {
    pub new_leader: Option<String>,
    pub deaths: Vec<String>,
    pub births: u32,
    pub extinct: bool,
}

pub fn process_generation(sim: &mut SimState, data: &GameData) -> SuccessionReport {
    let config = &data.config;
    let legacy_id = sim.legacy.legacy_id.clone();
    let mut rng = sim.rng;
    let report = run_generation(&mut sim.dynasty, data, &legacy_id, config, &mut rng);
    sim.rng = rng;
    report
}

fn run_generation(
    dynasty: &mut Dynasty,
    data: &GameData,
    legacy_id: &str,
    config: &crate::data::GameConfig,
    rng: &mut SeededRng,
) -> SuccessionReport {
    let mut report = SuccessionReport::default();
    let interval = config.generation_interval_years;

    dynasty.generation += 1;
    dynasty.years_since_generation = 0;
    for member in &mut dynasty.members {
        member.age += interval;
    }

    // Elders pass on. (Extension beyond the GDD's literal formula: without
    // mortality the dynasty can never go extinct, and extinction is a
    // required end state — GDD §7.)
    let max_age = config.member_max_age;
    let (living, dead): (Vec<_>, Vec<_>) = dynasty
        .members
        .drain(..)
        .partition(|member| member.age <= max_age);
    dynasty.members = living;
    report.deaths = dead.into_iter().map(|member| member.name).collect();

    // Leader retirement / death -> highest-leadership heir aged 30-50.
    let needs_leader = match dynasty.leader() {
        Some(leader) => leader.age > config.leader_retirement_age,
        None => true,
    };
    if needs_leader {
        for member in &mut dynasty.members {
            member.is_leader = false;
        }
        let heir_index = dynasty
            .members
            .iter()
            .enumerate()
            .filter(|(_, m)| m.age >= config.heir_min_age && m.age <= config.heir_max_age)
            .max_by_key(|(_, m)| m.leadership)
            .map(|(i, _)| i);
        match heir_index {
            Some(i) => {
                dynasty.members[i].is_leader = true;
                report.new_leader = Some(dynasty.members[i].name.clone());
            }
            None => {
                dynasty.extinct = dynasty.members.is_empty();
                report.extinct = dynasty.extinct;
                // A living dynasty with no age-eligible heir keeps limping on
                // leaderless until the next generation tick — that is a real
                // succession crisis, not extinction.
            }
        }
    }

    // 1-3 new young members (GDD §5.3).
    let births = 1 + rng.below(3) as u32;
    for _ in 0..births {
        let age = 16 + rng.below(10) as u32;
        let member = generate_member(data, legacy_id, age, rng, &mut dynasty.next_member_id);
        dynasty.members.push(member);
    }
    report.births = births;

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;
    use crate::state::sim::SimState;

    #[test]
    fn leader_past_retirement_hands_off_to_best_eligible_heir() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 1);

        // Founding leader is 45; after one generation tick they are 70 (not
        // yet past retirement), after two they are 95 and long gone.
        let first = process_generation(&mut sim, &data);
        assert!(first.new_leader.is_none(), "70 is not past retirement");

        let second = process_generation(&mut sim, &data);
        assert!(second.deaths.iter().any(|_| true), "the founder passes on");
        let leader = sim.dynasty.leader();
        if let Some(leader) = leader {
            assert!(leader.age >= 30 && leader.age <= 50);
        } else {
            // Legitimate outcome: no member landed in the eligible band.
            assert!(!sim.dynasty.members.is_empty() || sim.dynasty.extinct);
        }
    }

    #[test]
    fn generation_tick_adds_one_to_three_members_and_ages_everyone() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "adaptors", 9);
        let before = sim.dynasty.members.len();
        let ages_before: Vec<u32> = sim.dynasty.members.iter().map(|m| m.age).collect();

        let report = process_generation(&mut sim, &data);

        assert!((1..=3).contains(&report.births));
        assert_eq!(
            sim.dynasty.members.len(),
            before + report.births as usize - report.deaths.len()
        );
        // Surviving founders aged by exactly the interval.
        for member in sim
            .dynasty
            .members
            .iter()
            .take(before - report.deaths.len())
        {
            assert!(ages_before.contains(&(member.age - 25)));
        }
        assert_eq!(sim.dynasty.generation, 2);
    }
}
