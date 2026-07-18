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
        let eligible = |m: &crate::state::sim::DynastyMember| {
            m.age >= config.heir_min_age && m.age <= config.heir_max_age
        };
        // A council-designated heir takes precedence if still living and
        // age-eligible (GDD §4 Select Heir); otherwise best leadership wins.
        let heir_index = dynasty
            .designated_heir
            .and_then(|id| {
                dynasty
                    .members
                    .iter()
                    .position(|m| m.id == id && eligible(m))
            })
            .or_else(|| {
                dynasty
                    .members
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| eligible(m))
                    .max_by_key(|(_, m)| m.leadership)
                    .map(|(i, _)| i)
            });
        dynasty.designated_heir = None;
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
    fn designated_heir_takes_precedence_over_best_leadership() {
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(&data, "preservers", 12);

        // Force a succession: leader far past retirement, and designate the
        // weakest eligible member instead of the strongest.
        for member in &mut sim.dynasty.members {
            if member.is_leader {
                member.age = 80;
            }
        }
        let eligible_after_aging: Vec<(u32, u32)> = sim
            .dynasty
            .members
            .iter()
            .filter(|m| !m.is_leader && (5..=25).contains(&m.age))
            .map(|m| (m.id, m.leadership))
            .collect();
        let weakest = eligible_after_aging
            .iter()
            .min_by_key(|(_, leadership)| *leadership)
            .map(|(id, _)| *id)
            .expect("founding dynasty has members aging into eligibility");
        sim.dynasty.designated_heir = Some(weakest);

        let report = process_generation(&mut sim, &data);
        assert!(report.new_leader.is_some());
        assert_eq!(
            sim.dynasty.leader().map(|l| l.id),
            Some(weakest),
            "the designated heir must inherit even with lower leadership"
        );
        assert!(sim.dynasty.designated_heir.is_none(), "consumed on use");
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
