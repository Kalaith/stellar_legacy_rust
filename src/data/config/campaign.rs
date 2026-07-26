//! The campaign skeleton: which event families a beat may draw from, when
//! the scripted beats fire, and the thresholds that arm them.

use serde::{Deserialize, Serialize};

/// Seeded-campaign-skeleton tunables (content-depth iteration): the phase→family
/// beat pools, moved out of Rust so the campaign's shape is data like everything
/// else, plus era layering that tints founding-era and homecoming-era beats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignSkeletonConfig {
    /// One beat per this many months of mission duration.
    pub months_per_window: u32,
    /// No beats before this many months into the voyage.
    pub skip_months: u32,
    /// Family pools drawn from by the phase a beat lands in.
    pub travel_pool: Vec<String>,
    pub operation_pool: Vec<String>,
    pub return_pool: Vec<String>,
    /// Families eligible in any phase, always added to the draw.
    pub any_pool: Vec<String>,
    /// Extra families layered in for beats in the first `early_fraction` of the
    /// voyage (founding-era texture) and the last `late_fraction`→end
    /// (homecoming-era texture).
    pub early_pool: Vec<String>,
    pub late_pool: Vec<String>,
    pub early_fraction: f32,
    pub late_fraction: f32,
    /// Extra families layered into beats in the deep middle of the voyage
    /// (between `early_fraction` and `late_fraction`) — the era no living soul
    /// remembers launching into and none expects to see the end of, when the
    /// ship is the only world anyone has known (content-depth round 4). Empty =
    /// no mid-era tint.
    #[serde(default)]
    pub mid_pool: Vec<String>,
    /// Cultural-drift thresholds (ascending) that each fire one beat the first
    /// time the voyage crosses them (content-depth round 2). This is how the
    /// signature Long-Term Expedition beats read as *consequences of the long
    /// voyage* — the people having drifted far enough — rather than random
    /// rolls. Empty = no drift beats.
    #[serde(default)]
    pub drift_beats: Vec<f32>,
    /// The family a drift-threshold beat draws from.
    #[serde(default)]
    pub drift_beat_family: String,
    /// Adaptation thresholds (ascending), the physiological/instinctive parallel
    /// to `drift_beats` (content-depth round 3): each fires one beat the first
    /// time the people's `adaptation` crosses it — the descendants growing suited
    /// to the ship in body and habit. Empty = no adaptation beats.
    #[serde(default)]
    pub adaptation_beats: Vec<f32>,
    /// The family an adaptation-threshold beat draws from.
    #[serde(default)]
    pub adaptation_beat_family: String,
    /// Dead-air backstop (content-depth round 5): the most years the voyage may
    /// pass with no event before the skeleton *forces* one. Long eventless
    /// stretches are a content-coverage bug, not a mercy — beyond this gap a beat
    /// is guaranteed. 0 = no backstop.
    #[serde(default)]
    pub dead_air_years: u32,
    /// Families a forced dead-air beat may draw from (one picked via state RNG,
    /// so it stays deterministic). Must be non-empty when `dead_air_years` > 0.
    #[serde(default)]
    pub dead_air_pool: Vec<String>,
    /// Cohesion-collapse thresholds (content-depth round 6): the *descending*
    /// mirror of `drift_beats`/`adaptation_beats`. As the people's `unity` falls
    /// to or below each threshold (thresholds authored high→low), a beat is
    /// forced — the ship coming apart surfaces its own reckoning rather than
    /// waiting on a random roll. Empty = no crisis beats.
    #[serde(default)]
    pub crisis_beats: Vec<f32>,
    /// The family a cohesion-collapse crisis beat draws from.
    #[serde(default)]
    pub crisis_beat_family: String,
    /// Recovery threshold (content-depth round 13): the crisis beat's *hopeful
    /// mirror*. Once the ship has fractured (a crisis beat has fired) and its
    /// `unity` then climbs back to or above this, a beat is forced — the mending, a
    /// ship pulling itself back from the brink — and the crisis counter is reset so
    /// a relapse re-arms the collapse beats. Set well above the crisis thresholds
    /// for hysteresis (the band between neither fires). 0 = no recovery beat.
    #[serde(default)]
    pub recovery_beat_threshold: f32,
    /// The family a recovery/mending beat draws from.
    #[serde(default)]
    pub recovery_beat_family: String,
    /// Loyalty-collapse thresholds (content-depth round 14): the last identity stat
    /// without a beat. As the people's `legacy_loyalty` falls to or below each
    /// threshold (authored high→low), a beat is forced — not the *cultural* drift the
    /// drift beats mark (becoming someone new) but the *political* one: the founders'
    /// covenant lapsing, a generation that no longer treats the founding charter as
    /// binding. Empty = no loyalty beats.
    #[serde(default)]
    pub loyalty_beats: Vec<f32>,
    /// The family a loyalty-collapse beat draws from.
    #[serde(default)]
    pub loyalty_beat_family: String,
    /// Covenant-recovery threshold (content-depth campaign-skeleton round 31): the loyalty beat's
    /// *hopeful mirror*, and the last of the four decline stats to get a recovery beat (after
    /// unity it13, stability it28, and morale/despair it30). Once the founders' covenant has
    /// lapsed (a loyalty beat has fired) and `legacy_loyalty` then climbs back to or above this, a
    /// beat is forced — the covenant *renewed*, a generation that had drifted from the founding
    /// cause returning to it, the charter re-embraced as binding — and the loyalty-collapse
    /// counter is reset so a relapse re-arms the collapse beats. Set above the worst loyalty
    /// threshold for hysteresis. 0 = no covenant-recovery beat (the ship only ever marks the
    /// covenant lapsing, never its renewal).
    #[serde(default)]
    pub loyalty_recovery_beat_threshold: f32,
    /// The family the covenant-recovery beat draws from.
    #[serde(default)]
    pub loyalty_recovery_beat_family: String,
    /// Governance-collapse thresholds (content-depth round 15): the last population
    /// stat without a beat. As `stability` falls to or below each threshold (high→
    /// low), a beat is forced — not the *people* fracturing (the crisis beat) nor
    /// the *founders'* authority lapsing (the loyalty beat), but the ship's own
    /// institutions ceasing to function: councils that cannot reach quorum, offices
    /// unfilled, the charter gone to folklore. Empty = no stability beats.
    #[serde(default)]
    pub stability_beats: Vec<f32>,
    /// The family a governance-collapse stability beat draws from.
    #[serde(default)]
    pub stability_beat_family: String,
    /// Governance-recovery threshold (content-depth campaign-skeleton round 28): the stability
    /// beat's *hopeful mirror*, the exact twin of the it13 unity `recovery_beat_threshold`. Once
    /// the ship's institutions have collapsed (a stability beat has fired) and its `stability`
    /// then climbs back to or above this, a beat is forced — the government rebuilt, councils
    /// reconvened, the charter re-codified, a ship pulling its own institutions back from anarchy
    /// — and the stability-collapse counter is reset so a relapse re-arms the collapse beats. Set
    /// above the collapse thresholds for hysteresis. 0 = no governance-recovery beat (the ship
    /// only ever marks its institutions failing, never their rebuilding).
    #[serde(default)]
    pub stability_recovery_beat_threshold: f32,
    /// The family the governance-recovery beat draws from.
    #[serde(default)]
    pub stability_recovery_beat_family: String,
    /// Reputation beat (content-depth round 16): the skeleton's first trigger on the
    /// ship's *cumulative character* (it105) rather than a population stat. When the
    /// named reputation trait crosses *into* a strong band — famously high (≥ `high`)
    /// or notoriously low (≤ `low`) — a beat is forced: the ship reckoning with the
    /// name it has earned. A return to the middle re-arms it. Empty trait/family = off.
    #[serde(default)]
    pub reputation_beat_trait: String,
    #[serde(default)]
    pub reputation_beat_high: f32,
    #[serde(default)]
    pub reputation_beat_low: f32,
    #[serde(default)]
    pub reputation_beat_family: String,
    /// Flourishing thresholds (content-depth round 8): the *positive* pole of the
    /// crisis beat. As the people's `morale` climbs to or past each threshold
    /// (authored low→high) a beat is forced — a thriving ship generates its own
    /// golden age, so good stewardship surfaces its own beats, not only decline.
    /// Empty = no flourish beats.
    #[serde(default)]
    pub flourish_beats: Vec<f32>,
    /// The family a golden-age flourish beat draws from.
    #[serde(default)]
    pub flourish_beat_family: String,
    /// Despair thresholds (content-depth campaign-skeleton round 29): the *descending* negative
    /// pole of the `flourish_beats` — where flourish marks morale climbing into a golden age,
    /// this marks it *crashing* into a collective despair. As the crew's `morale` falls to or
    /// below each threshold (authored high→low) a beat is forced — the missing morale-collapse
    /// beat, distinct from the it6 crisis beat (which watches `unity` *fracturing*, not spirits
    /// *sinking*). So a ship that loses heart, not only one that comes apart, surfaces its own
    /// reckoning rather than waiting on a reactive roll. Empty = no despair beats.
    #[serde(default)]
    pub despair_beats: Vec<f32>,
    /// The family a morale-collapse despair beat draws from.
    #[serde(default)]
    pub despair_beat_family: String,
    /// Heartening-recovery threshold (content-depth campaign-skeleton round 30): the despair
    /// beat's *hopeful mirror*, the morale twin of the it13 unity and it28 stability recovery
    /// beats. Once the crew has sunk into despair (a despair beat has fired) and its `morale` then
    /// climbs back to or above this, a beat is forced — spirits lifting from the depths, a crew
    /// that had lost heart finding it again — and the despair counter is reset so a relapse
    /// re-arms the collapse beats. Set above the worst despair threshold for hysteresis, and below
    /// the golden-age flourish band (this marks a climb back to a *livable* baseline, not a
    /// triumph). 0 = no heartening-recovery beat (the ship only ever marks the sinking, never the
    /// lifting).
    #[serde(default)]
    pub heartening_recovery_beat_threshold: f32,
    /// The family the heartening-recovery beat draws from.
    #[serde(default)]
    pub heartening_recovery_beat_family: String,
    /// Depopulation thresholds (content-depth round 12): the crew's *headcount*
    /// finally gets a beat — the one major state dimension none watched. As the
    /// population falls to or below each fraction of its *founding* size (authored
    /// high→low, e.g. 0.6/0.4/0.25 of the launch thousands), a beat is forced — the
    /// sealed ship's defining slow tragedy, the decks thinning across the centuries,
    /// marked at its stages. Campaign-scoped (fires once per fraction a voyage, not
    /// per contract). Empty = no depopulation beats.
    #[serde(default)]
    pub depopulation_beats: Vec<f32>,
    /// The family a crew-thinning depopulation beat draws from.
    #[serde(default)]
    pub depopulation_beat_family: String,
    /// Objective-progress thresholds (content-depth round 9): the first pacing
    /// keyed to *the mission itself* rather than time or an identity stat. As the
    /// active charter's `objective_fraction` crosses each (authored low→high) a
    /// beat is forced — the crew's bond to a purpose most of them will not live
    /// to see completed, marked at its milestones. Empty = no objective beats.
    #[serde(default)]
    pub objective_beats: Vec<f32>,
    /// The family a mission-progress objective beat draws from.
    #[serde(default)]
    pub objective_beat_family: String,
    /// Homecoming beat family (content-depth round 10): the first beat keyed to a
    /// voyage *phase* rather than a stat, time, or the objective. Once the charter
    /// turns for home (enters its Return leg) a single beat is forced from this
    /// family — the climactic identity reckoning the doc names, a generation
    /// meeting a homeport that still remembers the founders it no longer resembles.
    /// Empty = no homecoming beat.
    #[serde(default)]
    pub homecoming_beat_family: String,
    /// Mid-voyage beat family (content-depth campaign-skeleton round 21): the era
    /// counterpart to the homecoming beat and the founding-era pool bias — the beat
    /// the "early / mid / homecoming" texture was missing in the middle. Once the
    /// voyage passes its temporal midpoint *with home still ahead* (before the Return
    /// leg), a single beat is forced from this family: the deep middle, when the
    /// founders are generations dead and landfall generations away, and the crew who
    /// will neither remember the launch nor see the arrival must reckon with a life
    /// lived wholly in transit. Empty = no mid-voyage beat.
    #[serde(default)]
    pub midvoyage_beat_family: String,
    /// Founding-era beat family (content-depth campaign-skeleton round 22): the early
    /// member of the era trio, completing what the founding-era pool bias (r5) only
    /// tilted and the mid-voyage/homecoming beats forced for the later eras. The
    /// campaign-year the voyage passes `founding_beat_year`, a single beat is forced from
    /// this family: the founding generation — the ones who chose to leave — having by
    /// then largely passed, and the ship handed for the first time wholly to those born
    /// to the void. Empty = no founding beat.
    #[serde(default)]
    pub founding_beat_family: String,
    /// The campaign year the founding-era beat fires (content-depth campaign-skeleton
    /// round 22): set to when the launch generation has largely died out and the ship is
    /// crewed by the first fully ship-born cohorts — early enough to read as the founding
    /// era's close. 0 disables the beat.
    #[serde(default)]
    pub founding_beat_year: u32,
    /// Hull-collapse beat family (content-depth campaign-skeleton round 23): the
    /// structural parallel to the it17 subsystem-collapse beat (which watches a *module's*
    /// condition) — this watches the *ship's own frame*. The moment `hull_integrity`
    /// falls to or below `hull_beat_threshold`, a beat is forced from this family: the
    /// crew confronting that the vessel itself is failing, not merely a system aboard it.
    /// It is the reckoning the it22 hull *voice* only murmurs before. Empty = no hull beat.
    #[serde(default)]
    pub hull_beat_family: String,
    /// Hull integrity at or below which the hull-collapse beat fires (content-depth
    /// campaign-skeleton round 23): a red line well past the it warning threshold — the
    /// ship not merely worn but structurally failing. A refit back above it re-arms the
    /// beat. 0 disables it.
    #[serde(default)]
    pub hull_beat_threshold: f32,
    /// Hull-recovery beat family (content-depth campaign-skeleton round 32): the structural twin
    /// of the crew-stat recovery beats and the *ascending* mirror of the it23 hull-collapse beat.
    /// Once the frame has failed (a hull beat fired) and `hull_integrity` climbs back to or above
    /// `hull_recovery_beat_threshold`, a beat is forced from this family — the crew reckoning with
    /// a vessel dragged back from structural failure and made whole. Empty = no hull-recovery beat.
    #[serde(default)]
    pub hull_recovery_beat_family: String,
    /// Hull integrity at or above which the hull-recovery beat fires (content-depth campaign-
    /// skeleton round 32): set *above* `hull_beat_threshold` so a mere wobble over the red line
    /// does not count as a rebuild — only a genuine refit does (hysteresis). 0 disables it.
    #[serde(default)]
    pub hull_recovery_beat_threshold: f32,
    /// Air-collapse beat family (content-depth campaign-skeleton round 24): the
    /// atmosphere twin of the hull-collapse beat — where that watches the ship's frame,
    /// this watches its *air*. The moment `life_support` falls to or below
    /// `air_beat_threshold`, a beat is forced from this family: the crew confronting that
    /// the ship itself is suffocating. The reckoning the it23 air *voice* only murmurs
    /// before. Empty = no air beat.
    #[serde(default)]
    pub air_beat_family: String,
    /// Life-support at or below which the air-collapse beat fires (content-depth
    /// campaign-skeleton round 24): a red line past the it warning threshold — the air
    /// not merely stale but failing. An overhaul back above it re-arms the beat. 0
    /// disables it.
    #[serde(default)]
    pub air_beat_threshold: f32,
    /// Air-recovery beat family (content-depth campaign-skeleton round 33): the atmosphere twin of
    /// the it32 hull-recovery beat and the *ascending* mirror of the it24 air-collapse beat. Once
    /// the air has failed (an air beat fired) and `life_support` climbs back to or above
    /// `air_recovery_beat_threshold`, a beat is forced from this family — the crew reckoning with a
    /// ship whose air was dragged back from suffocation and made breathable. Empty = no air-recovery
    /// beat.
    #[serde(default)]
    pub air_recovery_beat_family: String,
    /// Life-support at or above which the air-recovery beat fires (content-depth campaign-skeleton
    /// round 33): set *above* `air_beat_threshold` so a mere wobble over the red line does not count
    /// as an overhaul — only a real one does (hysteresis). 0 disables it.
    #[serde(default)]
    pub air_recovery_beat_threshold: f32,
    /// Becalmed beat family (content-depth campaign-skeleton round 25): the *mobility*
    /// twin of the hull/air *integrity* collapse beats — where those watch the ship
    /// falling apart or suffocating, this watches it *stranded*. Once the ship has been
    /// fuel-stalled (a Travel leg dry, unable to burn) for `becalmed_beat_years` running,
    /// a beat is forced from this family: the crew confronting a ship that cannot make its
    /// heading. Empty = no becalmed beat.
    #[serde(default)]
    pub becalmed_beat_family: String,
    /// Consecutive stalled years past which the becalmed beat fires (content-depth
    /// campaign-skeleton round 25): a bad month coasting is not a stranding, so only a
    /// *sustained* stall forces the reckoning. A year that burns again re-arms it. 0
    /// disables it.
    #[serde(default)]
    pub becalmed_beat_years: u32,
    /// Becalmed-recovery beat family (content-depth campaign-skeleton round 34): the mobility twin
    /// of the it32 hull-recovery and it33 air-recovery beats, and the *ascending* mirror of the it25
    /// becalmed collapse beat — the last collapse beat to gain its recovery. Once the ship has been
    /// stranded (a becalmed beat fired) and it *burns again* (`fuel_stall_years` back to 0), a beat
    /// is forced from this family — the crew reckoning with a voyage underway once more. Needs no
    /// threshold: the stall counter resets to 0 in one step, so "moving again" is unambiguous. Empty
    /// = no becalmed-recovery beat.
    #[serde(default)]
    pub becalmed_recovery_beat_family: String,
    /// Adaptation-divergence beat (content-depth campaign-skeleton round 26): the *crew-body*
    /// twin of the hull/air/becalmed *ship-body* crisis beats, and the terminal counterpart to
    /// the gentle ascending `adaptation_beats` milestones — where those mark the descendants
    /// growing suited to the ship, this fires once the people have grown so shipborn they can no
    /// longer survive a planet at all: the founding mission (make landfall) has become
    /// physically impossible. The month `adaptation` first rises to or above this fraction a beat
    /// is forced from `divergence_beat_family`; a fall back below re-arms it (a strong infirmary
    /// holding the line can un-fire the reckoning). 0 disables it. Pairs with the it adaptation
    /// voice: the voice murmurs the drift, the beat forces its point of no return.
    #[serde(default)]
    pub divergence_beat_threshold: f32,
    /// The family the adaptation-divergence beat draws from.
    #[serde(default)]
    pub divergence_beat_family: String,
    /// Cultural-divergence beat (content-depth campaign-skeleton round 27): the *cultural* twin
    /// of `divergence_beat_threshold` (their bodies), and the terminal counterpart to the
    /// ascending `drift_beats` milestones. Where the adaptation beat fires when the crew's
    /// bodies can no longer survive a planet, this fires once their *culture* has drifted so far
    /// the founders' charter is a dead language — the mission carried by rote by people who no
    /// longer understand its why. The month `cultural_drift` first rises to or above this
    /// fraction a beat is forced from `cultural_divergence_beat_family`; a fall back below re-arms
    /// it (a strong archive reviving the old ways can un-fire the reckoning). Set above the top
    /// `drift_beats` milestone so it is the *terminal* mark, not another rung. 0 disables it.
    /// Pairs with the it26 cultural-drift voice: the voice murmurs the drift, the beat forces
    /// its point of no return.
    #[serde(default)]
    pub cultural_divergence_beat_threshold: f32,
    /// The family the cultural-divergence beat draws from.
    #[serde(default)]
    pub cultural_divergence_beat_family: String,
    /// Power-transition beat family (content-depth round 11): a beat keyed not to
    /// a stat or a time but to a *political* change — the first tick the dominant
    /// faction differs from the one the skeleton last marked (demographic drift
    /// grew a minority into the majority, or a schism unseated the largest people),
    /// a beat is forced from this family: the ship reckoning with new leadership.
    /// Empty = no power-transition beat.
    #[serde(default)]
    pub power_transition_beat_family: String,
    /// Succession beat family (content-depth round 18 — the first beat keyed to the
    /// real-time-loop continuous-mortality system): the month a *sitting leader dies
    /// in office*, a beat is forced from this family — the ship reckoning with a
    /// captain lost mid-voyage and an untried heir in the chair. A planned retirement
    /// handoff does not fire it. Empty = no succession beat.
    #[serde(default)]
    pub succession_beat_family: String,
    /// Long-reign beat (content-depth campaign skeleton round 19 — the hopeful mirror
    /// of the succession beat): once a *sitting leader* has held the first chair for
    /// `long_reign_years`, a beat is forced from `long_reign_beat_family` — the ship
    /// reckoning with an era defined by one enduring hand, rare now that continuous
    /// mortality takes most leaders young. Fires once per reign (a succession re-arms
    /// it). 0 / empty = no long-reign beat.
    #[serde(default)]
    pub long_reign_years: u32,
    #[serde(default)]
    pub long_reign_beat_family: String,
    /// Dynasty-crisis beat (content-depth campaign skeleton round 20 — the third
    /// leadership beat, after succession and long-reign): the first beat keyed to the
    /// *dynasty's* headcount rather than the population's. When the founding line
    /// dwindles to or below `dynasty_crisis_size` (continuous mortality outrunning the
    /// renewal), a beat is forced from `dynasty_crisis_beat_family` — the ship staring
    /// at the end of the family that has led it since the founding. Fires once per
    /// brush with extinction; re-arms only once the line is restored to its target
    /// (`mortality.dynasty_target_size`). 0 / empty = no dynasty-crisis beat.
    #[serde(default)]
    pub dynasty_crisis_size: u32,
    #[serde(default)]
    pub dynasty_crisis_beat_family: String,
    /// Anniversary cadence (content-depth round 7): every this-many years of the
    /// voyage, a beat is forced from `anniversary_beat_family` — a periodic
    /// archetype (vs the threshold beats), giving the voyage a commemorative
    /// heartbeat as the founding recedes into ritual over the centuries. 0 = off.
    #[serde(default)]
    pub anniversary_years: u32,
    /// The family an anniversary beat draws from.
    #[serde(default)]
    pub anniversary_beat_family: String,
    /// Subsystem-collapse beats (content-depth round 17): the first forced skeleton
    /// beat keyed to a *subsystem's condition* rather than a stat, time, phase, the
    /// objective, or a political change — the physical-crisis dimension the beat
    /// lattice never watched. The first tick a listed module's condition falls to or
    /// below its red line, a beat is forced from its family: the ship reckoning with
    /// a keystone that has *truly* failed, a guaranteed reckoning where before only a
    /// reactive condition-gated event might (or might not) roll. Campaign-scoped —
    /// fires once per module a voyage, tracked by id, so a repaired-then-re-collapsed
    /// module does not re-mark. Empty = no subsystem beats.
    #[serde(default)]
    pub subsystem_beats: Vec<SubsystemBeat>,
}
/// One subsystem-collapse beat (content-depth campaign skeleton round 17): when the
/// named module's `condition` first falls to or below `threshold`, a beat is forced
/// from `family` — the physical-crisis trigger the beat lattice lacked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubsystemBeat {
    pub subsystem: String,
    pub threshold: f32,
    pub family: String,
}
