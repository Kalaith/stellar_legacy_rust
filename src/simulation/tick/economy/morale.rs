//! What the year did to the crew's spirits and the ship's politics.

use crate::data::GameData;
use crate::simulation::{crew, subsystems};
use crate::state::sim::SimState;

use super::super::TickReport;

/// Life-support losses, the corps that steadies the ship, the cohesion its
/// peoples grant or grind away, and what a long lean or plenty does.
pub(super) fn settle_morale_and_politics(
    sim: &mut SimState,
    data: &GameData,
    _report: &mut TickReport,
) {
    let config = &data.config;

    // A life-support plant that has failed badly cannot sustain everyone (content-depth
    // subsystems round 15): the module's most fundamental effect. Below the failure
    // threshold the ship thins each year, scaled by how far the plant has collapsed.
    let ls_loss = subsystems::life_support_mortality_loss(sim, data);
    if ls_loss > 0 {
        sim.population.count = sim.population.count.saturating_sub(ls_loss);
        // Pooled so a failing-air stretch doesn't reprint one line every year it holds
        // (content-depth voice round 24); indexed by year, built-in fallback.
        let pool = &data.config.flavor.life_support_loss;
        let line = if pool.is_empty() {
            format!(
                "The failing life-support could not hold the whole ship in breathable air; {ls_loss} were lost to the thinning decks."
            )
        } else {
            pool[sim.year() as usize % pool.len()].replace("{losses}", &ls_loss.to_string())
        };
        sim.push_log(line);
    }

    // A skilled security chief and a well-kept security corps both slowly steady
    // a fractious ship (content-depth subsystems round 9): crew skill + module
    // condition stack.
    let recovery = crew::unity_recovery(sim, data) + subsystems::security_unity_recovery(sim, data);
    if recovery > 0.0 {
        sim.population.unity = (sim.population.unity + recovery).min(1.0);
    }

    // A functioning security/justice corps also keeps the ship's *institutions*
    // in order (content-depth subsystems round 16): stability's first maintenance
    // counterweight, steadying a fracturing government toward the ceiling.
    let stability_recovery = subsystems::security_stability_recovery(sim, data);
    if stability_recovery > 0.0 {
        sim.population.stability = (sim.population.stability + stability_recovery).min(1.0);
    }

    // A ship holds together as well as its peoples are content (content-depth
    // factions round 15): the faction system finally touches the ship's own
    // cohesion. Each year unity drifts by the member-weighted mood of the aboard
    // peoples — a content polity steadies the ship, a resentful one erodes it —
    // so mistreating your factions doesn't only risk their departure, it wears at
    // the unity the crisis and recovery beats turn on. Neutral mood (0.5) is inert.
    let cohesion = data.config.factions.approval_unity_coupling;
    if cohesion != 0.0 {
        let mood = sim.aboard_approval_mean();
        sim.population.unity = (sim.population.unity + cohesion * (mood - 0.5)).clamp(0.0, 1.0);
    }
    // …and whether the aboard peoples *get along* touches cohesion too (content-depth
    // factions round 23): where the coupling above reads how *content* they are, this
    // reads how they stand *to each other* — two large aboard rivals sharing a hull
    // grind at unity year over year (a standing friction, not only the it14 event-time
    // spillover), while an aboard allied bloc lifts it. So the *composition* of the
    // roster, not just its mood, is a standing cohesion cost or dividend.
    sim.apply_faction_relationship_cohesion(data);

    // A divided ship is harder to govern (content-depth factions round 18): where the
    // approval→unity coupling reads how *content* the peoples are, this reads how
    // ideologically *split* they are — a coalition spanning the tech↔tradition spectrum
    // strains the institutions, eroding `stability` each year its spread runs past the
    // threshold. Distinct from cohesion: a polity can be content yet fractious. A
    // single-minded ship (spread below the line) governs freely. A well-kept security corps
    // now *directly* softens the strain (content-depth subsystems round 28): the peacekeeping
    // corps mediating the divided polity, so the drain the it18 spread inflicts is scaled by
    // `security_spread_relief_factor` — a promise the it18 comment made and this finally wires,
    // distinct from the corps' it16 general stability *recovery* (which lifts a fallen stability
    // back) by reducing the drain at its source. Neutral only within the tolerated spread.
    let spread_penalty = data.config.factions.ideology_spread_stability_penalty;
    if spread_penalty > 0.0 {
        let excess = (sim.aboard_ideology_spread(data)
            - data.config.factions.ideology_spread_threshold)
            .max(0.0);
        if excess > 0.0 {
            let corps_relief = subsystems::security_spread_relief_factor(sim, data);
            sim.population.stability =
                (sim.population.stability - spread_penalty * excess * corps_relief).max(0.0);
        }
    }

    // The habitat is where the people live (content-depth subsystems round 11): a
    // home kept sound lifts the ship's morale year over year, a failing one drags
    // it — the one maintenance-driven counterweight morale has to the voyage strain.
    let habitat = subsystems::habitat_morale_effect(sim, data);
    if habitat != 0.0 {
        sim.population.morale = (sim.population.morale + habitat).clamp(0.0, 1.0);
    }
    // …and the ship's *cultural* life is the other pillar of its spirits (content-depth
    // subsystems round 22): a living education/culture module — schools, arts, the
    // year's festivals, the shared story — lifts morale the way a sound home does, and a
    // hollowed-out one drags it, so a crew can be warm and fed and still grim. The
    // cultural twin of the habitat morale swing, completing morale's environmental map.
    let culture = subsystems::education_morale_effect(sim, data);
    if culture != 0.0 {
        sim.population.morale = (sim.population.morale + culture).clamp(0.0, 1.0);
    }

    // The long lean wears the crew down (content-depth provisioning round 17): the
    // provisioning axis's first *systemic* coupling — where every prior scarcity
    // mechanic was an event gate or a counter, a hunger that has ground on for years
    // now bites the year tick directly. The it89 lean-years counter, until now only
    // gating content and the drift-aware ambient (voice r13), gets a mechanical toll:
    // a chronic hunger doesn't merely read hungry, it *is* wearing. Threshold-gated so
    // one bad winter is inert (the acute famine events' domain) — only a sustained
    // lean grinds the crew's spirits down, and via the ship-mood voice the decks
    // audibly go heavy as it does.
    if config.chronic_hunger_morale_drain > 0.0
        && sim.lean_food_years >= config.chronic_hunger_years
    {
        sim.population.morale =
            (sim.population.morale - config.chronic_hunger_morale_drain).max(0.0);
    }
    // …and it turns the peoples against their government (content-depth provisioning round 28):
    // the *political* toll of a long hunger, beside its toll on the crew's spirits (above) and
    // bodies (the it18 death bonus). A people that goes hungry stops trusting the council that
    // rations it, so a chronic shortage sours every aboard faction — and that discontent feeds
    // the whole faction machinery (the it100 approval→unity cohesion, the withdrawal beats, the
    // it13 demographic drift), so hunger does not only wear the ship but turns its peoples
    // against the leadership. Same "sustained lean" gate; one bad winter is inert.
    if config.chronic_hunger_faction_penalty > 0.0
        && sim.lean_food_years >= config.chronic_hunger_years
    {
        for f in &mut sim.factions {
            if f.is_aboard() {
                f.adjust_approval(-config.chronic_hunger_faction_penalty);
            }
        }
    }
    // …and the long plenty lifts them (content-depth provisioning round 20): the
    // morale mirror of the chronic-hunger drain, on the same "sustained" threshold —
    // a well-fed generation is a happy one, so a fat spell held past `chronic_hunger_
    // years` adds a little morale each year, completing the provisioning→morale pole
    // (hunger wears the spirit, plenty eases it) beside the death/birth poles.
    if config.sustained_plenty_morale_lift > 0.0
        && config.chronic_hunger_years > 0
        && sim.fat_food_years >= config.chronic_hunger_years
    {
        sim.population.morale =
            (sim.population.morale + config.sustained_plenty_morale_lift).min(1.0);
    }
    // …and it warms the peoples toward their government (content-depth provisioning
    // round 31): the *political* mirror of the it28 chronic-hunger faction penalty, on
    // the same sustained-plenty gate as the morale lift above. A people fed well and
    // long comes to trust the council that keeps its holds full — a standing granary is
    // a quiet argument for the leadership — so every aboard faction warms a little each
    // year the fat spell holds, feeding the same faction machinery hunger sours (the
    // it100 approval→unity cohesion, the withdrawal beats, the it13 drift). This closes
    // the food→faction pole (hunger turns them against the council, plenty wins them
    // back) beside the food→morale pole (it17 drain / it20 lift) and the food→body pole
    // (it18 death / it19 birth). Same threshold as the drain; one good winter is inert.
    if config.sustained_plenty_faction_bonus > 0.0
        && config.chronic_hunger_years > 0
        && sim.fat_food_years >= config.chronic_hunger_years
    {
        for f in &mut sim.factions {
            if f.is_aboard() {
                f.adjust_approval(config.sustained_plenty_faction_bonus);
            }
        }
    }
}
