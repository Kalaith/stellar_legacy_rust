//! The peoples' voice speaks once on a turn, not every year after it.

use super::*;

#[test]
fn a_souring_people_says_so_once_not_every_year() {
    // Content-depth voice round 8: the approval meter's voice. A people
    // crossing into restlessness surfaces one pooled line, then stays quiet
    // while it remains there — no yearly reprint — and a recovery to
    // contentment gets its own, opposite line.
    let (data, mut sim, _picks) = armed(14);
    let target = sim.factions.iter().find(|f| f.is_aboard()).unwrap();
    let id = target.faction_id.clone();
    let name = log_name(&data.factions, &id);
    let restless = |sim: &SimState| {
        sim.log
            .iter()
            .filter(|l| l.text.contains(&name) && l.text.contains("restless"))
            .count()
    };

    // A neutral people says nothing.
    sim.announce_faction_moods(&data);
    assert_eq!(restless(&sim), 0, "a content people is silent");

    // Sour them past the restless line — one announcement.
    sim.factions
        .iter_mut()
        .find(|f| f.faction_id == id)
        .unwrap()
        .approval = 0.2;
    sim.announce_faction_moods(&data);
    let after_first = restless(&sim);
    assert_eq!(after_first, 1, "crossing into restlessness says so once");

    // Still restless the next year — no reprint.
    sim.announce_faction_moods(&data);
    assert_eq!(restless(&sim), 1, "staying restless is not re-announced");

    // Win them all the way back — a warming line, distinct from the souring.
    sim.factions
        .iter_mut()
        .find(|f| f.faction_id == id)
        .unwrap()
        .approval = 0.85;
    let log_before = sim.log.len();
    sim.announce_faction_moods(&data);
    assert!(
        sim.log.len() > log_before,
        "a people won back to contentment says so"
    );
}
#[test]
fn the_ships_collective_mood_says_so_once_when_it_turns() {
    // Content-depth voice round 11: the ship-wide morale voice. Crossing into a
    // grim band surfaces one pooled line, then stays quiet while it sits there,
    // and a recovery into a buoyant band gets its own, opposite line.
    let (data, mut sim, _picks) = armed(19);
    let mood_lines = |sim: &SimState| {
        let dark = &data.config.flavor.ship_mood_darkening;
        let light = &data.config.flavor.ship_mood_lifting;
        sim.log
            .iter()
            .filter(|l| {
                dark.iter().chain(light.iter()).any(|p| {
                    // Match on a distinctive opening clause so we count only
                    // these pooled lines, not other log text.
                    l.text.contains("heaviness has settled")
                        || l.text.contains("lightness has come")
                        || l.text.contains("mood aboard has turned")
                        || l.text.contains("greyness in the crew")
                        || l.text.contains("gone out of the ship's spirit")
                        || l.text.contains("low season")
                        || l.text.contains("something has lifted")
                        || l.text.contains("warmth has spread")
                        || l.text.contains("happy this season")
                        || p == &l.text
                })
            })
            .count()
    };

    // At its launch baseline the ship says nothing (the starting band is
    // recorded, not announced).
    sim.announce_ship_mood(&data);
    assert_eq!(mood_lines(&sim), 0, "the launch baseline is silent");

    // Sink the crew into a grim band — one announcement.
    sim.population.morale = 0.2;
    sim.announce_ship_mood(&data);
    assert_eq!(mood_lines(&sim), 1, "the decks going grim says so once");
    assert_eq!(sim.morale_band, -1);

    // Still grim next year — no reprint.
    sim.announce_ship_mood(&data);
    assert_eq!(mood_lines(&sim), 1, "staying grim is not re-announced");

    // Lift them into a buoyant band — a second, distinct line.
    sim.population.morale = 0.85;
    sim.announce_ship_mood(&data);
    assert_eq!(mood_lines(&sim), 2, "the ship lifting says so afresh");
    assert_eq!(sim.morale_band, 1);
}
#[test]
fn the_ships_political_climate_says_so_once_when_it_turns() {
    // Content-depth voice round 15: the polity-mood voice. Distinct from the
    // crew's spirits and from any one people's mood, this reads the aggregate
    // mood of the aboard peoples. Crossing into broad discontent surfaces one
    // pooled line; a return to broad ease gets its own, opposite line.
    let (data, mut sim, _picks) = armed(23);
    let polity_lines = |sim: &SimState| {
        let sour = &data.config.flavor.polity_souring;
        let warm = &data.config.flavor.polity_warming;
        sim.log
            .iter()
            .filter(|l| sour.contains(&l.text) || warm.contains(&l.text))
            .count()
    };

    // A fairly-treated polity (launch approvals 0.5) says nothing.
    sim.announce_polity_mood(&data);
    assert_eq!(polity_lines(&sim), 0, "a fairly-treated polity is silent");

    // Sour every aboard people: the whole political climate curdles — one line.
    for f in sim.factions.iter_mut().filter(|f| f.is_aboard()) {
        f.approval = 0.15;
    }
    sim.announce_polity_mood(&data);
    assert_eq!(polity_lines(&sim), 1, "the polity curdling says so once");
    assert_eq!(sim.polity_mood_band, -1);

    // Still sour next year — no reprint.
    sim.announce_polity_mood(&data);
    assert_eq!(polity_lines(&sim), 1, "staying sour is not re-announced");

    // Win them all back: the climate turns to broad ease — a second, distinct line.
    for f in sim.factions.iter_mut().filter(|f| f.is_aboard()) {
        f.approval = 0.9;
    }
    sim.announce_polity_mood(&data);
    assert_eq!(polity_lines(&sim), 2, "the polity settling says so afresh");
    assert_eq!(sim.polity_mood_band, 1);
}
#[test]
fn the_ship_remarks_when_it_passes_into_new_hands() {
    // Content-depth voice round 31: the ruling-people voice, the first keyed to a change in
    // *who runs the ship* rather than a stat crossing a band. The launch majority is the silent
    // baseline; when demographic drift hands the ship to a new largest people, the decks remark
    // the changing of the guard once, naming the newcomers; staying under the same people does
    // not reprint.
    let data = GameData::load().unwrap();
    assert!(
        !data.config.flavor.ruling_people_change.is_empty(),
        "this test needs the ruling-people voice enabled"
    );
    let picks = crate::state::sim::founding_faction_ids(&data);
    // Count log lines that announce `name` taking the ship (a pooled line with {name} filled).
    let ruling_lines = |sim: &SimState, name: &str| {
        let subs: Vec<String> = data
            .config
            .flavor
            .ruling_people_change
            .iter()
            .map(|p| p.replace("{name}", name))
            .collect();
        sim.log.iter().filter(|l| subs.contains(&l.text)).count()
    };

    // A fresh ship records its founding majority silently — the launch is no changing of guard.
    let mut fresh = SimState::new_campaign(&data, "preservers", 7, &picks);
    let founding = fresh.dominant_faction_id().map(str::to_owned).unwrap();
    let founding_name = data.factions.get(&founding).unwrap().name.clone();
    fresh.announce_ruling_people(&data);
    assert_eq!(
        ruling_lines(&fresh, &founding_name),
        0,
        "the founding majority is the silent baseline"
    );
    assert_eq!(
        fresh.ruling_people_voice.as_deref(),
        Some(founding.as_str())
    );

    // A ship the Hearth clearly runs, its baseline recorded.
    let fs = |id: &str, members: u32| FactionState {
        faction_id: id.to_string(),
        members,
        status: FactionStatus::Aboard,
        approval: 0.5,
        mood_band: 0,
    };
    let mut sim = SimState::new_campaign(&data, "preservers", 31, &picks);
    sim.factions = vec![fs("hearth_union", 500), fs("steel_covenant", 300)];
    sim.ruling_people_voice = Some("hearth_union".to_string());
    let steel_name = data.factions.get("steel_covenant").unwrap().name.clone();

    // The Hearth still runs the ship: no remark.
    sim.announce_ruling_people(&data);
    assert_eq!(
        ruling_lines(&sim, &steel_name),
        0,
        "no shift in the majority, no remark"
    );

    // Demographic drift hands the ship to the Steel Covenant: the guard changes, once, by name.
    sim.factions = vec![fs("hearth_union", 300), fs("steel_covenant", 600)];
    sim.announce_ruling_people(&data);
    assert_eq!(
        ruling_lines(&sim, &steel_name),
        1,
        "the changing of the guard is remarked once, naming the new people"
    );
    assert_eq!(sim.ruling_people_voice.as_deref(), Some("steel_covenant"));

    // Still the Steel Covenant: no reprint.
    sim.announce_ruling_people(&data);
    assert_eq!(
        ruling_lines(&sim, &steel_name),
        1,
        "staying under the same people is not re-announced"
    );
}
#[test]
fn the_ship_remarks_when_its_name_begins_to_mean_something() {
    // Content-depth voice round 16: the reputation voice, the quiet companion to
    // the it109 reputation beat at a gentler threshold. Crossing into a merciful
    // name surfaces one pooled line; a return to the middle re-arms; a feared
    // name gets its own, opposite line.
    let (data, mut sim, _picks) = armed(29);
    let trait_id = data.config.campaign_skeleton.reputation_beat_trait.clone();
    let high = data.config.flavor.reputation_voice_high;
    let low = data.config.flavor.reputation_voice_low;
    assert!(
        !trait_id.is_empty() && high > 0.0 && data.config.flavor.reputation_merciful.len() >= 3,
        "this test needs the reputation voice enabled"
    );
    let name_lines = |sim: &SimState| {
        let m = &data.config.flavor.reputation_merciful;
        let f = &data.config.flavor.reputation_feared;
        sim.log
            .iter()
            .filter(|l| m.contains(&l.text) || f.contains(&l.text))
            .count()
    };

    // A ship of neutral repute says nothing.
    sim.announce_reputation_name(&data);
    assert_eq!(name_lines(&sim), 0, "an unknown ship remarks nothing");

    // A name for mercy: one line.
    sim.reputation.insert(trait_id.clone(), high + 0.05);
    sim.announce_reputation_name(&data);
    assert_eq!(name_lines(&sim), 1, "a growing merciful name says so once");
    assert_eq!(sim.reputation_voice_band, 1);

    // Still merciful — no reprint.
    sim.announce_reputation_name(&data);
    assert_eq!(name_lines(&sim), 1, "staying merciful is not re-announced");

    // A name for fear: a second, distinct line.
    sim.reputation.insert(trait_id.clone(), low - 0.05);
    sim.announce_reputation_name(&data);
    assert_eq!(name_lines(&sim), 2, "a feared name says so afresh");
    assert_eq!(sim.reputation_voice_band, -1);
}
#[test]
fn the_ship_remarks_when_it_becomes_known_for_wonder_or_incuriosity() {
    // Content-depth voice round 28: the wonder reputation voice, the companion to the mercy
    // voice on the it28 `wonder` trait. A ship of neutral repute is silent; a name for
    // marvels surfaces one line; a return to the middle re-arms; an incurious name gets its
    // own, opposite line.
    let (data, mut sim, _picks) = armed(28);
    let fl = &data.config.flavor;
    let high = fl.wonder_voice_high;
    let low = fl.wonder_voice_low;
    assert!(
        high > 0.0 && fl.wonder_famed.len() >= 3,
        "this test needs the wonder voice enabled"
    );
    let wonder_lines = |sim: &SimState| {
        let famed = &data.config.flavor.wonder_famed;
        let incurious = &data.config.flavor.wonder_incurious;
        sim.log
            .iter()
            .filter(|l| famed.contains(&l.text) || incurious.contains(&l.text))
            .count()
    };

    // A ship of neutral repute says nothing.
    sim.announce_wonder_name(&data);
    assert_eq!(
        wonder_lines(&sim),
        0,
        "an unremarkable ship remarks nothing"
    );

    // A name for marvels: one line.
    sim.reputation.insert("wonder".to_string(), high + 0.05);
    sim.announce_wonder_name(&data);
    assert_eq!(
        wonder_lines(&sim),
        1,
        "a growing name for wonder says so once"
    );
    assert_eq!(sim.wonder_voice_band, 1);

    // Still famed — no reprint.
    sim.announce_wonder_name(&data);
    assert_eq!(wonder_lines(&sim), 1, "staying famed is not re-announced");

    // A name for incuriosity: a second, distinct line.
    sim.reputation.insert("wonder".to_string(), low - 0.05);
    sim.announce_wonder_name(&data);
    assert_eq!(wonder_lines(&sim), 2, "an incurious name says so afresh");
    assert_eq!(sim.wonder_voice_band, -1);
}
#[test]
fn the_ship_remarks_when_it_becomes_known_for_resolve_or_folding() {
    // Content-depth voice round 29: the resolve reputation voice, completing the
    // mercy/wonder/resolve set. A ship of neutral repute is silent; a name for steadfastness
    // surfaces one line; a return to the middle re-arms; a name for folding gets its own line.
    let (data, mut sim, _picks) = armed(31);
    let fl = &data.config.flavor;
    let high = fl.resolve_voice_high;
    let low = fl.resolve_voice_low;
    assert!(
        high > 0.0 && fl.resolve_steadfast.len() >= 3,
        "this test needs the resolve voice enabled"
    );
    let resolve_lines = |sim: &SimState| {
        let steadfast = &data.config.flavor.resolve_steadfast;
        let yielding = &data.config.flavor.resolve_yielding;
        sim.log
            .iter()
            .filter(|l| steadfast.contains(&l.text) || yielding.contains(&l.text))
            .count()
    };

    // A ship of neutral repute says nothing.
    sim.announce_resolve_name(&data);
    assert_eq!(
        resolve_lines(&sim),
        0,
        "an unremarkable ship remarks nothing"
    );

    // A name for steadfastness: one line.
    sim.reputation.insert("resolve".to_string(), high + 0.05);
    sim.announce_resolve_name(&data);
    assert_eq!(
        resolve_lines(&sim),
        1,
        "a growing name for resolve says so once"
    );
    assert_eq!(sim.resolve_voice_band, 1);

    // Still steadfast — no reprint.
    sim.announce_resolve_name(&data);
    assert_eq!(
        resolve_lines(&sim),
        1,
        "staying steadfast is not re-announced"
    );

    // A name for folding: a second, distinct line.
    sim.reputation.insert("resolve".to_string(), low - 0.05);
    sim.announce_resolve_name(&data);
    assert_eq!(resolve_lines(&sim), 2, "a name for folding says so afresh");
    assert_eq!(sim.resolve_voice_band, -1);
}
#[test]
fn the_ship_remarks_when_its_government_slips_or_steadies() {
    // Content-depth voice round 17: the governance voice, the institutional twin of
    // the morale and polity voices. A founding ship's sound government is the silent
    // baseline; crossing into a fraying band surfaces one pooled line; a return to a
    // firm band gets its own, opposite line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(31);
    let fl = &data.config.flavor;
    assert!(
        fl.stability_voice_high > 0.0 && fl.stability_fraying.len() >= 3,
        "this test needs the governance voice enabled"
    );
    let low = fl.stability_voice_low;
    let high = fl.stability_voice_high;
    let gov_lines = |sim: &SimState| {
        let fray = &data.config.flavor.stability_fraying;
        let firm = &data.config.flavor.stability_firming;
        sim.log
            .iter()
            .filter(|l| fray.contains(&l.text) || firm.contains(&l.text))
            .count()
    };

    // A founding ship's institutions are sound — the launch band is recorded, silent.
    sim.announce_stability_mood(&data);
    assert_eq!(gov_lines(&sim), 0, "a sound founding government is silent");

    // The institutions fray past the gentle line: one line.
    sim.population.stability = low - 0.05;
    sim.announce_stability_mood(&data);
    assert_eq!(gov_lines(&sim), 1, "a government slipping says so once");
    assert_eq!(sim.stability_voice_band, -1);

    // Still fraying — no reprint.
    sim.announce_stability_mood(&data);
    assert_eq!(gov_lines(&sim), 1, "staying frayed is not re-announced");

    // The institutions firm up again: a second, distinct line.
    sim.population.stability = high + 0.05;
    sim.announce_stability_mood(&data);
    assert_eq!(
        gov_lines(&sim),
        2,
        "the government steadying says so afresh"
    );
    assert_eq!(sim.stability_voice_band, 1);
}
#[test]
fn the_ship_remarks_when_the_crew_frays_or_pulls_together() {
    // Content-depth voice round 21: the unity (cohesion) voice, the fourth
    // internal-state voice. A founding crew's one-people unity is the silent
    // baseline; crossing into a fraying band surfaces one pooled line; a return to a
    // cohering band gets its own, opposite line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(43);
    let fl = &data.config.flavor;
    assert!(
        fl.unity_voice_high > 0.0 && fl.unity_fraying.len() >= 3,
        "this test needs the unity voice enabled"
    );
    let low = fl.unity_voice_low;
    let high = fl.unity_voice_high;
    let unity_lines = |sim: &SimState| {
        let fray = &data.config.flavor.unity_fraying;
        let cohere = &data.config.flavor.unity_cohering;
        sim.log
            .iter()
            .filter(|l| fray.contains(&l.text) || cohere.contains(&l.text))
            .count()
    };

    // A founding crew is one people — the launch band is recorded, silent.
    sim.announce_unity_mood(&data);
    assert_eq!(unity_lines(&sim), 0, "a founding crew's unity is silent");

    // The crew frays past the low line: one line.
    sim.population.unity = low - 0.05;
    sim.announce_unity_mood(&data);
    assert_eq!(unity_lines(&sim), 1, "a crew splintering says so once");
    assert_eq!(sim.unity_voice_band, -1);

    // Still fraying — no reprint.
    sim.announce_unity_mood(&data);
    assert_eq!(unity_lines(&sim), 1, "staying frayed is not re-announced");

    // The crew pulls back together: a second, distinct line.
    sim.population.unity = high + 0.05;
    sim.announce_unity_mood(&data);
    assert_eq!(unity_lines(&sim), 2, "the crew cohering says so afresh");
    assert_eq!(sim.unity_voice_band, 1);
}
#[test]
fn the_ship_remarks_when_the_founders_fire_gutters_or_flares() {
    // Content-depth voice round 20: the loyalty voice, the identity-side twin of
    // the morale and governance voices. A founding crew's moderate loyalty is the
    // silent baseline; crossing into a guttering band (the founders' purpose fading)
    // surfaces one pooled line; a return to a bright band gets its own, opposite
    // line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(37);
    let fl = &data.config.flavor;
    assert!(
        fl.loyalty_voice_high > 0.0 && fl.loyalty_guttering.len() >= 3,
        "this test needs the loyalty voice enabled"
    );
    let low = fl.loyalty_voice_low;
    let high = fl.loyalty_voice_high;
    let loyalty_lines = |sim: &SimState| {
        let gut = &data.config.flavor.loyalty_guttering;
        let bright = &data.config.flavor.loyalty_bright;
        sim.log
            .iter()
            .filter(|l| gut.contains(&l.text) || bright.contains(&l.text))
            .count()
    };

    // A founding crew's moderate devotion — the launch band is recorded, silent.
    sim.announce_loyalty_mood(&data);
    assert_eq!(
        loyalty_lines(&sim),
        0,
        "a founding crew's loyalty is silent"
    );

    // The founders' fire gutters past the low line: one line.
    sim.population.legacy_loyalty = low - 0.05;
    sim.announce_loyalty_mood(&data);
    assert_eq!(loyalty_lines(&sim), 1, "the mission fading says so once");
    assert_eq!(sim.loyalty_voice_band, -1);

    // Still guttering — no reprint.
    sim.announce_loyalty_mood(&data);
    assert_eq!(loyalty_lines(&sim), 1, "staying faded is not re-announced");

    // The dream flares bright again: a second, distinct line.
    sim.population.legacy_loyalty = high + 0.05;
    sim.announce_loyalty_mood(&data);
    assert_eq!(
        loyalty_lines(&sim),
        2,
        "the founders' fire rekindled says so afresh"
    );
    assert_eq!(sim.loyalty_voice_band, 1);
}
#[test]
fn the_ship_remarks_when_the_crew_turns_shipborn_or_holds_baseline() {
    // Content-depth voice round 25: the adaptation voice, the physiological companion
    // to the loyalty voice. A founding crew's baseline body is the silent baseline;
    // crossing into a shipborn band surfaces one pooled line; holding to baseline gets
    // its own, opposite line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(53);
    let fl = &data.config.flavor;
    assert!(
        fl.adaptation_voice_high > 0.0 && fl.crew_shipborn.len() >= 3,
        "this test needs the adaptation voice enabled"
    );
    let low = fl.adaptation_voice_low;
    let high = fl.adaptation_voice_high;
    let body_lines = |sim: &SimState| {
        let ship = &data.config.flavor.crew_shipborn;
        let base = &data.config.flavor.crew_baseline;
        sim.log
            .iter()
            .filter(|l| ship.contains(&l.text) || base.contains(&l.text))
            .count()
    };

    // A founding crew is baseline-human — the launch band is recorded, silent.
    sim.announce_adaptation_mood(&data);
    assert_eq!(body_lines(&sim), 0, "a founding crew's bodies are silent");

    // The descendants cross into shipborn: one line.
    sim.population.adaptation = high + 0.05;
    sim.announce_adaptation_mood(&data);
    assert_eq!(body_lines(&sim), 1, "a shipborn crew says so once");
    assert_eq!(sim.adaptation_voice_band, 1);

    // Still shipborn — no reprint.
    sim.announce_adaptation_mood(&data);
    assert_eq!(body_lines(&sim), 1, "staying shipborn is not re-announced");

    // Held back to baseline: a second, distinct line.
    sim.population.adaptation = low - 0.05;
    sim.announce_adaptation_mood(&data);
    assert_eq!(
        body_lines(&sim),
        2,
        "a crew held to baseline says so afresh"
    );
    assert_eq!(sim.adaptation_voice_band, -1);
}
#[test]
fn the_ship_remarks_when_the_crew_becomes_a_new_people_or_keeps_the_old_ways() {
    // Content-depth voice round 26: the cultural-drift voice, the cultural companion to
    // the adaptation voice. A founding crew keeps the founders' ways (the silent launch
    // baseline, band -1 since drift launches below the low line); drifting into a
    // new-people band surfaces one pooled line; keeping the old ways afresh after a drift
    // gets its own, opposite line; staying put does not reprint.
    let (data, mut sim, _picks) = armed(57);
    let fl = &data.config.flavor;
    assert!(
        fl.drift_voice_high > 0.0 && fl.culture_newfound.len() >= 3,
        "this test needs the drift voice enabled"
    );
    let low = fl.drift_voice_low;
    let high = fl.drift_voice_high;
    let culture_lines = |sim: &SimState| {
        let new = &data.config.flavor.culture_newfound;
        let kept = &data.config.flavor.culture_founders_kept;
        sim.log
            .iter()
            .filter(|l| new.contains(&l.text) || kept.contains(&l.text))
            .count()
    };

    // A founding crew keeps the founders' ways — the launch band (-1) is recorded, silent.
    sim.announce_drift_mood(&data);
    assert_eq!(
        culture_lines(&sim),
        0,
        "a founding crew's culture is silent"
    );
    assert_eq!(sim.drift_voice_band, -1);

    // The descendants drift into a new people: one line.
    sim.population.cultural_drift = high + 0.05;
    sim.announce_drift_mood(&data);
    assert_eq!(culture_lines(&sim), 1, "a new people says so once");
    assert_eq!(sim.drift_voice_band, 1);

    // Still a new people — no reprint.
    sim.announce_drift_mood(&data);
    assert_eq!(
        culture_lines(&sim),
        1,
        "staying a new people is not re-announced"
    );

    // Held back to the founders' ways: a second, distinct line.
    sim.population.cultural_drift = low - 0.05;
    sim.announce_drift_mood(&data);
    assert_eq!(
        culture_lines(&sim),
        2,
        "a crew that keeps the old ways afresh says so"
    );
    assert_eq!(sim.drift_voice_band, -1);
}
