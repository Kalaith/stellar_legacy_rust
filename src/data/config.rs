//! `game_config.json` in Rust: the one tuning surface the whole sim reads.
//! Split by area, re-exported flat so `crate::data::<Thing>Config` still
//! resolves everywhere.

use serde::{Deserialize, Serialize};

use crate::data::factions::FactionConfig;
use crate::data::subsystems::SubsystemsConfig;
use crate::data::{ProductionRates, ResourceDelta};

mod campaign;
mod crew;
mod flavor;
mod onboarding;
mod ship;

pub use campaign::*;
pub use crew::*;
pub use flavor::*;
pub use onboarding::*;
pub use ship::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub game_name: String,
    pub display_name: String,
    pub save_slot: String,
    pub chronicle_slot: String,
    pub version: String,
    pub starting_resources: ResourceDelta,
    pub base_production: ProductionRates,
    pub starting_population: u32,
    pub food_per_person_per_year: f32,
    pub low_food_threshold: i64,
    pub low_energy_threshold: i64,
    /// Fraction of *industrial* production (credits + minerals) shed at zero energy while the
    /// ship is below `low_energy_threshold` (content-depth provisioning round 29): power runs the
    /// whole ship, not only its life-support (it15) and its fabricators (it21) — a reactor short
    /// of reserve cannot keep the factories and refineries at full output, so a power-starved ship
    /// *earns and mines less*. The industrial output is scaled by `1 - this·(1 - energy/threshold)`
    /// below the line (full at the line, `1 - this` at empty tanks). Food is spared — a crew sheds
    /// the factory before the grow-lamps — so this cannot cascade into famine. 0 = power scarcity
    /// does not touch production (the default; energy stays purely a life-support/fabrication
    /// resource).
    #[serde(default)]
    pub low_energy_production_shed: f32,
    /// How much a unit of a bulk trade moves the local price against the ship
    /// (content-depth provisioning round 22): the market's first responsiveness to the
    /// ship's own actions — a lone generation ship is a whale in a thin waypoint market,
    /// so stocking up drives a price up and dumping a surplus drives it down, scaled by
    /// this per-unit fraction of the resource's base price and clamped to the drift's
    /// 0.5x-3x band. 0 = a bottomless market a single ship never moves. Copied onto the
    /// `MarketState` at campaign start.
    #[serde(default)]
    pub market_impact_per_unit: f32,
    /// How much the ship's *reputation* bends its trade terms (content-depth provisioning round
    /// 30): the market's second responsiveness (the it22 volume coupling was the first). A
    /// waystation prices for who it is dealing with — a merciful, well-regarded hull is dealt with
    /// squarely (it buys cheaper and sells dearer), a feared or ruthless one draws a risk premium
    /// (it buys dear and sells cheap). The effective price is scaled by `1 ∓ this·(mercy − 0.5)`
    /// (minus on a buy, plus on a sell), so a neutral name (0.5) trades at the base and no
    /// fresh-campaign path changes. 0 = the market ignores the ship's name. Copied onto
    /// `MarketState` at campaign start.
    #[serde(default)]
    pub trade_reputation_scale: f32,
    /// The premium a ship pays to *buy* a survival good it is critically low on (content-depth
    /// provisioning round 32): the market's third responsiveness (volume it22, name it30, now
    /// *need*). Traders read a near-empty hold and price the desperation in — buying food with the
    /// larder near famine (below `low_food_threshold`) or energy with the grid near dark (below
    /// `low_energy_threshold`) costs `1 + this`. It rewards buying *early*, before the ship is over
    /// a barrel, and stings the crisis-buyer who waited. Applies to buys only; copied onto
    /// `MarketState` at campaign start. 0 = the market charges the same however empty the hold.
    #[serde(default)]
    pub market_desperation_premium: f32,
    /// The discount the market takes on a *sell* made while the ship is broke (content-depth
    /// provisioning round 33): the sell-side mirror of `market_desperation_premium`. A ship selling
    /// its stores because the coffers are bare — credits below `distress_credit_floor` — is
    /// lowballed by `1 - this`, the trader smelling a fire sale. Together with the buy premium the
    /// market now reads the ship's position from both sides: gouged buying what it lacks, lowballed
    /// selling because it is broke. Applies to sells only; copied onto `MarketState` at campaign
    /// start. 0 = the market pays the same however empty the treasury.
    #[serde(default)]
    pub market_distress_discount: f32,
    /// The credit level below which a *sell* reads as a distress sale (content-depth provisioning
    /// round 33): the treasury-bare line the it33 sell discount bites below. Copied onto
    /// `MarketState` at campaign start. 0 = no sell is ever distressed.
    #[serde(default)]
    pub distress_credit_floor: i64,
    /// The food store the ship can keep *fresh* — its carrying capacity (content-depth
    /// provisioning round 24): food is the one resource with no upkeep and no cap, so it
    /// could otherwise pile up without limit. Everything above this line spoils by
    /// `food_spoilage_fraction` per year (of the excess), a gentle soft cap: sensible
    /// stores lose nothing, only a deep hoard erodes toward what the cold-holds and
    /// hydroponics can actually cycle. Set comfortably above `fat_food_threshold` so a
    /// prudent reserve still reads as plenty. 0 = stores never spoil.
    #[serde(default)]
    pub food_carrying_capacity: i64,
    /// Fraction of the food held *above* `food_carrying_capacity` lost each year to
    /// spoilage (content-depth provisioning round 24). Gentle — the hoard asymptotes
    /// toward the capacity rather than being clipped to it, so a temporary surplus is
    /// tolerated and only a sustained one wears away.
    #[serde(default)]
    pub food_spoilage_fraction: f32,
    /// Governance line below which the ship's *influence* income begins to fall
    /// (content-depth provisioning round 26): influence is political capital, and political
    /// capital is only as real as the institutions that mint it — a ship whose `stability`
    /// has slipped below this line generates less of it, a council that cannot reach quorum
    /// unable to issue the authority its officers spend. At or above the line, full income
    /// (inert). 0 = the coupling is off (influence income ignores governance).
    #[serde(default)]
    pub influence_governance_threshold: f32,
    /// Fraction of influence income that survives even a total governance collapse
    /// (content-depth provisioning round 26): the floor the income factor decays toward as
    /// `stability` falls from the threshold to 0, so even an ungoverned ship mints a little
    /// political capital (raw standing, not institutional authority) — never zero. Must sit
    /// in [0, 1); the factor runs `floor + (1 - floor)·(stability / threshold)` below the
    /// line and clamps to 1.0 at or above it.
    #[serde(default)]
    pub influence_governance_floor: f32,
    /// Food store below which a year counts as *lean* (content-depth provisioning
    /// round 13): distinct from the near-famine `low_food_threshold`, this is the
    /// "not comfortably stocked" line whose sustained crossing drives `lean_food_years`
    /// — the state that separates a bad year from a bad generation. 0 = disabled.
    #[serde(default)]
    pub lean_food_threshold: i64,
    /// Food store at or above which a year counts as *fat* (content-depth
    /// provisioning round 14): the symmetric mirror of `lean_food_threshold` — the
    /// "comfortably flush" line whose sustained crossing drives `fat_food_years`, the
    /// state that separates a windfall year from a lifetime of plenty. 0 = disabled.
    #[serde(default)]
    pub fat_food_threshold: i64,
    /// Years of sustained lean the crew endures before chronic hunger begins to wear
    /// their spirits (content-depth provisioning round 17): the provisioning axis's
    /// first *systemic* coupling. Once `lean_food_years` reaches this, the year tick
    /// drains a little morale each year the lean holds — so a grinding multi-year
    /// hunger doesn't merely gate content (it89) and read hungry (voice r13), it
    /// mechanically wears the ship down. A single bad winter stays below it (the acute
    /// famine events' domain). 0 = no chronic-hunger toll.
    #[serde(default)]
    pub chronic_hunger_years: u32,
    /// Morale drained per year while the ship is in a sustained lean past
    /// `chronic_hunger_years` (content-depth provisioning round 17). Gentle by design —
    /// the slow attrition of a hunger that will not end, not a single hard blow.
    #[serde(default)]
    pub chronic_hunger_morale_drain: f32,
    /// Morale worn away each year the ship has been *chronically becalmed* — stalled dry
    /// past `chronic_hunger_years` running (content-depth provisioning round 25): the
    /// fuel/mobility twin of `chronic_hunger_morale_drain`. Where a hunger that will not
    /// end wears the crew's spirits, so does a *voyage* that will not move — a ship going
    /// nowhere for years loses heart. The standing cost beside the it25 becalmed *beat*
    /// (the reckoning): the beat confronts the stranding once, this grinds at the crew
    /// every year it holds. Gentle by design; 0 = a becalming costs no morale.
    #[serde(default)]
    pub becalmed_morale_drain: f32,
    /// Morale worn away each year the ship has been *chronically unmended* — short of its
    /// spare-parts upkeep past `chronic_hunger_years` running (content-depth provisioning round
    /// 27): the third sustained-privation morale cost, beside `chronic_hunger_morale_drain`
    /// (the larder) and `becalmed_morale_drain` (the drive). Where a hunger that will not end
    /// and a voyage that will not move each wear the crew's spirits, so does a *home that will
    /// not stay whole* — a ship where the deck plates buckle and the seals weep and there is
    /// nothing to fix them with grinds the heart down year over year. Gentle by design; 0 = a
    /// long disrepair costs no morale.
    #[serde(default)]
    pub disrepair_morale_drain: f32,
    /// Morale worn away each year the ship's grid has been *chronically dark* — energy below
    /// `low_energy_threshold` past `chronic_hunger_years` running (content-depth provisioning round
    /// 34): the fourth sustained-privation morale cost, beside `chronic_hunger_morale_drain` (the
    /// larder), `becalmed_morale_drain` (the drive), and `disrepair_morale_drain` (the home). A ship
    /// run for years on rationed light and cold decks — systems cycled off to keep the essential
    /// ones lit — wears the crew's spirit the way a chronic hunger does, the standing morale cost
    /// the it33 power *voice* only narrates. Same sustained gate as the others; gentle by design; 0
    /// = a long power poverty costs no morale.
    #[serde(default)]
    pub chronic_low_energy_morale_drain: f32,
    /// Approval each aboard people loses per year while the ship is in a sustained lean past
    /// `chronic_hunger_years` (content-depth provisioning round 28): the *political* toll of a
    /// long hunger, beside its toll on the crew's spirits (`chronic_hunger_morale_drain`, it17)
    /// and bodies (`chronic_hunger_death_bonus`, it18). A people that goes hungry stops trusting
    /// the council that rations it — so a chronic shortage sours every aboard faction, and the
    /// discontent feeds the whole faction machinery (the it100 approval→unity cohesion, the
    /// it withdrawal beats, the it13 demographic drift): hunger does not only wear the ship, it
    /// turns the peoples against their government. Gentle by design; 0 = a long hunger costs no
    /// faction goodwill.
    #[serde(default)]
    pub chronic_hunger_faction_penalty: f32,
    /// Extra *monthly death chance* added to every character while the ship has been
    /// lean past `chronic_hunger_years` (content-depth provisioning round 18 — the
    /// provisioning axis's coupling to the real-time-loop mortality system). Where
    /// `chronic_hunger_morale_drain` wears the crew's *spirits*, this wears their
    /// *bodies*: a hunger that grinds on for years thins the roster, the old and weak
    /// first. Added to the age curve (a well-kept infirmary still eases it, the hard
    /// age cap still holds). Gentle — the slow toll of long want, not a famine's blow.
    /// 0 = chronic hunger costs no lives directly.
    #[serde(default)]
    pub chronic_hunger_death_bonus: f32,
    /// Fractional boost to the dynasty's yearly renewal while the ship has stood in
    /// sustained plenty past `chronic_hunger_years` (content-depth provisioning round
    /// 19 — the positive pole of `chronic_hunger_death_bonus`, and the mirror of the
    /// hunger's toll). Where a long lean thins the roster, a long plenty fills the
    /// cradles: a well-fed generation raises more of its young to their majority, so
    /// the birth chance is multiplied by `1 + this` while the fat years hold. A second
    /// lever (with the habitat, it152) on the renewal that stands between the line and
    /// extinction. 0 = plenty gives no renewal boost.
    #[serde(default)]
    pub sustained_plenty_birth_bonus: f32,
    /// Morale added per year while the ship has stood in sustained plenty past
    /// `chronic_hunger_years` (content-depth provisioning round 20 — the morale mirror
    /// of `chronic_hunger_morale_drain`, on the same threshold). A well-fed generation
    /// is a happier one, so a long fat spell eases the crew's spirits each year it
    /// holds, completing the provisioning→morale pole (hunger wears it down, plenty
    /// lifts it back) beside the death/birth couplings. Gentle by design. 0 = plenty
    /// gives no morale lift.
    #[serde(default)]
    pub sustained_plenty_morale_lift: f32,
    /// Yearly approval each aboard faction gains while a fat spell holds past
    /// `chronic_hunger_years` (content-depth provisioning round 31). The *political*
    /// mirror of `chronic_hunger_faction_penalty`, on the same sustained-plenty gate as
    /// `sustained_plenty_morale_lift`: a people fed well and long comes to trust the
    /// council that keeps its holds full, so the standing granary warms every aboard
    /// faction the way a chronic hunger sours it — closing the food→faction pole (hunger
    /// against, plenty toward) beside the food→morale and food→body poles. Gentle by
    /// design, and the exact positive counterpart of the penalty's [0, 0.05] guard.
    /// 0 = plenty wins no goodwill.
    #[serde(default)]
    pub sustained_plenty_faction_bonus: f32,
    /// Energy level at or above which the ship's surplus reactor output is run into
    /// the fabricators (content-depth provisioning round 21). Energy, unlike food, has
    /// no upkeep — it simply *accumulates and sits idle*, the voyage's one wholly
    /// wasted resource. This gives that idle power a purpose: while energy is above
    /// this line, the ship converts spare watts and raw minerals into spare parts,
    /// fabricating its own maintenance stock in flight. The provisioning axis's first
    /// coupling off *food* — a conversion (energy + minerals → parts), self-throttling
    /// (the run spends energy back below the line). 0 = the fabricators stay cold.
    #[serde(default)]
    pub surplus_energy_threshold: i64,
    /// Energy spent per year running the fabricators while in surplus (round 21):
    /// enough that the run draws the idle pile back down, so surplus energy is genuinely
    /// *used*, not merely checked.
    #[serde(default)]
    pub fabrication_energy_cost: i64,
    /// Raw minerals consumed per fabrication year (round 21): the feedstock the spare
    /// power works into parts — so the conversion needs both idle watts *and* ore, and
    /// never runs a mineral-poor ship's stores dry (it is gated on holding at least this
    /// much).
    #[serde(default)]
    pub fabrication_minerals_cost: i64,
    /// Spare parts a fabrication year yields (round 21): the maintenance stock the
    /// surplus buys, feeding the it `parts_upkeep_per_year` decay relief — so a
    /// power-rich ship keeps itself in better repair off its own idle reactors.
    #[serde(default)]
    pub fabrication_parts_yield: i64,
    pub hull_warning_threshold: f32,
    pub life_support_warning_threshold: f32,
    pub hull_decay_per_year: f32,
    pub life_support_decay_per_year: f32,
    /// Spare parts the ship launches with (W1-rescale). A generational voyage
    /// carries a deeper store than the old ~55-yr charters needed.
    pub starting_spare_parts: i64,
    /// Spare parts spent per year keeping the ship maintained (PLAN M4.2).
    /// While parts remain to cover it, yearly wear is eased by
    /// `maintenance_decay_relief`; once the stores run dry, wear is full rate.
    pub parts_upkeep_per_year: i64,
    /// Fraction of a year's hull/life-support decay avoided while the ship is
    /// maintained (0 = no relief, 0.4 = 40% less wear that year).
    pub maintenance_decay_relief: f32,
    pub generation_interval_years: u32,
    pub leader_retirement_age: u32,
    pub heir_min_age: u32,
    pub heir_max_age: u32,
    pub member_max_age: u32,
    pub event_chance_base: f32,
    pub event_chance_cap: f32,
    /// Chance a legacy dilemma confronts each new generation (GDD §5.5).
    pub dilemma_chance_per_generation: f32,
    /// Founding-faction tunables (W7).
    pub factions: FactionConfig,
    /// Pre-launch provisioning + fuel-as-consumable tunables (W4).
    pub provisioning: ProvisioningConfig,
    /// First-voyage tutorial content: the drydock hint and PREP checklist.
    pub tutorial: TutorialConfig,
    /// First-run welcome overlay content (shown once, per install).
    pub welcome: WelcomeConfig,
    /// Ship-subsystem knowledge/training tunables (W5).
    pub subsystems: SubsystemsConfig,
    /// Seeded-campaign-skeleton beat pools + era layering (content-depth).
    pub campaign_skeleton: CampaignSkeletonConfig,
    /// Generational obituary/succession/coming-of-age flavor pools (content-depth
    /// voice iteration).
    pub flavor: FlavorConfig,
    pub crew: CrewConfig,
    /// Per-character aging + death (real-time loop follow-up).
    pub mortality: MortalityConfig,
    pub failure_risk: FailureRiskConfig,
    pub ship: ShipConfig,
    /// Per-year population drift over a voyage (PLAN M4.1).
    pub voyage_drift: VoyageDrift,
    /// Field-vs-port repair tunables (PLAN M4.3).
    pub repair: RepairConfig,
    /// Real-time voyage pacing (real-time loop): auto-advance cadence, decision
    /// auto-resolve timeout, and ranged-impact tuning.
    pub real_time: RealTimeConfig,
    /// Fixed campaign seed for reproducible testing (real-time loop follow-up).
    /// When set, every New Game uses this exact seed, so the same events fire in
    /// the same order run to run. `null` (the default) picks a fresh random seed
    /// per campaign from the wall-clock-seeded generator.
    #[serde(default)]
    pub fixed_seed: Option<u64>,
    /// Gating for installing salvaged parts underway (PLAN M4.4).
    pub field_install: FieldInstallConfig,
    /// Commission-a-new-ship tunables (PLAN M4.5).
    pub commission: CommissionConfig,
    /// Heritage tiers (GDD §7), ascending by `min_renown`. The highest tier a
    /// new dynasty's accumulated Chronicle renown clears grants its bonus.
    pub heritage: Vec<HeritageTier>,
    pub log_limit: usize,
}
