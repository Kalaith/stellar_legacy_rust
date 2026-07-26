//! The ship's own voice: band-crossing narrators for the hull, the air, the
//! drive, and the stores that keep the people alive.

use crate::data::{FlavorConfig, GameData};
use crate::state::sim::SimState;

use super::stability_voice_band_for;

impl SimState {
    /// Give the ship's *own body* a voice (content-depth voice round 22): the first that
    /// speaks for the vessel rather than the crew. Where the morale/unity/stability/
    /// loyalty voices read the *people*, this reads the aging machine that carries them —
    /// when `hull_integrity` crosses *into* a groaning band (the plates weeping at the
    /// seams, the frame complaining on every burn) or a sound one (riding tight and true
    /// again after a refit), the decks remark it once. Deterministic (indexed by year),
    /// no RNG; a return to the middle re-arms.
    pub fn announce_hull_condition(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.hull_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(
            self.ship.hull_integrity,
            fl.hull_voice_high,
            fl.hull_voice_low,
        );
        if band == self.hull_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.hull_sound,
            -1 => &fl.hull_groaning,
            _ => {
                self.hull_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.hull_voice_band = band;
    }

    /// Give the ship's *air* a voice (content-depth voice round 23): the second ship-body
    /// voice, the atmosphere twin of the it22 hull (structure) voice, on `life_support`.
    /// When the air crosses *into* a stale band (close and thick, the scrubbers labouring)
    /// or a fresh one (clean and cool again after an overhaul), the decks remark it once.
    /// Deterministic (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_air_condition(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.air_voice_high <= 0.0 {
            return;
        }
        let band =
            stability_voice_band_for(self.ship.life_support, fl.air_voice_high, fl.air_voice_low);
        if band == self.air_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.air_fresh,
            -1 => &fl.air_stale,
            _ => {
                self.air_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.air_voice_band = band;
    }

    /// Give the ship's *drive* a voice (content-depth voice round 27): the third ship-body
    /// voice, the motion twin of the it22 hull (structure) and it23 air (atmosphere) voices, on
    /// `ship.fuel`. When the tanks cross *into* a thin band (running on fumes, the crew
    /// husbanding every gram, the drive lit only when it must be) or a full one (deep tanks and
    /// a free hand on the throttle again after a scoop or a resupply), the decks remark it once.
    /// Completes the ship-body voice trio to match the it hull/air/becalmed crisis-beat trio —
    /// the drive murmurs as it thins, the it25 becalmed beat reckons when it is truly stranded.
    /// Deterministic (indexed by year), no RNG; a return to the middle re-arms.
    pub fn announce_drive_condition(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.fuel_voice_high <= 0.0 {
            return;
        }
        let band = stability_voice_band_for(self.ship.fuel, fl.fuel_voice_high, fl.fuel_voice_low);
        if band == self.fuel_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.drive_strong,
            -1 => &fl.drive_thin,
            _ => {
                self.fuel_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.fuel_voice_band = band;
    }

    /// Give the ship's *headcount* a voice (content-depth voice round 30): the crew growing or
    /// dwindling, read against the founding complement (`starting_population`). The it12
    /// depopulation *beat* reckons when the crew crashes and the hollow ambient colours a
    /// depleted ship, but the crossing itself — and the *growth* side entirely — went unremarked.
    /// When the crew crosses *into* a swelling band (the cradles full, new decks opened, a people
    /// expanding) or a thinning one (corridors gone quiet, whole decks closed, a shrinking
    /// people), the decks remark it once. The launch band (a ship at its founding complement) is
    /// recorded not announced; a return to the middle re-arms. Deterministic (indexed by year),
    /// no RNG.
    pub fn announce_crew_size_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        let starting = data.config.starting_population;
        if fl.crew_size_voice_high_ratio <= 0.0 || starting == 0 {
            return;
        }
        let ratio = self.population.count as f32 / starting as f32;
        let band = if ratio >= fl.crew_size_voice_high_ratio {
            1
        } else if ratio <= fl.crew_size_voice_low_ratio {
            -1
        } else {
            0
        };
        if band == self.crew_size_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.crew_swelling,
            -1 => &fl.crew_thinning,
            _ => {
                self.crew_size_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.crew_size_voice_band = band;
    }

    /// Remark when the ship's coffers cross into flush or bare (content-depth voice round 32): the
    /// material-fortune voice, read against `starting_resources.credits` the way the it30 crew-size
    /// voice reads against `starting_population`. When the treasury crosses *into* a flush band (a
    /// run of well-paid charters, the council debating what to build) or a bare one (every credit
    /// counted twice, requisitions stalled), the ledger's turning is remarked once. The launch band
    /// (a ship at its founding stake, ratio 1.0) is recorded not announced; a return to the middle
    /// re-arms. Deterministic (indexed by year), no RNG.
    pub fn announce_treasury_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        let starting = data.config.starting_resources.credits;
        if fl.treasury_voice_high_ratio <= 0.0 || starting <= 0 {
            return;
        }
        let ratio = self.resources.credits as f32 / starting as f32;
        let band = if ratio >= fl.treasury_voice_high_ratio {
            1
        } else if ratio <= fl.treasury_voice_low_ratio {
            -1
        } else {
            0
        };
        if band == self.treasury_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.treasury_flush,
            -1 => &fl.treasury_bare,
            _ => {
                self.treasury_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.treasury_voice_band = band;
    }

    /// Remark when the ship's power crosses into flush or dark (content-depth voice round 33): the
    /// power-fortune voice, the sibling of the it32 treasury (money) voice, read against absolute
    /// energy lines (energy has no founding-stake reference the way credits do). When the reactors
    /// cross *into* a flush band (energy past `power_voice_high`, more than even the it21 fabricators
    /// can drink) or a dark one (fallen to `power_voice_low`, the it15 plant and it29 factories
    /// starving), the ship's power fortune is remarked once. The launch band (a ship at its founding
    /// stock, bracketed between the lines) is recorded not announced; a return to the middle re-arms.
    /// Deterministic (indexed by year), no RNG.
    pub fn announce_power_mood(&mut self, data: &GameData) {
        let fl = &data.config.flavor;
        if fl.power_voice_high <= 0 {
            return;
        }
        let energy = self.resources.energy;
        let band = if energy >= fl.power_voice_high {
            1
        } else if energy <= fl.power_voice_low {
            -1
        } else {
            0
        };
        if band == self.power_voice_band {
            return;
        }
        let pool = match band {
            1 => &fl.power_flush,
            -1 => &fl.power_starved,
            _ => {
                self.power_voice_band = band;
                return;
            }
        };
        if let Some(line) = FlavorConfig::line_with_name(pool, self.year() as usize, "") {
            self.push_log(line);
        }
        self.power_voice_band = band;
    }
}
