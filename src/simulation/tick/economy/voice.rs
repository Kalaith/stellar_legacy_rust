//! The year's crossings are given a voice, and the drift given its due.

use crate::data::GameData;
use crate::state::sim::SimState;

use super::super::TickReport;
use super::factors::apply_voyage_drift;

/// Subsystem decay, the peoples who tend them, voyage drift, and every band
/// narrator that speaks once when a meter turns.
pub(super) fn decay_modules_and_speak(
    sim: &mut SimState,
    data: &GameData,
    _report: &mut TickReport,
) {
    // …and the people whose craft is a module notice when it is left to rot
    // (content-depth subsystems round 8): sustained neglect of a faction's
    // tended subsystem erodes its approval, feeding the round-8 withdrawal.
    sim.apply_subsystem_neglect_sentiment(data);
    // …and its bright mirror on the approval side (content-depth factions round 29): a people
    // whose module the ship keeps *excellent* is pleased to see its craft honored, gaining a
    // little approval — the condition→approval *up* direction the it neglect penalty (which only
    // ran condition→approval *down*) never drew, completing the two-sided coupling.
    sim.apply_honored_tender_sentiment(data);
    // …and its bright mirror on the condition side (content-depth factions round 22): a people
    // *delighted* with its lot tends its module with pride, keeping it a shade sharper than duty
    // alone would — so a kept module keeps its people content (it29 above) and content people
    // keep the module kept (this), a virtuous circle across the faction↔subsystem boundary.
    sim.apply_proud_tender_upkeep(data);

    // …and give the approval meter a voice (content-depth voice round 8): a people
    // crossing into restlessness or contentment says so in the log, once, so the
    // player feels the mood turn well before a withdrawal beat fires.
    sim.announce_faction_moods(data);
    // …and give the ship's whole political climate a voice (content-depth voice
    // round 15): when the aggregate mood of the peoples crosses into broad
    // discontent or broad ease, the polity as a whole says so once — the ship-level
    // companion to the per-faction and morale voices.
    sim.announce_polity_mood(data);
    // …and the standing character of whoever runs the ship bends its reputation over
    // the generations (content-depth factions round 16): a kind majority drifts the
    // ship toward a merciful name, a cold one hardens it, no dramatic choice required.
    sim.apply_dominant_reputation_lean(data);
    // …and the name the ship has earned warms or cools each people toward it (content-depth
    // factions round 27): the reverse of the lean above, closing the reputation_leanings loop —
    // a merciful ship contents the peoples who prize mercy and sours those who scorn it, so the
    // ship's character is no longer only *shaped by* its factions but *felt by* them.
    sim.apply_reputation_alignment_sentiment(data);
    // …and the ship remarks when its name begins to mean something (content-depth
    // voice round 16): a merciful or a feared reputation says so once, at a gentler
    // threshold than the it109 beat — the quiet marker before the defining reckoning.
    sim.announce_reputation_name(data);
    // …and its *other* name too (content-depth voice round 28): the mercy voice reads only the
    // one watched trait, but a ship earns a whole character — when the it28 `wonder` trait crosses
    // into famed-for-marvels or incurious, the decks remark that too, the companion to the mercy
    // voice on the trait this session made load-bearing.
    sim.announce_wonder_name(data);
    // …and its *third* name (content-depth voice round 29): completing the mercy/wonder/resolve
    // voice set — when the `resolve` trait crosses into steadfast or yielding, the decks remark
    // the ship's growing name for seeing the hard thing through, or for folding.
    sim.announce_resolve_name(data);

    // Voyage drift (PLAN M4.1): a long voyage changes the people, not just the
    // ship — adaptation and cultural drift rise, loyalty to the founders fades,
    // and the strain wears at morale and unity. Deterministic; the founders'
    // hopeful crew slowly becomes someone else the longer they fly.
    apply_voyage_drift(sim, data);

    // …and give the ship's *collective* morale a voice (content-depth voice round
    // 11), now that the year's habitat lift and voyage strain have both settled:
    // when the whole crew's spirits cross into a grim or a buoyant band, the decks
    // say so once — the ship-wide twin of the faction-mood announcement above.
    sim.announce_ship_mood(data);
    // …and give the ship's *institutions* a voice (content-depth voice round 17), now
    // that the year's security-driven recovery and any event shifts have settled: when
    // stability crosses into a fraying or a firm band the decks remark the government
    // slipping or working — the governance twin of the spirits and political-climate
    // voices above.
    sim.announce_stability_mood(data);
    // …and give the crew's *devotion to the founders' mission* a voice (content-depth
    // voice round 20), now that the year's drift has eroded loyalty and any event
    // shifts have settled: when loyalty crosses into a guttering band (the founders'
    // purpose fading to a story) or a bright one (the dream taken up afresh), the decks
    // remark it once — the identity-side twin of the spirits and governance voices, and
    // the voice that gives the game's core theme (a ship forgetting why it flies) a line.
    sim.announce_loyalty_mood(data);
    // …and give the crew's *bodies* a voice too (content-depth voice round 25), now that
    // the year's drift (and the it25 medical resistance to it) has settled on adaptation:
    // when the descendants cross into a shipborn body or hold to the founders' baseline,
    // the decks remark the crew becoming, or refusing to become, a new kind of people —
    // the physiological companion to the loyalty (their belief) voice above.
    sim.announce_adaptation_mood(data);
    // …and give the crew's *culture* a voice too (content-depth voice round 26), now that the
    // year's drift (and the it10 archive resistance to it) has settled on cultural_drift: when
    // the crew's customs cross into a people the founders would not recognise, or hold the old
    // ways close, the decks remark it once — the cultural companion to the adaptation (their
    // bodies) voice just above, the two identity-drift voices side by side.
    sim.announce_drift_mood(data);
    // …and give the crew's *cohesion* a voice (content-depth voice round 21), now that
    // the year's faction-mood coupling (it100), security recovery, and voyage strain
    // have all settled on unity: when the crew crosses into fraying into cliques or
    // pulling back together as one, the decks remark it once — the internal-state voice
    // beside the spirits, the governance, and the founders' fire.
    sim.announce_unity_mood(data);
    // …and the ship's *own body* has a voice too (content-depth voice round 22), now that
    // the year's hull wear (and any refit) has settled: when the hull crosses into a
    // groaning band or back into a sound one, the decks remark the vessel itself aging or
    // renewed — the first voice for the machine rather than the crew it carries.
    sim.announce_hull_condition(data);
    // …and the ship's *air* is the other half of its body (content-depth voice round 23),
    // now that the year's life-support wear (and any repair) has settled: when the air
    // crosses into a stale band or back into a fresh one, the decks remark the atmosphere
    // going close or clearing — the structure and the air, the two survival systems, both
    // now speak.
    sim.announce_air_condition(data);
    // …and the ship's *drive* is the third of its survival systems (content-depth voice round
    // 27), now that the year's burn and any scooping have settled on the tanks: when the fuel
    // crosses into a thin band (running on fumes, husbanding every gram) or back into a full
    // one (tanks deep, the drive free to burn), the decks remark it once — completing the
    // ship-body voice trio (structure, air, and now motion) to match the it23/it24/it25
    // hull/air/becalmed crisis-beat trio: the drive murmurs as it thins, the becalmed beat
    // reckons when the ship is truly stranded.
    sim.announce_drive_condition(data);
}
