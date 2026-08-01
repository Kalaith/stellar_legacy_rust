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

/// One captaincy: a name that held the first chair, and the years it held it.
/// Kept because neither of the other records can answer "who commanded this
/// voyage" a century on — `Dynasty::members` holds only the living, and the
/// ship's log is trimmed to `log_limit`. The homecoming debrief reads this to
/// name every commander a mission passed through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reign {
    pub name: String,
    /// Campaign year the captain took the chair.
    pub began_year: u32,
    /// Campaign year the chair passed on; `None` while they still hold it.
    pub ended_year: Option<u32>,
    /// Dynasty generation the captain belonged to.
    pub generation: u32,
    /// Leadership score at the moment they took the chair.
    pub leadership: u32,
    pub trait_name: String,
}

impl Reign {
    /// Whole years the captain held the chair, counted to `now` while the reign
    /// is still open.
    pub fn years_held(&self, now: u32) -> u32 {
        self.ended_year
            .unwrap_or(now)
            .saturating_sub(self.began_year)
    }

    /// True if this captaincy overlapped the window `[from, to]` — the test the
    /// debrief uses to pick out the commanders a single voyage passed through.
    pub fn overlaps(&self, from: u32, to: u32) -> bool {
        self.began_year <= to && self.ended_year.unwrap_or(u32::MAX) >= from
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dynasty {
    pub generation: u32,
    pub years_since_generation: u32,
    pub next_member_id: u32,
    pub members: Vec<DynastyMember>,
    /// Every captaincy in order, oldest first; the last is the sitting one while
    /// its `ended_year` is `None`. Empty on a save written before the roster
    /// existed — the debrief degrades to naming only the captain who docked.
    #[serde(default)]
    pub reigns: Vec<Reign>,
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

    /// Open a reign for whoever now sits in the first chair. No-op when the seat
    /// is empty, so an extinct line does not record a phantom captain.
    pub fn begin_reign(&mut self, year: u32) {
        let Some(leader) = self.leader() else {
            return;
        };
        let reign = Reign {
            name: leader.name.clone(),
            began_year: year,
            ended_year: None,
            generation: self.generation,
            leadership: leader.leadership,
            trait_name: leader.trait_name.clone(),
        };
        self.reigns.push(reign);
    }

    /// Close the open reign, if one is open. Idempotent — closing twice keeps
    /// the first end year, so a handoff and an extinction in the same year do
    /// not overwrite each other.
    pub fn end_reign(&mut self, year: u32) {
        if let Some(open) = self.reigns.last_mut() {
            if open.ended_year.is_none() {
                open.ended_year = Some(year.max(open.began_year));
            }
        }
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
        reigns: Vec::new(),
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
    // The founding captain opens the roster at year 0, so the first voyage's
    // debrief can name them alongside every successor.
    dynasty.begin_reign(0);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dynasty_with_leader(name: &str) -> Dynasty {
        Dynasty {
            generation: 1,
            years_since_generation: 0,
            next_member_id: 1,
            members: vec![DynastyMember {
                id: 0,
                name: name.to_owned(),
                age: 45,
                leadership: 70,
                specialization: "Command".to_owned(),
                trait_name: "Steady".to_owned(),
                is_leader: true,
            }],
            reigns: Vec::new(),
            designated_heir: None,
            births_this_generation: 0,
            leader_reign_years: 0,
            long_reign_marked: false,
            dynasty_crisis_marked: false,
            extinct: false,
        }
    }

    #[test]
    fn a_reign_opens_on_the_sitting_captain_and_closes_where_it_ended() {
        let mut dynasty = dynasty_with_leader("Boro Chartwright");
        dynasty.begin_reign(12);
        assert_eq!(dynasty.reigns.len(), 1);
        let open = &dynasty.reigns[0];
        assert_eq!(open.name, "Boro Chartwright");
        assert_eq!(open.began_year, 12);
        assert_eq!(open.ended_year, None, "a sitting captain has no end year");
        // An open reign is counted up to the present; a closed one to its end.
        assert_eq!(open.years_held(40), 28);
        dynasty.end_reign(40);
        assert_eq!(dynasty.reigns[0].ended_year, Some(40));
        assert_eq!(
            dynasty.reigns[0].years_held(90),
            28,
            "a closed reign is fixed"
        );
    }

    #[test]
    fn closing_a_reign_twice_keeps_the_first_end_year() {
        // A handoff and an extinction can land in the same tick; the second
        // close must not overwrite the year the chair actually passed on.
        let mut dynasty = dynasty_with_leader("Ilsa Vance");
        dynasty.begin_reign(5);
        dynasty.end_reign(30);
        dynasty.end_reign(31);
        assert_eq!(dynasty.reigns[0].ended_year, Some(30));
    }

    #[test]
    fn an_empty_chair_records_no_captain() {
        let mut dynasty = dynasty_with_leader("Ilsa Vance");
        dynasty.members.clear();
        dynasty.begin_reign(9);
        assert!(
            dynasty.reigns.is_empty(),
            "an extinct line records no phantom captaincy"
        );
    }

    #[test]
    fn overlap_picks_out_the_captains_a_voyage_passed_through() {
        // The debrief asks the roster which captains held the chair between the
        // launch year and the homecoming year — including one still sitting.
        let closed = |began, ended| Reign {
            name: "x".to_owned(),
            began_year: began,
            ended_year: Some(ended),
            generation: 1,
            leadership: 50,
            trait_name: String::new(),
        };
        // Voyage runs years 20..60.
        assert!(!closed(0, 19).overlaps(20, 60), "ended before the launch");
        assert!(closed(0, 25).overlaps(20, 60), "sat at the launch");
        assert!(closed(30, 40).overlaps(20, 60), "sat wholly within");
        assert!(
            closed(55, 80).overlaps(20, 60),
            "took the chair before docking"
        );
        assert!(!closed(61, 80).overlaps(20, 60), "took it after docking");
        let mut open = closed(55, 80);
        open.ended_year = None;
        assert!(open.overlaps(20, 60), "a sitting captain is aboard");
    }
}
