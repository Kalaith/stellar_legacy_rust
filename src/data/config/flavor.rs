//! The authored voice: every pooled line the ship, its peoples and its
//! systems can speak, plus the rotation that picks one without repeating.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Generational-flavor line pools (content-depth voice iteration): the
/// most-repeated text in the game — the obituary, succession, and coming-of-age
/// lines that fire every generation — moved out of Rust so they can vary instead
/// of reading the same three strings a dozen times a voyage. Lines are picked
/// deterministically (by generation index, no RNG), so a seed still replays
/// exactly. `{name}` / `{generation}` / `{births}` placeholders are substituted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlavorConfig {
    /// A dynasty member laid to rest. Placeholder: `{name}`.
    pub obituary: Vec<String>,
    /// A new head of the dynasty takes over. Placeholder: `{name}`.
    pub succession: Vec<String>,
    /// A new cohort comes of age. Placeholders: `{generation}`, `{births}`.
    pub coming_of_age: Vec<String>,
    /// A crew member stands down from their post at a generation turnover
    /// (content-depth voice round 5). Fires once per retiring holder — several a
    /// generation — so it needs pool variety or it is a repetition tell.
    /// Placeholder: `{name}`. Empty falls back to the built-in line.
    #[serde(default)]
    pub retirement: Vec<String>,
    /// The dynasty ends with no heir (content-depth voice round 5): the tragic
    /// counterpart to `homecoming`, indexed by generation. Empty falls back to
    /// the built-in line so the ending is never blank.
    #[serde(default)]
    pub extinction: Vec<String>,
    /// A serving officer dies at their post (real-time loop follow-up: characters
    /// age and die on a monthly roll, not only at generation ticks). Placeholders
    /// `{name}`, `{post}`. Indexed by the officer's id; empty falls back.
    #[serde(default)]
    pub crew_death: Vec<String>,
    /// A contract milestone reached (content-depth voice round 19): fires several
    /// times per charter across many charters, so a flat "Milestone reached: X"
    /// read as a form letter. Placeholder `{milestone}`; indexed by log length so
    /// consecutive marks vary. Empty falls back to the built-in line.
    #[serde(default)]
    pub milestone: Vec<String>,
    /// A blocking decision brought before the council (content-depth voice round 19):
    /// the line that precedes *every* decision-required event — dozens a voyage — so a
    /// flat "Council decision required: X" was the game's loudest repetition tell.
    /// Placeholder `{title}`; indexed by log length. Empty falls back.
    #[serde(default)]
    pub council_summons: Vec<String>,
    /// A starving year (content-depth voice round 6): fires once per *year* the
    /// larder is empty, so a multi-year famine needs variety or it reprints one
    /// line. Placeholder: `{losses}`. Indexed by year; empty falls back.
    #[serde(default)]
    pub famine: Vec<String>,
    /// A year coasting on a dry tank (content-depth voice round 6): like famine,
    /// fires once per stalled year. Indexed by year; empty falls back.
    #[serde(default)]
    pub fuel_stall: Vec<String>,
    /// The fabricators working surplus reactor output into spare parts (content-depth
    /// provisioning round 21): fires the years a power-rich ship converts idle energy
    /// and raw ore into maintenance stock, so the parts appearing reads as *something
    /// the ship did*. Placeholder: `{parts}`. Indexed by year; empty falls back.
    #[serde(default)]
    pub fabrication: Vec<String>,
    /// A named officer lost to a *disaster* (content-depth voice round 24): the event
    /// death-claim's officer line, which fires whenever a heavy event or complication
    /// takes a soul from the bridge. Was one flat string; pooled for variety.
    /// Placeholders: `{name}`, `{post}`. Indexed by the log length; empty falls back.
    #[serde(default)]
    pub event_loss_officer: Vec<String>,
    /// A named crew member lost to a disaster (content-depth voice round 24): the
    /// death-claim's non-officer line. Placeholder: `{name}`. Indexed; empty falls back.
    #[serde(default)]
    pub event_loss_member: Vec<String>,
    /// Souls lost to a *failing life-support* (content-depth voice round 24): the it15
    /// life-support mortality line, which reprints every year the air is failing — a real
    /// repetition tell. Placeholder: `{losses}`. Indexed by year; empty falls back.
    #[serde(default)]
    pub life_support_loss: Vec<String>,
    /// Over-deep food stores spoiling past the ship's carrying capacity (content-depth
    /// provisioning round 24): fires the years a hoard beyond what the cold-holds can keep
    /// erodes, so the loss reads as *something that happened* rather than an unexplained
    /// dip. Placeholder: `{spoiled}`. Indexed by year; empty = the spoilage is silent.
    #[serde(default)]
    pub food_spoilage: Vec<String>,
    /// The ramscoop/scanners replenishing reaction mass (real-time loop follow-up:
    /// legible stat changes): a periodic in-world report of the fuel the drive has
    /// gathered and processed over the last few travel years, so the fuel gauge's
    /// rise reads as *something the ship did* rather than an unexplained jump.
    /// Placeholder `{amount}` (whole fuel points gained). Indexed by year; empty =
    /// no fuel-replenishment narration.
    #[serde(default)]
    pub fuel_gain: Vec<String>,
    /// How many voyage years between provisioning reports (real-time loop follow-up):
    /// the fuel-gain line fires at most once per this many years, and only when a
    /// meaningful haul has actually accrued, so a long crossing gets an occasional
    /// legible "here is where your fuel comes from" beat without one line a year.
    /// 0 = disabled.
    #[serde(default)]
    pub fuel_report_gap_years: u32,
    /// A crew officer takes up a post (content-depth voice round 7): the positive
    /// twin of `retirement`, fired whenever a vacancy is filled — repeatedly
    /// across a voyage as posts turn over — so it needs variety and the post's
    /// human name, not the raw archetype id. Placeholders `{name}`, `{post}`.
    /// Indexed by crew id (deterministic). Empty falls back to the built-in line.
    #[serde(default)]
    pub appointment: Vec<String>,
    /// An officer completes a training program (content-depth voice round 7): a
    /// repeatable drydock verb, so it needs variety over the flat bracketed
    /// skill number. Placeholders `{name}`, `{post}`, `{skill}`. Indexed by the
    /// new skill; empty falls back to the built-in line.
    #[serde(default)]
    pub training: Vec<String>,
    /// A people crossing *into* restlessness (content-depth voice round 8): the
    /// otherwise-silent approval meter finally speaks, so the player feels a
    /// faction souring toward its withdrawal. Placeholder `{name}` (the people's
    /// log name). Indexed by year; empty falls back to silence.
    #[serde(default)]
    pub faction_souring: Vec<String>,
    /// A people crossing *into* contentment (content-depth voice round 8): the
    /// positive twin, when goodwill has climbed high. Placeholder `{name}`.
    #[serde(default)]
    pub faction_warming: Vec<String>,
    /// The *whole ship's* morale crossing *into* a heavy band (content-depth voice
    /// round 11): the collective parallel to `faction_souring` — where that voices
    /// one people souring, this voices the decks as a whole going grim. No name;
    /// indexed by year; empty falls back to silence.
    #[serde(default)]
    pub ship_mood_darkening: Vec<String>,
    /// The whole ship's morale crossing *into* a light band (content-depth voice
    /// round 11): the positive twin, the decks lifting together. No name; indexed
    /// by year; empty falls back to silence.
    #[serde(default)]
    pub ship_mood_lifting: Vec<String>,
    /// The ship's *political climate* crossing into broad discontent (content-depth
    /// voice round 15): distinct from the crew's spirits — the peoples as a whole
    /// growing restive about their treatment. No name; indexed by year; empty =
    /// silence.
    #[serde(default)]
    pub polity_souring: Vec<String>,
    /// The political climate crossing into broad ease (content-depth voice round
    /// 15): the peoples as a whole settling, content with their lot. No name.
    #[serde(default)]
    pub polity_warming: Vec<String>,
    /// The ship crossing into a *merciful* reputation (content-depth voice round 16):
    /// the quiet marker, at a gentler threshold than the it109 beat, that the ship's
    /// name has begun to mean kindness in the dark. No name; indexed by year. Empty =
    /// silence. Watches the `campaign_skeleton.reputation_beat_trait`.
    #[serde(default)]
    pub reputation_merciful: Vec<String>,
    /// The ship crossing into a *feared* reputation (content-depth voice round 16):
    /// the mirror — its name beginning to mean the hard thing done without flinching.
    #[serde(default)]
    pub reputation_feared: Vec<String>,
    /// Reputation levels at/above which the ship remarks a merciful name (`_high`) or
    /// at/below which it remarks a feared one (`_low`) — gentler than the beat bands,
    /// so the voice precedes the reckoning (content-depth voice round 16).
    #[serde(default)]
    pub reputation_voice_high: f32,
    #[serde(default)]
    pub reputation_voice_low: f32,
    /// The ship crossing into a *famed-for-wonder* reputation (content-depth voice round 28): the
    /// companion to the mercy reputation voice, on the it28 `wonder` trait — the ship's name
    /// becoming a byword for marvels, its chronicle thick with charted impossibilities. No name;
    /// indexed by year; empty = silence.
    #[serde(default)]
    pub wonder_famed: Vec<String>,
    /// The ship crossing into an *incurious* reputation (content-depth voice round 28): the
    /// mirror — a ship known for keeping its head down, sailing past every strangeness rather
    /// than chasing it. No name; indexed by year.
    #[serde(default)]
    pub wonder_incurious: Vec<String>,
    /// Wonder reputation at/above which the ship remarks a famed-for-marvels name (`_high`) or
    /// at/below which it remarks an incurious one (`_low`) (content-depth voice round 28).
    #[serde(default)]
    pub wonder_voice_high: f32,
    #[serde(default)]
    pub wonder_voice_low: f32,
    /// The ship crossing into a *steadfast* reputation (content-depth voice round 29): the third
    /// built-trait voice, on `resolve` — the ship's name becoming a byword for seeing the hard
    /// thing through, a hull that does not flinch and does not fold. No name; indexed by year;
    /// empty = silence.
    #[serde(default)]
    pub resolve_steadfast: Vec<String>,
    /// The ship crossing into a *yielding* reputation (content-depth voice round 29): the mirror —
    /// a name for folding, for the writ quit half-done, a hull known to buckle when the work turns
    /// grim. No name; indexed by year.
    #[serde(default)]
    pub resolve_yielding: Vec<String>,
    /// Resolve reputation at/above which the ship remarks a steadfast name (`_high`) or at/below
    /// which it remarks a yielding one (`_low`) (content-depth voice round 29).
    #[serde(default)]
    pub resolve_voice_high: f32,
    #[serde(default)]
    pub resolve_voice_low: f32,
    /// The ship's *institutions* crossing into disorder (content-depth voice round 17):
    /// the governance twin of the morale (`ship_mood_darkening`) and polity
    /// (`polity_souring`) voices — distinct from the crew's spirits and from how
    /// content the peoples are, this voices the *machinery of government* beginning to
    /// slip: quorums missed, offices going unfilled, decisions drifting. Gated at a
    /// gentler threshold than the it102 collapse *beat*, so the voice (a fraying
    /// noticed) precedes the reckoning (a government failed). No name; indexed by year;
    /// empty = silence.
    #[serde(default)]
    pub stability_fraying: Vec<String>,
    /// The ship's institutions crossing into good order (content-depth voice round 17):
    /// the positive twin — councils reaching quorum again, offices filled, the charter
    /// honored in practice, the government visibly working. No name; indexed by year.
    #[serde(default)]
    pub stability_firming: Vec<String>,
    /// Stability at/above which the ship remarks its institutions in good order
    /// (`_high`) or at/below which it remarks them fraying (`_low`) — the `_low`
    /// gentler than the it102 collapse-beat bands, so the voice precedes the reckoning
    /// (content-depth voice round 17).
    #[serde(default)]
    pub stability_voice_high: f32,
    #[serde(default)]
    pub stability_voice_low: f32,
    /// The crew's *devotion to the founders' mission* crossing into a guttering band
    /// (content-depth voice round 20): the identity-side twin of the morale/governance
    /// voices, on `legacy_loyalty`. Distinct from the crew's spirits and from how the
    /// people have *changed* (drift) — this voices the founders' *purpose* fading: the
    /// charter read as a story rather than a promise, the young unable to feel why the
    /// ship flies. No name; indexed by year; empty = silence.
    #[serde(default)]
    pub loyalty_guttering: Vec<String>,
    /// The founders' mission burning fierce again (content-depth voice round 20): the
    /// positive twin — a generation that has taken the founders' dream as its own, the
    /// charter honored not from duty but conviction, the purpose felt afresh. No name;
    /// indexed by year.
    #[serde(default)]
    pub loyalty_bright: Vec<String>,
    /// Loyalty at/above which the ship remarks the founders' fire burning bright
    /// (`_high`) or at/below which it remarks the mission guttering (`_low`)
    /// (content-depth voice round 20).
    #[serde(default)]
    pub loyalty_voice_high: f32,
    #[serde(default)]
    pub loyalty_voice_low: f32,
    /// The crew crossing into a *shipborn* body (content-depth voice round 25): the
    /// physiological companion to the loyalty voice, on `adaptation`. When the
    /// descendants have drifted far enough from the baseline-human stock — longer, leaner,
    /// their bones gone light, a people fitted to the ship and no longer to a world — one
    /// of these surfaces. No name; indexed by year; empty = silence.
    #[serde(default)]
    pub crew_shipborn: Vec<String>,
    /// The crew holding to the founders' *baseline* shape (content-depth voice round 25):
    /// the rarer twin — a well-kept infirmary (it25) and a deliberate discipline holding
    /// the bodies human against the ship's pull, so a crew bound for a world stays fit for
    /// one. No name; indexed by year.
    #[serde(default)]
    pub crew_baseline: Vec<String>,
    /// Adaptation at/above which the ship remarks a shipborn crew (`_high`) or at/below
    /// which it remarks one held to baseline (`_low`) (content-depth voice round 25).
    #[serde(default)]
    pub adaptation_voice_high: f32,
    #[serde(default)]
    pub adaptation_voice_low: f32,
    /// The crew crossing into a *new people* in custom and memory (content-depth voice round
    /// 26): the cultural companion to `crew_shipborn` (their bodies), on `cultural_drift`.
    /// When the crew's calendars, festivals, and tongue have drifted far enough that the
    /// founders would not recognise the ship's daily life, one of these surfaces. No name;
    /// indexed by year; empty = silence.
    #[serde(default)]
    pub culture_newfound: Vec<String>,
    /// The crew holding the founders' *ways* close (content-depth voice round 26): the rarer
    /// twin — a well-kept archive (education_culture) and a deliberate keeping-of-faith
    /// holding the old customs against the voyage's drift, so a ship centuries out still keeps
    /// the founders' calendar. No name; indexed by year.
    #[serde(default)]
    pub culture_founders_kept: Vec<String>,
    /// Cultural drift at/above which the ship remarks a new people (`_high`) or at/below which
    /// it remarks the founders' ways kept (`_low`) (content-depth voice round 26).
    #[serde(default)]
    pub drift_voice_high: f32,
    #[serde(default)]
    pub drift_voice_low: f32,
    /// The crew's *cohesion* crossing into a fraying band (content-depth voice round 21):
    /// the fourth internal-state voice, on `unity`. Distinct from the crew's spirits
    /// (`ship_mood_darkening`), the peoples' contentment (`polity_souring`), and the
    /// government's order (`stability_fraying`) — this voices the crew *splintering*: one
    /// people becoming several, wary cliques hardening along deck or trade or bloodline,
    /// the sense of a single crew thinning out. No name; indexed by year; empty = silence.
    #[serde(default)]
    pub unity_fraying: Vec<String>,
    /// The crew pulling back together (content-depth voice round 21): the positive twin —
    /// the cliques softening, the ship remembering it is one crew crossing one dark, a
    /// cohesion that asks no notice because it simply holds. No name; indexed by year.
    #[serde(default)]
    pub unity_cohering: Vec<String>,
    /// Unity at/above which the ship remarks a crew grown close (`_high`) or at/below
    /// which it remarks one fraying (`_low`) (content-depth voice round 21).
    #[serde(default)]
    pub unity_voice_high: f32,
    #[serde(default)]
    pub unity_voice_low: f32,
    /// The ship's *hull* crossing into a groaning band (content-depth voice round 22):
    /// the first voice for the vessel's own body rather than the crew's inner life. When
    /// `hull_integrity` falls into disrepair — the plates weeping at the seams, the frame
    /// groaning on every burn, patches over patches — one of these surfaces. No name;
    /// indexed by year; empty = silence.
    #[serde(default)]
    pub hull_groaning: Vec<String>,
    /// The ship's hull crossing back into good order (content-depth voice round 22): the
    /// positive twin — the seams sealed, the frame riding tight and quiet again after a
    /// hard refit, a hull that feels, for a while, new. No name; indexed by year.
    #[serde(default)]
    pub hull_sound: Vec<String>,
    /// Hull integrity at/above which the ship remarks a sound body (`_high`) or at/below
    /// which it remarks one groaning (`_low`) (content-depth voice round 22).
    #[serde(default)]
    pub hull_voice_high: f32,
    #[serde(default)]
    pub hull_voice_low: f32,
    /// The ship's *air* crossing into a stale band (content-depth voice round 23): the
    /// atmosphere twin of the hull (structure) voice, on `life_support`. When the air
    /// goes close and thick — the scrubbers labouring, a faint reek that never quite
    /// clears, a headache waiting on the lower decks — one of these surfaces. No name;
    /// indexed by year; empty = silence.
    #[serde(default)]
    pub air_stale: Vec<String>,
    /// The ship's air crossing back into good order (content-depth voice round 23): the
    /// positive twin — clean and cool again after a scrubber overhaul, a ship that
    /// breathes easy. No name; indexed by year.
    #[serde(default)]
    pub air_fresh: Vec<String>,
    /// Life-support at/above which the ship remarks clean air (`_high`) or at/below which
    /// it remarks the air gone stale (`_low`) (content-depth voice round 23).
    #[serde(default)]
    pub air_voice_high: f32,
    #[serde(default)]
    pub air_voice_low: f32,
    /// The ship's drive running thin (content-depth voice round 27): the third ship-body
    /// voice, the motion twin of the hull (structure) and air (atmosphere) voices, on
    /// `ship.fuel`. When the tanks run low — the crew husbanding every gram, the drive lit only
    /// when it must be, corrections shaved to the bone — one of these surfaces. No name; indexed
    /// by year; empty = silence.
    #[serde(default)]
    pub drive_thin: Vec<String>,
    /// The ship's drive running full again (content-depth voice round 27): the positive twin —
    /// deep tanks and a free hand on the throttle after a scoop or a resupply. No name; indexed
    /// by year.
    #[serde(default)]
    pub drive_strong: Vec<String>,
    /// Fuel at/above which the ship remarks a full drive (`_high`) or at/below which it remarks
    /// the tanks running thin (`_low`) (content-depth voice round 27).
    #[serde(default)]
    pub fuel_voice_high: f32,
    #[serde(default)]
    pub fuel_voice_low: f32,
    /// The crew *swelling* past its founding complement (content-depth voice round 30): the
    /// growth side of the headcount voice, which no beat or ambient touched — the cradles full,
    /// new decks opened, a people outgrowing the ship the founders launched. No name; indexed by
    /// year; empty = silence.
    #[serde(default)]
    pub crew_swelling: Vec<String>,
    /// The crew *thinning* below its founding complement (content-depth voice round 30): the
    /// quieter twin of the it12 depopulation beat — corridors gone quiet, whole decks closed for
    /// want of anyone to fill them, a shrinking people. No name; indexed by year.
    #[serde(default)]
    pub crew_thinning: Vec<String>,
    /// The ship passing into a new people's hands (content-depth voice round 31): the first voice
    /// keyed to a change in *which faction is dominant* rather than a stat crossing a band. Over
    /// centuries the it11/it13 demographic drift can hand the ship from one majority to another —
    /// the Hearth outgrowing the Ascension, a schism unseating the largest people — and the whole
    /// ship bends to the newcomers' ways (the it10 dilemma odds, the it16 reputation lean, the it21
    /// ambient all key on who runs it), but the turning itself went unremarked. When the dominant
    /// people changes, the decks remark the changing of the guard once. Placeholder `{name}` (the
    /// new ruling people). Indexed by year; empty = the shift passes in silence.
    #[serde(default)]
    pub ruling_people_change: Vec<String>,
    /// Fraction of `starting_population` at/above which the ship remarks a swelling crew
    /// (`_high_ratio`) or at/below which it remarks a thinning one (`_low_ratio`) (content-depth
    /// voice round 30). 0 (`_high_ratio`) disables the crew-size voice.
    #[serde(default)]
    pub crew_size_voice_high_ratio: f32,
    #[serde(default)]
    pub crew_size_voice_low_ratio: f32,
    /// The ship's coffers grown *flush* (content-depth voice round 32): the material-fortune voice,
    /// read against `starting_resources.credits` the way the crew-size voice reads against
    /// `starting_population`. A run of well-paid charters and shrewd trades fills the treasury past
    /// anything the founders budgeted, and the council debates what to build rather than what to
    /// cut — the ease of a ship that can afford its own repairs, the it32 desperation premium a
    /// worry for other captains. No name; indexed by year; empty = wealth passes unremarked.
    #[serde(default)]
    pub treasury_flush: Vec<String>,
    /// The ship's coffers run *bare* (content-depth voice round 32): the low twin — whatever the
    /// ship earned has gone to the dark (repairs, resupply, the price of a hundred running systems),
    /// and the council counts every credit twice. A generation ship can survive poor, but not poor
    /// for long without something breaking that money would have fixed. No name; indexed by year.
    #[serde(default)]
    pub treasury_bare: Vec<String>,
    /// Fraction of `starting_resources.credits` at/above which the ship remarks a flush treasury
    /// (`_high_ratio`) or at/below which it remarks a bare one (`_low_ratio`) (content-depth voice
    /// round 32). 0 (`_high_ratio`) disables the treasury voice.
    #[serde(default)]
    pub treasury_voice_high_ratio: f32,
    #[serde(default)]
    pub treasury_voice_low_ratio: f32,
    /// The ship's reactors running *flush* (content-depth voice round 33): the power-fortune voice,
    /// the sibling of the it32 treasury (money) voice. When the energy store climbs past
    /// `power_voice_high` the reactors are making more than even the it21 fabricators can drink —
    /// everything lit, the drydock's power-hungry work cleared without a second thought, the ship
    /// running warm and bright. No name; indexed by year; empty = a surplus passes unremarked.
    #[serde(default)]
    pub power_flush: Vec<String>,
    /// The ship's grid running *dark* (content-depth voice round 33): the low twin — the energy
    /// store fallen to `power_voice_low`, the it15 life-support plant nearing its power-starvation
    /// line and the it29 factories shedding output, the decks on rationed light and the crew moving
    /// through a ship that hums lower than it should. No name; indexed by year.
    #[serde(default)]
    pub power_starved: Vec<String>,
    /// Energy store at/above which the ship remarks a flush grid (`power_voice_high`) or at/below
    /// which it remarks a dark one (`power_voice_low`) (content-depth voice round 33). Absolute
    /// energy levels (energy has no founding-stake reference the way credits do); set to bracket the
    /// founding stock so a launched ship reads neutral. 0 (`power_voice_high`) disables the voice.
    #[serde(default)]
    pub power_voice_high: i64,
    #[serde(default)]
    pub power_voice_low: i64,
    /// A subsystem patched back toward working order (content-depth voice round 9):
    /// the field-repair verb fires repeatedly across a voyage, so the flat line it
    /// used needs variety. Placeholder `{name}` (the module). Indexed by the month
    /// clock; empty falls back to the built-in line.
    #[serde(default)]
    pub subsystem_repair: Vec<String>,
    /// A new cohort trained up on a subsystem (content-depth voice round 9): the
    /// knowledge-training verb, likewise repeatable. Placeholder `{name}`. Indexed
    /// by the month clock; empty falls back to the built-in line.
    #[serde(default)]
    pub subsystem_training: Vec<String>,
    /// Atmospheric "life aboard" lines surfaced during long event-less stretches
    /// (content-depth voice round 2), so the passing centuries read as lived-in
    /// rather than empty. Dated by the log itself, indexed by year (no RNG).
    #[serde(default)]
    pub ambient: Vec<String>,
    /// Ambient lines for a *far-drifted* ship (content-depth voice round 10): once
    /// cultural drift crosses `ambient_drift_threshold`, the quiet stretches draw
    /// from this pool instead — the same lived-in texture gone alien, so the log
    /// itself reflects how far the people have come from the founders. Empty =
    /// always use `ambient`.
    #[serde(default)]
    pub ambient_drifted: Vec<String>,
    /// Cultural-drift level at or past which quiet stretches read from
    /// `ambient_drifted`. 0 with a non-empty drifted pool means always drifted.
    #[serde(default)]
    pub ambient_drift_threshold: f32,
    /// Ambient lines for a *hollowed-out* ship (content-depth voice round 12): once
    /// the crew has thinned to `ambient_population_threshold` or fewer, the quiet
    /// stretches draw from this pool — the same lived-in texture gone sparse and
    /// echoing, corridors built for thousands walked by hundreds, so the log
    /// reflects how empty the ship has become. Takes precedence over `ambient_drifted`
    /// (emptiness is the louder note in a silence). Empty = always use the others.
    #[serde(default)]
    pub ambient_hollow: Vec<String>,
    /// Crew headcount at or below which quiet stretches read from `ambient_hollow`
    /// (content-depth voice round 12). An absolute count (founding is ~1000).
    #[serde(default)]
    pub ambient_population_threshold: u32,
    /// Ambient lines for a *long-hungry* ship (content-depth voice round 13): once
    /// the food store has sat below the lean line for `ambient_lean_years_threshold`
    /// years or more (`SimState.lean_food_years`), the quiet stretches draw from this
    /// pool — the lived-in texture gone thin and rationed, the daily preoccupation
    /// with the next plate. Takes precedence over `ambient_hollow` (a sustained
    /// hunger is the most immediate lived condition). Empty = always use the others.
    #[serde(default)]
    pub ambient_lean: Vec<String>,
    /// Consecutive lean years at or past which quiet stretches read from
    /// `ambient_lean` (content-depth voice round 13).
    #[serde(default)]
    pub ambient_lean_years_threshold: u32,
    /// Ambient lines for a *long-prosperous* ship (content-depth voice round 14):
    /// the first positive-condition ambient — the mirror of `ambient_lean`. Once the
    /// larder has stood full for `ambient_fat_years_threshold` years
    /// (`SimState.fat_food_years`) *and* no grimmer note holds, the quiet stretches
    /// draw from this pool — the texture of ease and plenty, so a ship's good years
    /// finally *sound* good instead of merely neutral. Lowest priority (a grim ship
    /// reads grim first). Empty = a prosperous ship reads the ordinary ambient.
    #[serde(default)]
    pub ambient_fat: Vec<String>,
    /// Consecutive fat years at or past which quiet stretches read from
    /// `ambient_fat` (content-depth voice round 14).
    #[serde(default)]
    pub ambient_fat_years_threshold: u32,
    /// Years of event-less quiet between ambient lines (0 = ambient off).
    #[serde(default)]
    pub ambient_gap_years: u32,
    /// Phase-transition line pools keyed by phase (snake_case: travel, operation,
    /// return, completion, preparation), content-depth voice round 3. Indexed by
    /// how many times that phase has been entered this voyage, so a double-hop's
    /// second departure/arrival reads differently from the first. An empty or
    /// missing pool falls back to the built-in line.
    #[serde(default)]
    pub phase_lines: HashMap<String, Vec<String>>,
    /// Homecoming prose pools keyed by mission success level (snake_case:
    /// complete, partial, pyrrhic, failure), content-depth voice round 4. The
    /// end of a centuries-long voyage is the campaign's emotional climax; this
    /// gives it level-specific prose instead of one flat mechanical line.
    /// Placeholders `{years}`, `{generation}`. Empty or missing pool falls back
    /// to the built-in line so the log is never blank.
    #[serde(default)]
    pub homecoming: HashMap<String, Vec<String>>,
}
impl FlavorConfig {
    /// Deterministic pick from `pool` by rotating index `n`, with `{name}`
    /// substituted. Returns `None` only when the pool is empty.
    pub fn line_with_name(pool: &[String], n: usize, name: &str) -> Option<String> {
        (!pool.is_empty()).then(|| pool[n % pool.len()].replace("{name}", name))
    }

    /// Like `line_with_name`, additionally substituting `{post}` (the officer's
    /// human post name) — for crew-turnover lines that name both the person and
    /// the post they take or leave. `None` only when the pool is empty.
    pub fn line_with_name_post(
        pool: &[String],
        n: usize,
        name: &str,
        post: &str,
    ) -> Option<String> {
        (!pool.is_empty()).then(|| {
            pool[n % pool.len()]
                .replace("{name}", name)
                .replace("{post}", post)
        })
    }

    /// Homecoming line for a mission that ended at `level_key` (the success
    /// level, snake_case), indexed deterministically by `n` (the generation) so
    /// a seed replays the same line, with `{years}`/`{generation}` substituted.
    /// `None` when no pool is authored for that level — the caller keeps its
    /// built-in line.
    pub fn homecoming_line(
        &self,
        level_key: &str,
        n: usize,
        years: u32,
        generation: u32,
    ) -> Option<String> {
        let pool = self.homecoming.get(level_key)?;
        (!pool.is_empty()).then(|| {
            pool[n % pool.len()]
                .replace("{years}", &years.to_string())
                .replace("{generation}", &generation.to_string())
        })
    }
}
