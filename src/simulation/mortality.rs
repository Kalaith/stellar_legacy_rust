//! Per-character aging and death (real-time loop follow-up).
//!
//! Aboard a generation ship, everyone shares a birthday: the last day of the
//! year is "Founding Day", and every living soul gains a year at once whatever
//! their true birthdate. So [`annual_aging`] runs on each year boundary and adds
//! one year to every dynasty member and crew officer.
//!
//! *Death*, by contrast, is a monthly roll — [`monthly_tick`] gives each living
//! character a chance to die every month, low for the young and climbing with
//! age past `onset_age`, certain at `member_max_age`. A dead leader (or one who
//! has aged past retirement with an heir waiting) triggers succession here too.
//! A heavy population-loss event can additionally claim a named character via
//! [`event_claim`]. All rolls flow through the sim's seeded RNG.

use crate::data::{FlavorConfig, GameData, MortalityConfig};
use crate::simulation::tick::TickReport;
use crate::simulation::{subsystems, succession};
use crate::state::sim::{generate_member, CrewMember, DynastyMember, SimState};

/// The chance a character of `age` dies in a given month: a flat accident floor
/// at any age, plus an age-scaled term that switches on at `onset_age` and
/// doubles every `doubling_years`. Certain (1.0) at or past `max_age`.
pub fn monthly_death_chance(age: u32, cfg: &MortalityConfig, max_age: u32) -> f32 {
    if age >= max_age {
        return 1.0;
    }
    let mut chance = cfg.monthly_accident_chance;
    if age >= cfg.onset_age && cfg.doubling_years > 0.0 {
        let over = (age - cfg.onset_age) as f32;
        chance += cfg.monthly_base_chance * 2f32.powf(over / cfg.doubling_years);
    }
    chance.clamp(0.0, 1.0)
}

/// The shared "Founding Day" birthday (real-time loop follow-up): on each year
/// boundary every living character gains a year. Crew who cross their retirement
/// age stand down (a vacancy, not a death); their departures are logged.
pub fn annual_aging(sim: &mut SimState, data: &GameData) {
    for member in &mut sim.dynasty.members {
        member.age += 1;
    }
    for officer in &mut sim.crew {
        officer.age += 1;
    }
    // Count the sitting leader's reign each Founding Day (content-depth campaign
    // skeleton round 19); succession resets it. An enduring captaincy is the
    // long-reign beat's trigger.
    if sim.dynasty.leader().is_some() {
        sim.dynasty.leader_reign_years += 1;
    }

    // Officers past their term retire — the post falls vacant, to be re-crewed
    // in drydock. Distinct from the death roll below (they leave alive).
    let retirement = data.config.crew.retirement_age;
    let mut retired: Vec<CrewMember> = Vec::new();
    sim.crew.retain(|officer| {
        let leaving = officer.age > retirement;
        if leaving {
            retired.push(officer.clone());
        }
        !leaving
    });
    for officer in &retired {
        let post = post_name(data, &officer.archetype_id);
        let line = FlavorConfig::line_with_name(
            &data.config.flavor.retirement,
            officer.id as usize,
            &officer.name,
        )
        .unwrap_or_else(|| format!("{} stood down as {post}.", officer.name));
        sim.push_log(line);
    }

    // Renewal (real-time loop follow-up): young adults come of age to fill the
    // line back toward its target, the counterweight to the death roll. It takes
    // two to carry a line on — a dynasty down to one cannot renew and is doomed —
    // and each open slot below the target rolls once. The generation counter and
    // its coming-of-age line still track this, once every interval.
    let cfg = &data.config.mortality;
    let count = sim.dynasty.members.len() as u32;
    if count >= 2 && count < cfg.dynasty_target_size {
        // A failing home raises fewer children (content-depth subsystems round 19):
        // the habitat's condition scales the yearly birth chance. …and a ship long in
        // plenty fills its cradles (content-depth provisioning round 19): sustained fat
        // years lift the same chance, the positive pole of the chronic-hunger toll.
        let plenty = if data.config.chronic_hunger_years > 0
            && sim.fat_food_years >= data.config.chronic_hunger_years
        {
            1.0 + data.config.sustained_plenty_birth_bonus
        } else {
            1.0
        };
        // The home raises the young (r19 habitat) and the infirmary keeps them alive to
        // grow up (content-depth subsystems round 23) — housing × healthcare both scale
        // how many of the cohort reach their majority.
        let birth_chance = cfg.annual_birth_chance
            * subsystems::habitat_renewal_factor(sim, data)
            * subsystems::medical_renewal_factor(sim, data)
            * plenty;
        let legacy_id = sim.legacy.legacy_id.clone();
        let mut rng = sim.rng;
        let slots = cfg.dynasty_target_size - count;
        let mut born = 0u32;
        for _ in 0..slots {
            if rng.chance(birth_chance) {
                let age = 16 + rng.below(10) as u32;
                let member = generate_member(
                    data,
                    &legacy_id,
                    age,
                    &mut rng,
                    &mut sim.dynasty.next_member_id,
                );
                sim.dynasty.members.push(member);
                born += 1;
            }
        }
        sim.rng = rng;
        sim.dynasty.births_this_generation += born;
    }
}

/// One month of the death roll (real-time loop follow-up): every living dynasty
/// member and crew officer faces `monthly_death_chance`. Deaths are logged and a
/// vacated leadership triggers succession; the report carries out whether the
/// dynasty died out (so the game ends) and whether a *sitting leader* fell this
/// month (so the skeleton can force a succession beat).
pub fn monthly_tick(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    let max_age = data.config.member_max_age;
    let cfg = &data.config.mortality;
    // A well-kept infirmary thins the reaper's odds (content-depth subsystems round
    // 18): read the bay's relief once, applied to every pre-cap death roll below.
    let relief = subsystems::medical_mortality_relief(sim, data);
    // A hunger that has ground on for years wears bodies, not just spirits
    // (content-depth provisioning round 18): a flat monthly toll added to the age
    // curve while the ship sits in sustained lean past `chronic_hunger_years`.
    let lean_bonus = if data.config.chronic_hunger_years > 0
        && sim.lean_food_years >= data.config.chronic_hunger_years
    {
        data.config.chronic_hunger_death_bonus
    } else {
        0.0
    };
    let chance_for = |age: u32| {
        // The bay eases these deaths but nothing cheats the hard age cap.
        if age >= max_age {
            return 1.0;
        }
        ((monthly_death_chance(age, cfg, max_age) + lean_bonus) * (1.0 - relief)).clamp(0.0, 1.0)
    };

    // Roll deaths through a local copy of the seeded RNG, then write it back
    // (avoids borrowing `sim.rng` while draining `sim.dynasty`/`sim.crew`).
    let mut rng = sim.rng;
    let mut dead: Vec<DynastyMember> = Vec::new();
    sim.dynasty.members.retain(|member| {
        let dies = rng.chance(chance_for(member.age));
        if dies {
            dead.push(member.clone());
        }
        !dies
    });
    let mut crew_dead: Vec<CrewMember> = Vec::new();
    sim.crew.retain(|officer| {
        let dies = rng.chance(chance_for(officer.age));
        if dies {
            crew_dead.push(officer.clone());
        }
        !dies
    });
    sim.rng = rng;

    for member in &dead {
        let line = FlavorConfig::line_with_name(
            &data.config.flavor.obituary,
            member.id as usize,
            &member.name,
        )
        .unwrap_or_else(|| format!("{} passed away, aged {}.", member.name, member.age));
        sim.push_log(line);
    }
    for officer in &crew_dead {
        let post = post_name(data, &officer.archetype_id);
        let line = FlavorConfig::line_with_name_post(
            &data.config.flavor.crew_death,
            officer.id as usize,
            &officer.name,
            post,
        )
        .unwrap_or_else(|| {
            format!(
                "{}, the ship's {post}, died at {}.",
                officer.name, officer.age
            )
        });
        sim.push_log(line);
    }

    // A sitting leader falling in office (not a planned retirement handoff) is a
    // succession the ship did not choose — flag it for the skeleton's beat.
    if dead.iter().any(|m| m.is_leader) {
        report.leader_died = true;
    }

    // Succession: the seat is empty (the leader died), or the leader has aged
    // past retirement and an eligible heir is ready to take over.
    let leader_gone = sim.dynasty.leader().is_none();
    let leader_retired = sim
        .dynasty
        .leader()
        .is_some_and(|l| l.age > data.config.leader_retirement_age);
    if leader_gone
        || (leader_retired && succession::eligible_heir_exists(&sim.dynasty, &data.config))
    {
        let (new_leader, _) = succession::install_successor(&mut sim.dynasty, &data.config);
        if let Some(name) = new_leader {
            let idx = sim.dynasty.next_member_id as usize; // varies per handoff
            if let Some(line) =
                FlavorConfig::line_with_name(&data.config.flavor.succession, idx, &name)
            {
                sim.push_log(line);
            }
        }
    }

    // The last of the line gone is the campaign's end state (GDD §7). Announce it
    // once, on the crossing into extinction.
    if sim.dynasty.members.is_empty() && !sim.dynasty.extinct {
        sim.dynasty.extinct = true;
        let line = FlavorConfig::line_with_name(&data.config.flavor.extinction, 0, "")
            .unwrap_or_else(|| "The dynasty has no heirs. The line ends here.".to_owned());
        sim.push_log(line);
    }
    if sim.dynasty.extinct {
        report.dynasty_extinct = true;
        // Extinction supersedes any succession beat this same month.
        report.leader_died = false;
    }
}

/// A heavy population-loss outcome may also claim a named character (real-time
/// loop follow-up: "a random chance of dying … especially due to an event"). When
/// the loss meets `event_death_loss_threshold`, one roll against
/// `event_death_chance` takes a crew officer if any serve (they are in harm's
/// way), else a non-leader relative. The leader is spared here — only the age
/// roll unseats them, so a mid-event succession never surprises the player.
pub fn event_claim(sim: &mut SimState, data: &GameData, population_lost: u32) {
    let cfg = &data.config.mortality;
    if population_lost < cfg.event_death_loss_threshold {
        return;
    }
    if !sim.rng.chance(cfg.event_death_chance) {
        return;
    }
    if !sim.crew.is_empty() {
        let idx = sim.rng.below(sim.crew.len());
        let officer = sim.crew.remove(idx);
        let post = post_name(data, &officer.archetype_id).to_owned();
        // Pooled so a disaster-heavy voyage's many losses don't read as a form letter
        // (content-depth voice round 24); indexed by log length so consecutive vary.
        let pool = &data.config.flavor.event_loss_officer;
        let line = if pool.is_empty() {
            format!("{}, the ship's {post}, was among the lost.", officer.name)
        } else {
            pool[sim.log.len() % pool.len()]
                .replace("{name}", &officer.name)
                .replace("{post}", &post)
        };
        sim.push_log(line);
        return;
    }
    let candidates: Vec<usize> = sim
        .dynasty
        .members
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.is_leader)
        .map(|(i, _)| i)
        .collect();
    if !candidates.is_empty() {
        let pick = candidates[sim.rng.below(candidates.len())];
        let member = sim.dynasty.members.remove(pick);
        let pool = &data.config.flavor.event_loss_member;
        let line = if pool.is_empty() {
            format!(
                "{} was lost with the others — a name struck from the register.",
                member.name
            )
        } else {
            pool[sim.log.len() % pool.len()].replace("{name}", &member.name)
        };
        sim.push_log(line);
    }
}

/// The ship's human name for an archetype's post, falling back to the raw id.
fn post_name<'a>(data: &'a GameData, archetype_id: &'a str) -> &'a str {
    data.crew_archetypes
        .iter()
        .find(|a| a.id == archetype_id)
        .map_or(archetype_id, |a| a.name.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    #[test]
    fn death_chance_rises_with_age_and_is_certain_at_the_cap() {
        let data = GameData::load().unwrap();
        let cfg = &data.config.mortality;
        let max = data.config.member_max_age;
        let young = monthly_death_chance(20, cfg, max);
        let onset = monthly_death_chance(cfg.onset_age, cfg, max);
        let old = monthly_death_chance(cfg.onset_age + cfg.doubling_years as u32, cfg, max);
        assert!(young < onset, "the young are far safer than the old");
        assert!(old > onset, "risk climbs past the onset age");
        assert!(
            onset >= cfg.monthly_base_chance,
            "onset age carries the base risk"
        );
        assert_eq!(
            monthly_death_chance(max, cfg, max),
            1.0,
            "certain at the cap"
        );
        assert_eq!(monthly_death_chance(max + 5, cfg, max), 1.0);
    }

    #[test]
    fn founding_day_ages_everyone_by_a_year() {
        let data = GameData::load().unwrap();
        let mut sim = crate::state::sim::SimState::new_campaign(
            &data,
            "preservers",
            1,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let dyn_before: Vec<u32> = sim.dynasty.members.iter().map(|m| m.age).collect();
        let crew_before: Vec<u32> = sim.crew.iter().map(|c| c.age).collect();
        annual_aging(&mut sim, &data);
        for (member, before) in sim.dynasty.members.iter().zip(&dyn_before) {
            assert_eq!(member.age, before + 1);
        }
        for (officer, before) in sim.crew.iter().zip(&crew_before) {
            assert_eq!(officer.age, before + 1);
        }
    }

    #[test]
    fn sustained_plenty_fills_the_cradles_faster_than_lean() {
        // Content-depth provisioning round 19: a ship long in plenty lifts the yearly
        // renewal — the positive pole of the chronic-hunger death toll.
        let mut data = GameData::load().unwrap();
        // Decisive plenty: fed, the boosted chance clears 1.0 and every open cradle
        // fills; lean, only the base chance applies.
        data.config.mortality.annual_birth_chance = 0.3;
        data.config.sustained_plenty_birth_bonus = 4.0;
        let base = crate::state::sim::SimState::new_campaign(
            &data,
            "preservers",
            7,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let target = data.config.mortality.dynasty_target_size as usize;

        // Thin the line to two (the minimum that can renew) with a sound home.
        let mut fed = base.clone();
        fed.dynasty.members.truncate(2);
        if let Some(h) = fed.subsystems.get_mut("life_support_habitat") {
            h.condition = 1.0;
        }
        let mut lean = fed.clone();

        fed.fat_food_years = data.config.chronic_hunger_years.max(1);
        lean.fat_food_years = 0;
        annual_aging(&mut fed, &data);
        annual_aging(&mut lean, &data);

        assert_eq!(
            fed.dynasty.members.len(),
            target,
            "sustained plenty fills every open cradle"
        );
        assert!(
            fed.dynasty.members.len() > lean.dynasty.members.len(),
            "and faster than a lean ship raises its young"
        );
    }

    #[test]
    fn a_leader_dying_in_office_is_flagged_for_the_succession_beat() {
        // The succession beat (campaign-skeleton round 18) keys on a sitting
        // leader falling — so the monthly tick must flag it, and only for a death
        // in office, not a routine survival.
        let data = GameData::load().unwrap();
        let mut sim = crate::state::sim::SimState::new_campaign(
            &data,
            "preservers",
            4,
            &crate::state::sim::founding_faction_ids(&data),
        );

        // No one due to die: the flag stays down.
        let mut report = crate::simulation::tick::TickReport::default();
        monthly_tick(&mut sim, &data, &mut report);
        assert!(!report.leader_died, "no death in office means no flag");

        // Force the sitting leader to certain death (age at the cap), leaving an
        // eligible heir among the founders.
        let max_age = data.config.member_max_age;
        for member in &mut sim.dynasty.members {
            if member.is_leader {
                member.age = max_age;
            }
        }
        let mut report = crate::simulation::tick::TickReport::default();
        monthly_tick(&mut sim, &data, &mut report);
        assert!(
            report.leader_died,
            "the leader's death in office is flagged"
        );
        assert!(!report.dynasty_extinct, "an heir carries the line on");
        assert!(sim.dynasty.leader().is_some(), "the seat is filled at once");
    }

    #[test]
    fn a_long_hunger_raises_the_death_roll() {
        // Content-depth provisioning round 18: a sustained lean past
        // `chronic_hunger_years` adds a monthly death toll on top of the age curve.
        let mut data = GameData::load().unwrap();
        // Isolate the hunger toll: no accident floor, a decisive hunger bonus.
        data.config.mortality.monthly_accident_chance = 0.0;
        data.config.chronic_hunger_death_bonus = 1.0;
        let mut sim = crate::state::sim::SimState::new_campaign(
            &data,
            "preservers",
            3,
            &crate::state::sim::founding_faction_ids(&data),
        );
        // Young members (below the age onset) so any death is the hunger's doing,
        // and a wrecked bay so the toll lands in full.
        for member in &mut sim.dynasty.members {
            member.age = 25;
        }
        if let Some(bay) = sim.subsystems.get_mut("medical_bay") {
            bay.condition = 0.0;
        }
        let founders = sim.dynasty.members.len();

        // Well-fed: the young are safe.
        sim.lean_food_years = 0;
        let mut report = crate::simulation::tick::TickReport::default();
        monthly_tick(&mut sim, &data, &mut report);
        assert_eq!(
            sim.dynasty.members.len(),
            founders,
            "a fed ship's young do not die of hunger"
        );

        // A hunger years past the threshold: it takes even the young.
        sim.lean_food_years = data.config.chronic_hunger_years.max(1);
        let mut report = crate::simulation::tick::TickReport::default();
        monthly_tick(&mut sim, &data, &mut report);
        assert!(
            sim.dynasty.members.len() < founders,
            "a long hunger thins the roster on top of the age curve"
        );
    }
}
