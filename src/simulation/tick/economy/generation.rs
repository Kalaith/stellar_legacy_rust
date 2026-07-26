//! Founding Day: everyone gains a year at once and a cohort comes of age.

use crate::data::GameData;
use crate::simulation::{legacy, mortality, subsystems, succession};
use crate::state::sim::SimState;

use super::super::TickReport;

/// Aging, succession, generational renewal, and the voices that read the
/// ship's headcount and fortunes now that the cohort has turned.
pub(super) fn turn_the_generation(sim: &mut SimState, data: &GameData, report: &mut TickReport) {
    let config = &data.config;

    // Founding Day (real-time loop follow-up): everyone gains a year at once, and
    // any officer aged past their term stands down. Aging is yearly; death is the
    // separate monthly roll in `mortality::monthly_tick` (driven from the tick).
    mortality::annual_aging(sim, data);
    // …and give the ship's *headcount* a voice (content-depth voice round 30), now that the
    // year's aging and mortality have settled on the count: when the crew swells past its founding
    // complement (the cradles full, new decks opened) or thins below it (corridors gone quiet,
    // decks closed), the decks remark it once — the gentle crossing the it12 depopulation beat and
    // the hollow ambient never gave, and the *only* narration the growth side has at all.
    sim.announce_crew_size_mood(data);
    // …and when the ship's material fortune turns (content-depth voice round 32): the treasury
    // crossing into flush (the coffers full, the council debating what to build) or bare (every
    // credit counted twice), read against the founding stake — the ledger's turning remarked once.
    sim.announce_treasury_mood(data);
    // …and when its power fortune turns (content-depth voice round 33): the energy store crossing
    // into flush (reactors past the surplus line, everything lit) or dark (the grid near the
    // life-support and production lines, decks on rationed light) — the money voice's power sibling.
    sim.announce_power_mood(data);
    // …and when the demographic drift finally hands the ship from one people to another (content-
    // depth voice round 31): who runs the ship — the largest aboard — bends the it10 dilemma odds,
    // the it16 reputation lean, and the it21 ambient, but the turning itself went unremarked. When
    // the dominant people changes, the decks remark the changing of the guard once.
    sim.announce_ruling_people(data);

    // Generational renewal (GDD §5.3): every interval a new cohort comes of age.
    // Aging, death, and succession are continuous now and live in `mortality`;
    // this tick only adds the young and runs the once-a-generation beats.
    sim.dynasty.years_since_generation += 1;
    if !sim.dynasty.extinct
        && sim.dynasty.years_since_generation >= config.generation_interval_years
    {
        let births = succession::process_generation(sim, data);
        let gen_index = sim.dynasty.generation as usize;
        let flavor = &data.config.flavor;
        if births > 0 && !flavor.coming_of_age.is_empty() {
            let pool = &flavor.coming_of_age;
            let line = pool[gen_index % pool.len()]
                .replace("{generation}", &sim.dynasty.generation.to_string())
                .replace("{births}", &births.to_string());
            sim.push_log(line);
        }

        // Each people's numbers wax or wane over the generations (content-depth
        // factions round 11): the balance of power shifts, so which people runs
        // the ship can change mid-voyage. Applied before assimilation, so a people
        // that dwindles far enough can then be folded into a larger one.
        sim.apply_faction_demographic_drift(data);

        // A generation of drift can quietly fold a dwindling faction into a
        // larger one (W7 soft assimilation).
        sim.assimilate_drifted_factions(data);
        // Knowledge dies with the people; the education subsystem passes it
        // forward (W5). A generation with no schooling loses expertise.
        subsystems::transmit_knowledge(sim, data);

        // Each new generation may confront its legacy's defining dilemma
        // (GDD §5.5). Dilemmas always block — they are never delegated.
        if let Some(pending) = legacy::roll_dilemma(sim, data) {
            if let Some(dilemma) = data
                .legacies
                .get(&sim.legacy.legacy_id)
                .and_then(|l| l.dilemmas.iter().find(|d| d.id == pending.dilemma_id))
            {
                sim.push_log(format!(
                    "The new generation faces a reckoning: {}",
                    dilemma.title
                ));
            }
            sim.pending_dilemma = Some(pending);
            report.decision_required = true;
        }
    }
}
