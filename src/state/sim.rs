//! The full serializable simulation state for one campaign.
//!
//! UI panels read this via `&SimState` and never mutate it directly — all
//! mutation happens through `UiAction` dispatch in `game.rs` and the
//! stateless services in `simulation/` (CODE_STANDARDS §7).

use crate::data::ProductionRates;
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod campaign;
pub mod contract;
pub mod debrief;
pub mod dynasty;
pub mod factions;
pub mod market;
pub mod pools;
pub mod session;
pub mod subsystems;

pub use campaign::*;
pub use contract::{ActiveContract, CampaignBeat, MetricState, MilestoneState};
pub use dynasty::*;
pub use market::*;
pub use pools::*;
pub use session::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimState {
    pub seed: u64,
    pub rng: SeededRng,
    /// Months since founding, starting at 0 (W3). Display year/month derive
    /// from it via `year()` / `month()`; the economic tick still applies on
    /// year boundaries.
    pub month_clock: u32,
    /// Month-clock reading when the last event fired, for the event-chance
    /// ramp (GDD §5.4, now month-resolution).
    pub last_event_month_clock: u32,
    /// Real-time auto-advance rate while under way (real-time loop). Read by the
    /// game-loop driver + the dashboard selector; never touched by the tick.
    #[serde(default)]
    pub speed: GameSpeed,
    pub resources: ResourcePool,
    pub production: ProductionRates,
    pub ship: ShipState,
    pub population: PopulationState,
    pub dynasty: Dynasty,
    #[serde(default)]
    pub crew: Vec<CrewMember>,
    #[serde(default)]
    pub next_crew_id: u32,
    pub legacy: LegacyTrack,
    pub contract: Option<ActiveContract>,
    /// A charter under consideration in port before launch (W4). Cleared when
    /// the mission launches; only ever set while `contract.is_none()`.
    #[serde(default)]
    pub selected_charter: Option<String>,
    /// Total Travel months spent coasting on a dry tank (W4) — calendar time
    /// that bought no progress toward the destination.
    #[serde(default)]
    pub stalled_months: u32,
    /// Set the moment a Travel month stalls for want of fuel; read (and reset)
    /// at the year boundary to double that year's systems decay (W4).
    #[serde(default)]
    pub fuel_stalled_this_year: bool,
    /// Consecutive years the ship has been *becalmed* — a Travel leg stalled dry for
    /// want of fuel (content-depth campaign-skeleton round 25): a rolling count of how
    /// long the ship has been unable to make its heading, reset the moment it burns
    /// again. It's what lets the skeleton tell a bad month coasting from a *stranding*,
    /// and drives the round-25 becalmed beat. 0 at launch.
    #[serde(default)]
    pub fuel_stall_years: u32,
    /// The band the becalmed beat last marked (content-depth campaign-skeleton round 25):
    /// -1 once the ship has been stranded long enough to force the reckoning, 0 while it
    /// still moves. The mobility twin of the it hull/air collapse beats; a return to
    /// burning re-arms it. 0 at launch.
    #[serde(default)]
    pub becalmed_beat_band: i8,
    /// The band the adaptation-divergence beat last marked (content-depth campaign-skeleton
    /// round 26): 1 once the crew has grown so shipborn it can no longer survive a planet,
    /// 0 while it is still planet-capable. The crew-body twin of the it hull/air/becalmed
    /// ship-body crisis beats; a fall back below the red line (a strong infirmary holding the
    /// baseline) re-arms it. 0 at launch — a founding crew is planet-born.
    #[serde(default)]
    pub adaptation_divergence_band: i8,
    /// The band the cultural-divergence beat last marked (content-depth campaign-skeleton
    /// round 27): 1 once the crew's culture has drifted so far the founders' charter is a dead
    /// language, 0 while the founding purpose is still intelligible. The cultural twin of the
    /// it26 adaptation-divergence band (their bodies); a fall back below the red line (a strong
    /// archive reviving the old ways) re-arms it. 0 at launch — a founding crew keeps the
    /// founders' meanings.
    #[serde(default)]
    pub cultural_divergence_band: i8,
    /// Fuel actually scooped by the drive since the last provisioning report
    /// (real-time loop follow-up: legible stat changes), 0-1 fraction. Accrued
    /// each year by the engine regen (only the part that wasn't capped away), so a
    /// periodic in-world line can tell the player where their fuel came from. Reset
    /// when that line fires.
    #[serde(default)]
    pub fuel_scooped_accum: f32,
    /// Set when the player dismisses the first-voyage checklist; it also stops
    /// showing once the Chronicle records a completed mission.
    #[serde(default)]
    pub tutorial_dismissed: bool,
    pub market: MarketState,
    pub delegation: DelegationSettings,
    pub pending_event: Option<PendingEvent>,
    #[serde(default)]
    pub pending_dilemma: Option<PendingDilemma>,
    /// Accumulated named consequences from past outcomes (Pillar 2). Read by
    /// future event weighting; append-only from outcome application.
    pub consequences: Vec<String>,
    /// Graded reputation traits (content-depth event families round 16): the ship's
    /// cumulative *character*, where `consequences` records discrete deeds. A named
    /// 0-1 scalar (0.5 neutral) nudged a little by many separate outcomes, so a
    /// tendency — mercy, ruthlessness — builds across a campaign and later events can
    /// read who the ship has *become*. Unset traits read neutral via `reputation`.
    #[serde(default)]
    pub reputation: HashMap<String, f32>,
    /// Follow-ups promised to fire at a *determined* year (content-depth event
    /// families round 9): an outcome can schedule a specific event to re-fire in
    /// N years, so an authored arc pays off on a clock rather than waiting for the
    /// RNG to surface it. Deterministic; fired and removed by `fire_scheduled_beat`.
    #[serde(default)]
    pub scheduled_events: Vec<ScheduledEvent>,
    /// How many times each event template has fired this campaign (content-depth
    /// event families round 11): lets a recurring crisis *escalate* instead of
    /// merely repeating — a complication can gate on prior occurrences, so the
    /// third outbreak of the same plague reads as the ship's patience wearing
    /// through. Incremented as each event resolves.
    #[serde(default)]
    pub event_fire_counts: HashMap<String, u32>,
    /// The dominant faction last marked by the skeleton (content-depth campaign
    /// skeleton round 11): so that when demographic drift or a schism flips *which
    /// people runs the ship*, a power-transition beat can fire on the change. Empty
    /// until the first tick records the launch majority (no spurious beat at start).
    #[serde(default)]
    pub last_dominant_faction: String,
    /// The morale band the ship's collective mood last announced (content-depth
    /// voice round 11): so a crossing *into* grim or buoyant surfaces one ambient
    /// line — the ship-wide parallel to a faction's `mood_band`. 0 (steady) at
    /// launch; settling back to steady is silent but remembered.
    #[serde(default)]
    pub morale_band: i8,
    /// The political-climate band the ship last announced (content-depth voice round
    /// 15): the aggregate faction mood (`aboard_approval_mean`) as a band, so a ship
    /// crossing *into* broad discontent or broad ease says so once. 0 (neutral) at
    /// launch; a return to neutral is silent but remembered.
    #[serde(default)]
    pub polity_mood_band: i8,
    /// The reputation band the skeleton last marked with a beat (content-depth
    /// campaign-skeleton round 16): so the ship reckons with its name once when it
    /// crosses *into* a strong reputation, not every year it holds one. 0 (neutral)
    /// at launch; a return to the middle silently re-arms it.
    #[serde(default)]
    pub reputation_beat_band: i8,
    /// The reputation band the ship's *voice* last announced (content-depth voice
    /// round 16): a gentler crossing than the beat's — the ship remarking it is
    /// becoming known for a trait, once, before that name grows defining. 0 at
    /// launch; a return to the middle silently re-arms.
    #[serde(default)]
    pub reputation_voice_band: i8,
    /// The band the ship's *wonder* reputation voice last announced (content-depth voice round
    /// 28): the it16 reputation voice reads only the watched mercy trait, so the it28 `wonder`
    /// trait (a name earned by chasing marvels) got its own. Tracks whether wonder last crossed
    /// into a famed band (a chronicle of charted impossibilities) or an incurious one (a ship
    /// that sails past every strangeness), so the decks remark it once. 0 at launch (a neutral
    /// name); a return to the middle re-arms.
    #[serde(default)]
    pub wonder_voice_band: i8,
    /// The band the ship's *resolve* reputation voice last announced (content-depth voice round
    /// 29): the third built-trait voice, completing the mercy/wonder/resolve set. Tracks whether
    /// resolve last crossed into a steadfast band (a hull known to see the grim thing through) or
    /// a yielding one (a name for folding, for the writ quit half-done), so the decks remark it
    /// once. 0 at launch (a neutral name); a return to the middle re-arms.
    #[serde(default)]
    pub resolve_voice_band: i8,
    /// The last-announced band of the ship's *institutional* order (content-depth
    /// voice round 17): the governance twin of `morale_band`. Tracks whether stability
    /// last crossed into a firm or a fraying band so a government quietly working, or
    /// quietly slipping, says so once rather than every year it holds. The launch band
    /// (a founding ship's institutions are sound) is recorded, not announced.
    #[serde(default)]
    pub stability_voice_band: i8,
    /// The last-announced band of the crew's *devotion to the founders' mission*
    /// (content-depth voice round 20): the twin of `morale_band`/`stability_voice_band`
    /// for `legacy_loyalty`. Tracks whether loyalty last crossed into a bright band (the
    /// founders' dream burning fierce) or a guttering one (the mission fading to a story
    /// the young no longer feel) so the ship remarks the crossing once, not every year it
    /// holds. The launch band (a founding crew's loyalty runs high) is recorded, not
    /// announced. 0 at launch; a return to the middle silently re-arms.
    #[serde(default)]
    pub loyalty_voice_band: i8,
    /// The last-announced band of the crew's *physiological* identity (content-depth
    /// voice round 25): the bodily companion to the it167 loyalty voice (their belief in
    /// the founders' cause) — this reads `adaptation`, how far the descendants' *bodies*
    /// have drifted from the baseline-human stock the founders launched. Tracks whether
    /// adaptation last crossed into a shipborn band (a crew longer, leaner, ill-suited to
    /// a world) or a baseline one (held human by a well-kept infirmary, it25) so the ship
    /// remarks the crossing once. The launch band (a founding crew is baseline-human) is
    /// recorded, not announced. 0 at launch; a return to the middle re-arms.
    #[serde(default)]
    pub adaptation_voice_band: i8,
    /// The last-announced band of the crew's *cultural* identity (content-depth voice round
    /// 26): the cultural companion to the `adaptation_voice_band` (their bodies) — this reads
    /// `cultural_drift`, how far the crew's customs, calendars, and tongue have drifted from
    /// the founders'. Tracks whether drift last crossed into a new-people band (a culture the
    /// founders would not recognise) or a founders-kept one (the old ways held close, the
    /// rarer crossing a strong archive earns) so the ship remarks the crossing once. The
    /// launch band (a founding crew keeps the founders' ways) is recorded, not announced.
    /// 0 at launch; a return to the middle re-arms.
    #[serde(default)]
    pub drift_voice_band: i8,
    /// The last-announced band of the crew's *cohesion* (content-depth voice round 21):
    /// the fourth internal-state voice beside morale (`morale_band`), governance
    /// (`stability_voice_band`), and mission-devotion (`loyalty_voice_band`), on the
    /// `unity` stat. Tracks whether cohesion last crossed into a fraying band (the crew
    /// splintering into wary cliques, one people becoming several) or a cohering one
    /// (the ship pulling together as one crew again) so the decks remark the crossing
    /// once, not every year it holds. The launch band (a founding crew is one people) is
    /// recorded, not announced. 0 at launch; a return to the middle re-arms.
    #[serde(default)]
    pub unity_voice_band: i8,
    /// The last-announced band of the ship's *own body* — its hull (content-depth voice
    /// round 22): the first voice for the vessel itself rather than the crew within it.
    /// Where the morale/unity/stability/loyalty voices read the *people*, this reads the
    /// aging machine that carries them — whether `hull_integrity` last crossed into a
    /// groaning band (the old hull weeping at the seams, patched and complaining) or a
    /// sound one (riding tight and true again after a refit), so the decks remark the
    /// crossing once. The launch band (a new-built hull is sound) is recorded, not
    /// announced. 0 at launch; a return to the middle re-arms.
    #[serde(default)]
    pub hull_voice_band: i8,
    /// The last-announced band of the ship's *air* — its life-support (content-depth
    /// voice round 23): the second ship-body voice, the atmosphere twin of the it22 hull
    /// (structure) voice. Tracks whether `life_support` last crossed into a stale band
    /// (the air gone close and thick, scrubbers labouring, a headache on every deck) or a
    /// fresh one (clean and cool again after an overhaul), so the decks remark the
    /// crossing once. The launch band (a new ship breathes clean) is recorded, not
    /// announced. 0 at launch; a return to the middle re-arms.
    #[serde(default)]
    pub air_voice_band: i8,
    /// The last-announced band of the ship's *drive* — its fuel (content-depth voice round
    /// 27): the third ship-body voice, the motion twin of the it22 hull (structure) and it23
    /// air (atmosphere) voices. Tracks whether `ship.fuel` last crossed into a thin band (tanks
    /// low, running on fumes, the drive lit only when it must be) or a full one (deep tanks and
    /// a free hand on the throttle after a scoop or resupply), so the decks remark the crossing
    /// once. The launch band (a new ship sets out with full tanks) is recorded, not announced.
    /// 0 at launch; a return to the middle re-arms.
    #[serde(default)]
    pub fuel_voice_band: i8,
    /// The last-announced band of the ship's *headcount* (content-depth voice round 30): the one
    /// core dimension with a beat (the it12 depopulation beat) and an ambient (the hollow pool)
    /// but no crossing-voice — and whose *growth* side no narration touched at all. Tracks whether
    /// the crew last crossed into a swelling band (the cradles full, new decks opened, a people
    /// expanding) or a thinning one (corridors gone quiet, whole decks closed, a shrinking
    /// people), read against `starting_population`, so the decks remark the crossing once. The
    /// launch band (a ship at its founding complement) is 0, recorded not announced; a return to
    /// the middle re-arms.
    #[serde(default)]
    pub crew_size_voice_band: i8,
    /// The last-announced band of the ship's *treasury* (content-depth voice round 32): the
    /// material-fortune voice, read against `starting_resources.credits` the way the it30 crew-size
    /// band is read against `starting_population`. Tracks whether the coffers last crossed into a
    /// flush band (well-paid charters filling the accounts, the council debating what to build) or a
    /// bare one (every credit counted twice, requisitions stalled), so the ledger's turning is
    /// remarked once. The launch band (a ship at its founding stake, ratio 1.0) is 0, not announced;
    /// a return to the middle re-arms.
    #[serde(default)]
    pub treasury_voice_band: i8,
    /// The last-announced band of the ship's *power* — its energy store (content-depth voice round
    /// 33): the power-fortune voice, the sibling of the it32 treasury (money) band. Tracks whether
    /// the reactors last crossed into a flush band (energy past the surplus line, everything lit) or
    /// a dark one (the grid near the it15 life-support and it29 production lines, decks on rationed
    /// light), so the ship's power fortune is remarked once at each turning. The launch band (a ship
    /// at its founding stock, bracketed between the lines) is 0, not announced; a return to the
    /// middle re-arms.
    #[serde(default)]
    pub power_voice_band: i8,
    /// The id of the people last announced as *running the ship* (content-depth voice round 31):
    /// the first voice keyed not to a stat crossing a band but to a *change in which faction is
    /// dominant* — the largest aboard, "who runs the ship" for the it10 dilemma odds, the it16
    /// reputation lean, and the it21 ambient. Over centuries the it11/it13 demographic drift can
    /// hand the ship from one people to another, and the whole ship bends to the new majority's
    /// ways, but the turning itself went unremarked. Records the launch dominant people silently;
    /// when the dominant people later *changes*, the decks remark the changing of the guard once,
    /// then this updates. None = not yet recorded (pre-launch / a ship with no aboard people).
    #[serde(default)]
    pub ruling_people_voice: Option<String>,
    /// The band the skeleton's hull-collapse beat last marked (content-depth campaign-
    /// skeleton round 23): -1 once the hull has crossed *into* structural failure (the
    /// beat fires the moment it does), 0 while the hull holds above the red line. The
    /// beat is the reckoning the it22 hull *voice* only murmurs before; a refit back over
    /// the line re-arms it, so a ship rebuilt and let fail again reckons anew. 0 at
    /// launch (a new-built hull is sound).
    #[serde(default)]
    pub hull_beat_band: i8,
    /// The band the skeleton's air-collapse beat last marked (content-depth campaign-
    /// skeleton round 24): the atmosphere twin of `hull_beat_band` — -1 once life-support
    /// has crossed *into* failure (the beat fires the moment it does, the ship
    /// suffocating), 0 while the air holds above the red line. The reckoning the it23 air
    /// *voice* only murmurs before; an overhaul back over the line re-arms it. 0 at launch
    /// (a new ship breathes clean).
    #[serde(default)]
    pub air_beat_band: i8,
    /// How many depopulation thresholds the skeleton has already marked
    /// (content-depth campaign-skeleton round 12): the crew-thinning beat fires
    /// once per authored fraction of the founding size across the whole campaign
    /// (not per contract), so a recruited-up ship between voyages does not re-mark
    /// a stage it already passed. 0 at launch.
    #[serde(default)]
    pub depopulation_beats_fired: u32,
    /// Subsystem modules whose collapse beat the skeleton has already marked
    /// (content-depth campaign-skeleton round 17): each listed module's red-line beat
    /// fires once across the whole campaign, tracked by id, so a keystone repaired and
    /// let collapse again does not re-mark a reckoning already had. Empty at launch.
    #[serde(default)]
    pub subsystem_beats_fired: Vec<String>,
    /// Whether the campaign's single *founding-era* beat has fired (content-depth
    /// campaign-skeleton round 22): the early-voyage member of the era-beat trio (the
    /// mid-voyage beat it and the homecoming beat cover the other two). Forced once, the
    /// year the voyage passes `founding_beat_year` — the founding generation, the ones
    /// who chose to leave, having by then largely passed, and the ship handed for the
    /// first time wholly to those born to the void. Campaign-scoped (fires once ever, not
    /// once per voyage), so a back-to-back second charter does not re-mark it. False at
    /// launch.
    #[serde(default)]
    pub founding_beat_fired: bool,
    /// Consecutive years the food store has sat below the *lean* line (content-depth
    /// provisioning round 13): a rolling count of how long scarcity has ground on,
    /// reset the moment the larder recovers. It's what lets content tell a chronic
    /// hunger — a lean *generation* — apart from one hungry winter. 0 at launch.
    #[serde(default)]
    pub lean_food_years: u32,
    /// Consecutive years the food store has sat at or above the *fat* line
    /// (content-depth provisioning round 14): the mirror of `lean_food_years`, reset
    /// the moment plenty ends. It's what lets content tell a lifetime of plenty — a
    /// generation raised never knowing want — from one bumper year. 0 at launch.
    #[serde(default)]
    pub fat_food_years: u32,
    /// Consecutive years the ship has run without the spare-parts stock to keep its upkeep
    /// (content-depth provisioning round 27): a rolling count of how long the ship has gone
    /// unmended, reset the moment the stores can cover a year's maintenance again. It's what
    /// lets content and the morale drain tell a *chronic* disrepair — a ship held together
    /// with tape for a generation — from one lean year between resupplies. 0 at launch.
    #[serde(default)]
    pub lean_parts_years: u32,
    /// Consecutive years the energy store has sat below the *low* line (content-depth provisioning
    /// round 34): a rolling count of how long the grid has run dark — rationed light, systems cycled
    /// off — reset the moment the reactors recover. The energy twin of `lean_food_years`; it lets
    /// content and the it34 morale drain tell a *chronic* power poverty from one lean season, and
    /// it is the mechanical companion to the it33 power *voice* that narrates the dark grid. 0 at
    /// launch.
    #[serde(default)]
    pub lean_energy_years: u32,
    /// Founding factions carried aboard (W7). `sum(members of Aboard) ==
    /// population.count` after every `rebalance_factions`.
    #[serde(default)]
    pub factions: Vec<factions::FactionState>,
    /// Ship subsystems keyed by catalog id (W5): tier, condition, knowledge.
    #[serde(default)]
    pub subsystems: HashMap<String, subsystems::SubsystemState>,
    /// The sealed homecoming report, set when a charter concludes and cleared
    /// when the player dismisses the debrief screen. Serialized so a quit
    /// mid-read comes back to it — a voyage's only summary should not be lost
    /// to closing the window.
    #[serde(default)]
    pub debrief: Option<debrief::VoyageDebrief>,
    pub log: Vec<LogEntry>,
}

impl SimState {
    /// Whole years since founding (W3). Time is stored in months; this is the
    /// display/arithmetic year the rest of the game reasons about.
    pub fn year(&self) -> u32 {
        self.month_clock / 12
    }

    /// Calendar month 1-12 for display (W3).
    pub fn month(&self) -> u32 {
        self.month_clock % 12 + 1
    }

    /// True while any council decision (event or dilemma) blocks the tick.
    pub fn has_pending_decision(&self) -> bool {
        self.pending_event.is_some() || self.pending_dilemma.is_some()
    }

    pub fn push_log(&mut self, text: impl Into<String>) {
        self.log.push(LogEntry {
            year: self.year(),
            month: self.month(),
            text: text.into(),
        });
    }

    /// The ship's current value on a reputation trait (content-depth event families
    /// round 16), 0.5 (neutral) for any trait no outcome has yet touched.
    pub fn reputation(&self, id: &str) -> f32 {
        self.reputation.get(id).copied().unwrap_or(0.5)
    }

    /// Nudge a reputation trait by `delta`, clamped to [0, 1] (content-depth event
    /// families round 16). Many small nudges across a campaign build a tendency.
    pub fn adjust_reputation(&mut self, id: &str, delta: f32) {
        let entry = self.reputation.entry(id.to_owned()).or_insert(0.5);
        *entry = (*entry + delta).clamp(0.0, 1.0);
    }

    pub fn trim_log(&mut self, limit: usize) {
        if self.log.len() > limit {
            let excess = self.log.len() - limit;
            self.log.drain(..excess);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GameData;

    #[test]
    fn new_campaign_is_deterministic_for_same_seed() {
        let data = GameData::load().unwrap();
        let a = SimState::new_campaign(
            &data,
            "preservers",
            42,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let b = SimState::new_campaign(
            &data,
            "preservers",
            42,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let names_a: Vec<_> = a.dynasty.members.iter().map(|m| m.name.clone()).collect();
        let names_b: Vec<_> = b.dynasty.members.iter().map(|m| m.name.clone()).collect();
        assert_eq!(names_a, names_b);
        assert_eq!(a.dynasty.leader().unwrap().age, 45);
    }

    #[test]
    fn resource_pool_clamps_at_zero_and_checks_affordability() {
        let mut pool = ResourcePool {
            credits: 100,
            ..Default::default()
        };
        let cost = crate::data::ResourceDelta {
            credits: -150,
            ..Default::default()
        };
        assert!(!pool.can_afford(&cost));
        pool.apply(&cost);
        assert_eq!(pool.credits, 0);
    }

    #[test]
    fn sim_state_round_trips_through_serde() {
        let data = GameData::load().unwrap();
        let sim = SimState::new_campaign(
            &data,
            "wanderers",
            7,
            &crate::state::sim::founding_faction_ids(&data),
        );
        let json = serde_json::to_string(&sim).unwrap();
        let back: SimState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.dynasty.members.len(), sim.dynasty.members.len());
        assert_eq!(back.legacy.legacy_id, "wanderers");
        // The reign roster is part of the saved state — a campaign that loses it
        // loses every captaincy before the sitting one.
        assert_eq!(back.dynasty.reigns.len(), sim.dynasty.reigns.len());
        assert_eq!(back.dynasty.reigns[0].name, sim.dynasty.reigns[0].name);
    }

    #[test]
    fn an_unread_homecoming_survives_a_save_and_load() {
        // The debrief is a full-screen takeover the player dismisses by hand.
        // Quitting while it is up and loading back must return to it — a
        // voyage's only summary should not be lost to closing the window.
        let data = GameData::load().unwrap();
        let mut sim = SimState::new_campaign(
            &data,
            "preservers",
            11,
            &crate::state::sim::founding_faction_ids(&data),
        );
        sim.debrief = Some(debrief::VoyageDebrief {
            contract_name: "Salvage Writ: The Long Tow".to_owned(),
            outcome: "Partial".to_owned(),
            score: 0.62,
            duration_years: 450,
            highlights: vec![debrief::VoyageHighlight {
                year: 22,
                month: 7,
                kind: debrief::HighlightKind::Milestone,
                text: "Departure burn complete".to_owned(),
            }],
            commanders: vec![sim.dynasty.reigns[0].clone()],
            ..Default::default()
        });

        let json = serde_json::to_string(&sim).unwrap();
        let back: SimState = serde_json::from_str(&json).unwrap();
        let report = back.debrief.expect("the unread report came back");
        assert_eq!(report.contract_name, "Salvage Writ: The Long Tow");
        assert_eq!(report.outcome, "Partial");
        assert_eq!(report.duration_years, 450);
        assert_eq!(report.highlights.len(), 1);
        assert_eq!(report.highlights[0].kind, debrief::HighlightKind::Milestone);
        assert_eq!(report.commanders.len(), 1);

        // …and a filed report stays filed.
        sim.debrief = None;
        let json = serde_json::to_string(&sim).unwrap();
        let back: SimState = serde_json::from_str(&json).unwrap();
        assert!(back.debrief.is_none());
    }
}
