//! What happens when a charter concludes: the pay, the marks it leaves on the
//! ship and its people, the Chronicle entry, and the homecoming report the
//! player is shown. Split out of `actions.rs` (which dispatches every other
//! verb) because the homecoming is the campaign's climax and had grown to the
//! largest single branch in that file.
//!
//! Order matters here. Everything that reads the concluded charter — the pay,
//! the faction sentiment, the lasting legacy, the sealed debrief — must run
//! *before* `sim.contract` is cleared, because clearing it discards the
//! metrics, milestones, and remembered beats the report is built from.

use crate::chronicle::ChronicleEntry;
use crate::game::Game;
use crate::simulation::{contract, debrief};
use crate::state::GameState;
use macroquad::prelude::get_time;

impl Game {
    /// Conclude the active charter at `score` / `level`: pay it out, apply what
    /// it leaves behind, seal the homecoming report, and file the Chronicle
    /// entry. The debrief screen takes over from here until the player
    /// dismisses it.
    pub(in crate::game) fn conclude_contract(&mut self, score: f32, level: contract::SuccessLevel) {
        let GameState::Gameplay(gameplay) = &mut self.state else {
            return;
        };
        let sim = &mut gameplay.sim;
        let entry = ChronicleEntry {
            completed_year: sim.year(),
            contract_name: sim
                .contract
                .as_ref()
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            objective: sim
                .contract
                .as_ref()
                .map(|c| c.objective.label().to_owned())
                .unwrap_or_default(),
            legacy_id: sim.legacy.legacy_id.clone(),
            leader_name: sim
                .dynasty
                .leader()
                .map(|l| l.name.clone())
                .unwrap_or_else(|| "an empty chair".to_owned()),
            generation: sim.dynasty.generation,
            score,
            outcome: level.label().to_owned(),
            duration_years: sim
                .contract
                .as_ref()
                .map(|c| c.months_elapsed / 12)
                .unwrap_or_default(),
        };
        // Freeze the run timer for the Homecoming (PLAN M4.7).
        self.last_mission_real_secs = self.mission_started.map(|t| (get_time() - t) as f32);
        self.mission_started = None;
        // The homecoming is the campaign's emotional climax: lead with
        // level-specific prose (content-depth voice round 4), then the
        // compact record line. The prose is data, indexed by generation so a
        // seed replays it; a missing pool just omits the prose line. It is kept
        // as well as logged, so the debrief can open on it rather than sending
        // the player to the log to find the one line that names the outcome.
        let level_key = entry.outcome.to_lowercase();
        let homecoming_line = self.data.config.flavor.homecoming_line(
            &level_key,
            entry.generation as usize,
            entry.duration_years,
            entry.generation,
        );
        if let Some(line) = &homecoming_line {
            sim.push_log(line.clone());
        }
        sim.push_log(format!(
            "Contract concluded: {} — {} (score {score:.2}).",
            entry.contract_name, entry.outcome
        ));
        // Pay is strictly proportional to objective completion (W2): a
        // full-term run pays in full, a truncated one pays its fraction, and
        // zero objective progress pays nothing. The failure band no longer
        // zeroes pay by itself — objective progress alone decides it.
        // A charter pays by objective completion, and now by the ship's *name* too
        // (content-depth charters round 29): a reputation the writ prizes lifts the pay, a
        // notorious one cuts it — so the character you build by your choices is worth money
        // on the missions that turn on it, not only a key to which missions you are offered.
        let payout = sim
            .contract
            .as_ref()
            .and_then(|c| {
                self.data.contracts.get(&c.template_id).map(|t| {
                    let rep_mult = contract::reputation_reward_multiplier(sim, t);
                    contract::prorated_reward(&t.reward, c.objective_fraction() * rep_mult)
                })
            })
            .unwrap_or_default();
        sim.resources.apply(&payout);
        // …and the crew feels the outcome in their own spirits (content-depth charters round
        // 31): a mission seen through lifts morale (pride in the work), one botched or
        // abandoned dents it (the failure felt), scaled around a middling score — the crew's
        // emotional stake in the ship's purpose, distinct from the pay, the name, and the
        // faction goodwill. Composes with the round-29 despair / round-30 heartening beats: a
        // run of failures drives the crew toward despair, a string of wins lifts them out.
        let morale_shift = contract::mission_outcome_morale_shift(
            score,
            self.data.config.ship.mission_outcome_morale_scale,
        );
        if morale_shift != 0.0 {
            sim.population.morale = (sim.population.morale + morale_shift).clamp(0.0, 1.0);
        }
        // A charter seen through to full term leaves its mark (content-depth
        // charters round 14): the seed of an arc — a survey proves the ground a
        // later colony writ needs, so the follow-on appears on the next board.
        let concluded_template = sim
            .contract
            .as_ref()
            .and_then(|c| self.data.contracts.get(&c.template_id))
            .cloned();
        let mut legacy_line = None;
        if let Some(template) = &concluded_template {
            // …and the people the writ was uniquely called to feel its conclusion
            // (content-depth charters round 32): a founding people gated onto the charter
            // (`requires_faction_aboard`) takes pride when it is seen through and is let down
            // when it is botched — the mission's outcome touching the crew's politics beside
            // its pay, its name, its morale (round 31), and the deed it leaves on record.
            sim.apply_charter_outcome_faction_sentiment(
                &self.data,
                template,
                level == contract::SuccessLevel::Failure,
            );
            if level == contract::SuccessLevel::Failure {
                // A charter defaulted or given up leaves the opposite of a legacy
                // (content-depth charters round 18): the negative mirror of the
                // completion reward. Only a Failure earns the mark; a genuinely
                // completed charter earns the positive legacy below instead.
                if let Some(line) = contract::apply_abandonment(sim, template) {
                    sim.push_log(line);
                }
                // …and a botched charter can leave a lasting deed on the record too
                // (content-depth charters round 30): the dark-deed mirror of the
                // completion_consequence — a mark later writs can read to bar a ship known
                // to have failed the like from being handed it again.
                if !template.failure_consequence.is_empty()
                    && !sim.consequences.contains(&template.failure_consequence)
                {
                    sim.consequences.push(template.failure_consequence.clone());
                }
            } else {
                if !template.completion_consequence.is_empty()
                    && !sim.consequences.contains(&template.completion_consequence)
                {
                    sim.consequences
                        .push(template.completion_consequence.clone());
                }
                // The mission's lasting legacy (content-depth charters round 15): a
                // charter seen through leaves the ship a capability it keeps, beyond
                // the pay it was flown for.
                legacy_line = contract::apply_completion_reward(sim, template, score);
                if let Some(line) = &legacy_line {
                    sim.push_log(line.clone());
                }
            }
        }
        // Seal the homecoming report while the charter is still here to read.
        sim.debrief = debrief::seal(sim, score, level, payout, homecoming_line, legacy_line);
        sim.contract = None;

        self.chronicle.record(entry);
        if let Err(err) = self.chronicle.save(
            &self.data.config.game_name,
            &self.data.config.chronicle_slot,
            &self.data.config.version,
        ) {
            self.notifications
                .danger(format!("Chronicle write failed: {err}"));
        }
    }
}
