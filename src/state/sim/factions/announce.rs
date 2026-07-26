//! The peoples' voice: band-crossing narrators that speak once when a
//! political or population meter turns, not every year it sits there.

use crate::data::{FlavorConfig, GameData};
use crate::state::sim::SimState;

use super::{log_name, mood_band_for, stability_voice_band_for};

mod condition;

impl SimState {
    /// Yearly (content-depth voice round 8): give the otherwise-silent approval
    /// meter a voice. When an aboard people crosses *into* restlessness or
    /// contentment — not every year it stays there — surface one pooled line, so
    /// the player feels a faction souring long before its withdrawal beat fires.
    /// Deterministic (indexed by year), no RNG; neutral crossings are silent.
    pub fn announce_faction_moods(&mut self, data: &GameData) {
        let year = self.year();
        let mut lines: Vec<String> = Vec::new();
        for fstate in &mut self.factions {
            if !fstate.is_aboard() {
                continue;
            }
            let band = mood_band_for(fstate.approval);
            if band == fstate.mood_band {
                continue;
            }
            let pool = match band {
                -1 => &data.config.flavor.faction_souring,
                1 => &data.config.flavor.faction_warming,
                // Settling back to neutral is silent, but still remembered so a
                // later re-souring announces afresh.
                _ => {
                    fstate.mood_band = band;
                    continue;
                }
            };
            let name = log_name(&data.factions, &fstate.faction_id);
            let idx = year as usize + fstate.faction_id.len();
            if let Some(line) = FlavorConfig::line_with_name(pool, idx, &name) {
                lines.push(line);
            }
            fstate.mood_band = band;
        }
        for line in lines {
            self.push_log(line);
        }
    }

    /// Give the *ship's* overall morale a voice (content-depth voice round 11):
    /// the collective parallel to `announce_faction_moods`. When the whole crew's
    /// morale crosses *into* a heavy or a light band — not every year it sits
    /// there — surface one pooled ambient line, so the decks going grim or lifting
    /// together says so. Deterministic (indexed by year), no RNG; settling back to
    /// steady is silent but remembered so a later crossing announces afresh.
    pub fn announce_ship_mood(&mut self, data: &GameData) {
        let band = mood_band_for(self.population.morale);
        if band == self.morale_band {
            return;
        }
        let pool = match band {
            -1 => &data.config.flavor.ship_mood_darkening,
            1 => &data.config.flavor.ship_mood_lifting,
            _ => {
                self.morale_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.morale_band = band;
    }

    /// Give the ship's *political climate* a voice (content-depth voice round 15):
    /// distinct from the crew's spirits (`announce_ship_mood`) and from any one
    /// people's mood (`announce_faction_moods`), this is the member-weighted mood of
    /// the aboard peoples as a whole (it100's `aboard_approval_mean`) — how content
    /// the polity is with its treatment. When it crosses *into* broad discontent or
    /// broad ease, surface one pooled line, so a ship's peoples curdling or settling
    /// together says so. Deterministic (indexed by year), no RNG; a return to
    /// neutral is silent but remembered.
    pub fn announce_polity_mood(&mut self, data: &GameData) {
        let band = mood_band_for(self.aboard_approval_mean());
        if band == self.polity_mood_band {
            return;
        }
        let pool = match band {
            -1 => &data.config.flavor.polity_souring,
            1 => &data.config.flavor.polity_warming,
            _ => {
                self.polity_mood_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.polity_mood_band = band;
    }

    /// Remark when the ship passes into a new people's hands (content-depth voice round 31): the
    /// first voice keyed not to a stat crossing a band but to a change in *which faction is
    /// dominant* — the largest aboard, the "who runs the ship" that the it10 dilemma odds, the it16
    /// reputation lean, and the it21 ambient all read. The it11/it13 demographic drift can, over
    /// centuries, hand the ship from one majority to another; when the dominant people differs from
    /// the one last recorded, the decks remark the changing of the guard once (naming the new
    /// ruling people), then this updates. The launch majority is recorded silently in
    /// `new_campaign`; a ship with no aboard people (`dominant_faction_id` None) holds its record
    /// and says nothing. Deterministic (indexed by year), no RNG. It layers over the it11 power-
    /// transition *beat* the way the hull voice layers over the hull beat: the voice is the decks'
    /// immediate remark, the beat the ship's fuller reckoning with new leadership.
    pub fn announce_ruling_people(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.ruling_people_change.is_empty() {
            return;
        }
        let Some(current) = self.dominant_faction_id().map(str::to_owned) else {
            return;
        };
        match &self.ruling_people_voice {
            // Not yet recorded (a save from before this field, or a pre-launch state): adopt the
            // current majority as the baseline without announcing.
            None => self.ruling_people_voice = Some(current),
            Some(prev) if *prev == current => {}
            Some(_) => {
                let name = data
                    .factions
                    .get(&current)
                    .map(|f| f.name.as_str())
                    .unwrap_or("");
                if let Some(line) = FlavorConfig::line_with_name(
                    &fl.ruling_people_change,
                    self.year() as usize,
                    name,
                ) {
                    self.push_log(line);
                }
                self.ruling_people_voice = Some(current);
            }
        }
    }

    /// Give the ship's growing *reputation* a voice (content-depth voice round 16):
    /// the quiet companion to the it109 reputation beat, at a gentler threshold. When
    /// the watched trait crosses *into* a merciful or a feared band, surface one
    /// pooled line — the ship remarking that its name has begun to mean something —
    /// before that name grows defining enough to force the beat's reckoning.
    /// Deterministic (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_reputation_name(&mut self, data: &GameData) {
        let trait_id = &data.config.campaign_skeleton.reputation_beat_trait;
        let fl = &data.config.flavor;
        if trait_id.is_empty() || fl.reputation_voice_high <= 0.0 {
            return;
        }
        let value = self.reputation(trait_id);
        let band = if value >= fl.reputation_voice_high {
            1
        } else if value <= fl.reputation_voice_low {
            -1
        } else {
            0
        };
        if band == self.reputation_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.reputation_merciful,
            -1 => &fl.reputation_feared,
            _ => {
                self.reputation_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.reputation_voice_band = band;
    }

    /// Give the ship's *other* name a voice (content-depth voice round 28): the it16 reputation
    /// voice reads only the one watched trait (mercy — merciful vs feared), but the ship earns a
    /// whole character, and this session made `wonder` a load-bearing trait — a name a ship grows
    /// by chasing marvels (the it28 science family, the it30 first-contact reactions, the it29
    /// charter that pays for it). When that name crosses *into* a famed-for-wonder band (a
    /// chronicle thick with charted impossibilities, a crew who have made a creed of curiosity)
    /// or an incurious one (a ship that keeps its head down and sails past every strangeness),
    /// the decks remark it once. The same shape as the mercy voice, on a different trait.
    /// Deterministic (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_wonder_name(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.wonder_voice_high <= 0.0 {
            return;
        }
        let value = self.reputation("wonder");
        let band = if value >= fl.wonder_voice_high {
            1
        } else if value <= fl.wonder_voice_low {
            -1
        } else {
            0
        };
        if band == self.wonder_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.wonder_famed,
            -1 => &fl.wonder_incurious,
            _ => {
                self.wonder_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.wonder_voice_band = band;
    }

    /// Give the ship's *third* name a voice (content-depth voice round 29): the last of the
    /// built reputation traits without one, completing the mercy (it16) / wonder (it28) / resolve
    /// voice set. `resolve` is what a ship earns by doing the hard thing and not flinching — the
    /// it29 enforcement charter pays for it, the it31 "wear the name" payoff builds it, and the
    /// it18 abandonment mark *costs* it — so a ship acquires a name for steadfastness or, at the
    /// low end, for *folding*. When resolve crosses into a steadfast band (a hull known to see
    /// the grim thing through) or a yielding one (a name for buckling, for the writ quit
    /// half-done), the decks remark it once. Same shape as the mercy voice, on a third trait.
    /// Deterministic (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_resolve_name(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.resolve_voice_high <= 0.0 {
            return;
        }
        let value = self.reputation("resolve");
        let band = if value >= fl.resolve_voice_high {
            1
        } else if value <= fl.resolve_voice_low {
            -1
        } else {
            0
        };
        if band == self.resolve_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.resolve_steadfast,
            -1 => &fl.resolve_yielding,
            _ => {
                self.resolve_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.resolve_voice_band = band;
    }

    /// Give the ship's *institutions* a voice (content-depth voice round 17): the
    /// governance twin of the morale (`announce_ship_mood`) and polity
    /// (`announce_polity_mood`) voices. Distinct from the crew's spirits and from how
    /// content the peoples are, this voices the *machinery of government* — when
    /// `stability` crosses *into* a fraying band (quorums missed, offices unfilled) or
    /// a firm one (the councils working, the charter honored in practice), surface one
    /// pooled line. Gated gentler than the it102 collapse *beat*, so the voice (a
    /// fraying noticed) precedes the reckoning (a government failed). Deterministic
    /// (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_stability_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.stability_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.population.stability,
            fl.stability_voice_high,
            fl.stability_voice_low,
        );
        if band == self.stability_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.stability_firming,
            -1 => &fl.stability_fraying,
            _ => {
                self.stability_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.stability_voice_band = band;
    }

    /// Give the crew's *devotion to the founders' mission* a voice (content-depth voice
    /// round 20): the identity-side twin of the morale (`announce_ship_mood`) and
    /// governance (`announce_stability_mood`) voices, on `legacy_loyalty`. Distinct from
    /// the crew's spirits and from how far the people have *changed* (the it-drift
    /// ambient) — this voices the founders' *purpose* itself waxing or fading: when
    /// loyalty crosses *into* a guttering band (the charter read as a story, the young
    /// unable to feel why the ship flies) or a bright one (the dream taken up afresh, the
    /// mission honored from conviction), surface one pooled line. Announced right after
    /// the year's voyage drift, which erodes loyalty, so the fading of the founders' fire
    /// is narrated as it happens. Deterministic (indexed by year), no RNG; a return to
    /// the middle re-arms.
    pub fn announce_loyalty_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.loyalty_voice_high <= 0.0 {
            return;
        }
        let value = self.population.legacy_loyalty;
        let band = if value >= fl.loyalty_voice_high {
            1
        } else if value <= fl.loyalty_voice_low {
            -1
        } else {
            0
        };
        if band == self.loyalty_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.loyalty_bright,
            -1 => &fl.loyalty_guttering,
            _ => {
                self.loyalty_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.loyalty_voice_band = band;
    }

    /// Give the crew's *cohesion* a voice (content-depth voice round 21): the fourth
    /// internal-state voice, beside the morale (`announce_ship_mood`), governance
    /// (`announce_stability_mood`), and mission-devotion (`announce_loyalty_mood`) ones,
    /// on `unity`. Distinct from all three — a crew can be high-spirited, well-governed,
    /// and sure of its purpose yet quietly *splintering* into cliques, one people
    /// becoming several. When unity crosses *into* a fraying band (the ship coming apart
    /// into wary factions) or a cohering one (the crew pulling back together as one), the
    /// decks remark it once. Distinct too from the it102 unity-*collapse* beat, which is
    /// the reckoning; this is the quieter thing noticed before and after it. Deterministic
    /// (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_unity_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.unity_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.population.unity,
            fl.unity_voice_high,
            fl.unity_voice_low,
        );
        if band == self.unity_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.unity_cohering,
            -1 => &fl.unity_fraying,
            _ => {
                self.unity_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.unity_voice_band = band;
    }

    /// Give the crew's *physiological* identity a voice (content-depth voice round 25):
    /// the bodily companion to the loyalty voice, on `adaptation`. When the descendants'
    /// bodies cross *into* a shipborn band (longer, leaner, fitted to the ship and no
    /// longer to a world) or a baseline one (held human by a well-kept infirmary, it25),
    /// the decks remark it once — the crew becoming, or refusing to become, a new kind of
    /// people in the flesh, distinct from the it167 loyalty voice (their belief) and the
    /// drift-aware ambient (their culture). Deterministic (indexed by year), no RNG; a
    /// return to the middle re-arms.
    pub fn announce_adaptation_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.adaptation_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.population.adaptation,
            fl.adaptation_voice_high,
            fl.adaptation_voice_low,
        );
        if band == self.adaptation_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.crew_shipborn,
            -1 => &fl.crew_baseline,
            _ => {
                self.adaptation_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.adaptation_voice_band = band;
    }

    /// Give the crew's *cultural* identity a voice (content-depth voice round 26): the
    /// cultural companion to the adaptation voice (their bodies), on `cultural_drift`. Where
    /// that reads how far the descendants' *bodies* have left the founders' stock, this reads
    /// how far their *customs* have — the calendars, festivals, and tongue drifting into
    /// something the founders would not know. When drift crosses *into* a new-people band (a
    /// culture the founders would not recognise) or a founders-kept one (the old ways held
    /// close, the rarer crossing a strong archive earns), the decks remark it once. Distinct
    /// from the it2 drift *beats* (the reckoning) and the drift-aware *ambient* (the dominant
    /// people's background flavor); this is the quiet identity crossing, said once.
    /// Deterministic (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_drift_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.drift_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.population.cultural_drift,
            fl.drift_voice_high,
            fl.drift_voice_low,
        );
        if band == self.drift_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.culture_newfound,
            -1 => &fl.culture_founders_kept,
            _ => {
                self.drift_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.drift_voice_band = band;
    }
}
