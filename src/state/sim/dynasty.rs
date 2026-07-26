//! The founding line and the crew it seeds: who is aboard, who leads, and
//! how a new name is drawn when the old one dies.

use crate::data::GameData;
use macroquad_toolkit::rng::SeededRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynastyMember {
    pub id: u32,
    pub name: String,
    pub age: u32,
    /// 0-100 leadership skill; drives heir selection (GDD §5.3).
    pub leadership: u32,
    pub specialization: String,
    pub trait_name: String,
    pub is_leader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dynasty {
    pub generation: u32,
    pub years_since_generation: u32,
    pub next_member_id: u32,
    pub members: Vec<DynastyMember>,
    /// Council-designated successor (GDD §4 Select Heir). Honored at the
    /// next succession if still living and age-eligible.
    #[serde(default)]
    pub designated_heir: Option<u32>,
    /// Young adults who have come of age since the last generational beat
    /// (real-time loop follow-up): births are yearly now, but the coming-of-age
    /// line is still logged once a generation, reporting this running tally.
    #[serde(default)]
    pub births_this_generation: u32,
    /// Years the current leader has held the first chair (content-depth campaign
    /// skeleton round 19): counted up each Founding Day while a leader sits, reset
    /// to 0 on every succession. Drives the long-reign beat — now that continuous
    /// mortality takes most leaders within a few decades, an enduring one is a rare,
    /// era-defining thing worth a reckoning.
    #[serde(default)]
    pub leader_reign_years: u32,
    /// Whether the long-reign beat has already fired for the *current* reign
    /// (content-depth campaign skeleton round 19): set when it fires, cleared on
    /// the next succession, so one enduring captaincy is marked once.
    #[serde(default)]
    pub long_reign_marked: bool,
    /// Whether the dynasty-crisis beat has fired for the *current* near-extinction
    /// (content-depth campaign skeleton round 20): set when the founding line dwindles
    /// to the crisis size and a beat marks the ship staring at its own end, cleared
    /// once the line is fully restored — so one brush with extinction is marked once.
    #[serde(default)]
    pub dynasty_crisis_marked: bool,
    /// Set when a generation tick finds no leader and no eligible heir.
    pub extinct: bool,
}

impl Dynasty {
    pub fn leader(&self) -> Option<&DynastyMember> {
        self.members.iter().find(|m| m.is_leader)
    }
}

/// One serving officer holding a ship post (GDD §4 Recruit/Train). At most
/// one crew member per archetype post; posts fall vacant on retirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewMember {
    pub id: u32,
    pub name: String,
    pub archetype_id: String,
    pub age: u32,
    /// 0-100, capped by the archetype's skill_max.
    pub skill: u32,
}

/// Per-legacy tracked inputs to the failure-risk formula (GDD §5.5). These
/// were hardcoded placeholders in the original web build; here they are real
/// state updated by dilemmas and events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyTrack {
    pub legacy_id: String,
    pub tradition_points: i32,
    pub body_horror_events: u32,
    pub existential_dread: f32,
    pub piracy_reputation: f32,
}

/// Generate the founding dynasty: one leader in their prime plus a spread of
/// relatives, named from the legacy's pools.
pub(crate) fn founding_dynasty(data: &GameData, legacy_id: &str, rng: &mut SeededRng) -> Dynasty {
    let mut dynasty = Dynasty {
        generation: 1,
        years_since_generation: 0,
        next_member_id: 0,
        members: Vec::new(),
        designated_heir: None,
        births_this_generation: 0,
        leader_reign_years: 0,
        long_reign_marked: false,
        dynasty_crisis_marked: false,
        extinct: false,
    };

    let ages = [45u32, 38, 33, 22, 17];
    for (i, &age) in ages.iter().enumerate() {
        let mut member = generate_member(data, legacy_id, age, rng, &mut dynasty.next_member_id);
        member.is_leader = i == 0;
        dynasty.members.push(member);
    }
    dynasty
}

pub fn generate_member(
    data: &GameData,
    legacy_id: &str,
    age: u32,
    rng: &mut SeededRng,
    next_id: &mut u32,
) -> DynastyMember {
    let pools = &data.dynasty_names;
    let given = pick(&pools.given_names, rng);
    let surname = pools
        .surnames_by_legacy
        .get(legacy_id)
        .map(|names| pick(names, rng))
        .unwrap_or_else(|| "Voyager".to_owned());
    let specialization = pick(&pools.specializations, rng);
    let trait_name = pools
        .traits_by_legacy
        .get(legacy_id)
        .map(|traits| pick(traits, rng))
        .unwrap_or_default();

    let id = *next_id;
    *next_id += 1;

    DynastyMember {
        id,
        name: format!("{given} {surname}"),
        age,
        leadership: 30 + rng.below(51) as u32,
        specialization,
        trait_name,
        is_leader: false,
    }
}

/// Generate a named officer for a post, skill rolled within the archetype's
/// range. Returns None for an unknown archetype id.
pub fn generate_crew_member(
    data: &GameData,
    legacy_id: &str,
    archetype_id: &str,
    age: u32,
    rng: &mut SeededRng,
    next_id: &mut u32,
) -> Option<CrewMember> {
    let archetype = data.crew_archetypes.iter().find(|a| a.id == archetype_id)?;
    let pools = &data.dynasty_names;
    let given = pick(&pools.given_names, rng);
    let surname = pools
        .surnames_by_legacy
        .get(legacy_id)
        .map(|names| pick(names, rng))
        .unwrap_or_else(|| "Voyager".to_owned());
    let skill_span = (archetype.skill_max - archetype.skill_min + 1) as usize;
    let skill = archetype.skill_min + rng.below(skill_span) as u32;

    let id = *next_id;
    *next_id += 1;
    Some(CrewMember {
        id,
        name: format!("{given} {surname}"),
        archetype_id: archetype.id.clone(),
        age,
        skill,
    })
}

/// Comma-join names with a trailing "and" for the founding log line (W7).
pub(crate) fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

pub(crate) fn pick(pool: &[String], rng: &mut SeededRng) -> String {
    rng.choose(pool).cloned().unwrap_or_default()
}
