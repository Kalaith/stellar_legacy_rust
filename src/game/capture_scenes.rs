//! Deterministic scene seeding for the headless screenshot harness. Split out
//! of `game.rs` (which owns the state machine); `begin_capture_scene` maps a
//! scene name to a fully-composed `GameState` so a capture photographs exactly
//! the state we want, never a mid-animation frame.

use super::Game;
use crate::simulation::{contract, tick};
use crate::state::{GameplayState, MenuState, Screen, SimState};
use crate::ui;
use macroquad_toolkit::achievements::Achievements;

impl Game {
    /// Seed a deterministic state for the headless screenshot harness.
    pub fn begin_capture_scene(&mut self, scene: &str) {
        // Screenshots want the final composed frame, not a mid-type one, and
        // never the boot log. Force canonical amber display so captures are
        // deterministic regardless of any persisted preference.
        self.instant_reveal = true;
        self.capture_run_secs = None;
        self.boot.finish();
        self.display = crate::settings::DisplaySettings::default();
        self.crt_style = self.display.crt_style();
        ui::term::set_phosphor(self.display.phosphor);
        self.delegation_defaults = crate::state::sim::DelegationSettings::default();
        match scene {
            "menu" => self.state = crate::state::GameState::Menu(MenuState::new(false)),
            "green" => {
                // Same menu on the green (P1) tube, to verify the recolor.
                self.display.phosphor = crate::settings::Phosphor::Green;
                self.crt_style = self.display.crt_style();
                ui::term::set_phosphor(self.display.phosphor);
                self.state = crate::state::GameState::Menu(MenuState::new(true));
            }
            "settings" => {
                // Delegate one category so the capture shows both toggle states.
                self.delegation_defaults.mission_milestone = true;
                self.state = crate::state::GameState::Menu(MenuState::new(true));
                self.settings_open = true;
            }
            "help" => {
                self.state = crate::state::GameState::Menu(MenuState::new(true));
                self.help_open = true;
            }
            "heritage" => {
                // Seed a storied Chronicle so the menu heritage line shows.
                for i in 0..6 {
                    self.chronicle.record(crate::chronicle::ChronicleEntry {
                        completed_year: 60,
                        contract_name: "Founding Charter: Meridian Reach".to_owned(),
                        objective: "Colonization".to_owned(),
                        legacy_id: "preservers".to_owned(),
                        leader_name: "Boro Chartwright".to_owned(),
                        generation: i + 1,
                        score: 0.95,
                        outcome: "Complete".to_owned(),
                        duration_years: 60,
                    });
                }
                self.state = crate::state::GameState::Menu(MenuState::new(true));
            }
            "boot" => {
                // Freeze the boot log mid-stream for a screenshot.
                self.boot.seek(1.4);
                self.state = crate::state::GameState::Menu(MenuState::new(false));
            }
            "log" => {
                // Dashboard with the newest log line frozen mid-stream
                // (cursor-visible phase).
                self.capture_log_reveal = Some(0.5);
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                if let Some(template) = self.data.contracts.get("founding_colony") {
                    sim.contract = Some(contract::start_contract(template, &sim));
                }
                self.state = crate::state::GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "event" => {
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                sim.pending_event = Some(crate::state::sim::PendingEvent {
                    template_id: "cultural_schism".to_owned(),
                    rolled_month_clock: 0,
                });
                self.state = crate::state::GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "crew" => {
                let sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::CrewDynasty;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "ship" => {
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                // Seed a salvage hold so the SALVAGE HOLD strip shows (M4.4).
                sim.ship.salvage = vec!["mass_driver".to_owned(), "solar_sail".to_owned()];
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::ShipBuilder;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "subsystems" => {
                // The subsystems screen (W5) with mixed tiers, worn condition,
                // and a knowledge stat dipping below a repair threshold.
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                if let Some(s) = sim.subsystems.get_mut("medical_bay") {
                    s.tier = 2;
                    s.condition = 0.44;
                    s.knowledge = 0.22;
                }
                if let Some(s) = sim.subsystems.get_mut("engineering_bay") {
                    s.tier = 1;
                    s.condition = 0.71;
                }
                if let Some(s) = sim.subsystems.get_mut("agriculture") {
                    s.tier = 3;
                }
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::Subsystems;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "market" => {
                let sim = SimState::new_campaign(
                    &self.data,
                    "wanderers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::Market;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "contracts" => {
                // No active contract, so the available-charters list is shown.
                let sim = SimState::new_campaign(
                    &self.data,
                    "wanderers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::Contract;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "prep" => {
                // A charter under consideration in port (W4): the PREP screen,
                // with deliberately mixed provisioning so shortfalls show red.
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "wanderers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                sim.selected_charter = Some("deep_vein_survey".to_owned());
                sim.ship.fuel = 0.6;
                sim.resources.food = 800;
                sim.ship.spare_parts = 45;
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::Contract;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "drydock" => {
                // Home from a mission (M4.6): no active contract, a worn ship,
                // and a concluded charter in the Chronicle → the Homecoming banner.
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "wanderers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                sim.ship.hull_integrity = 0.46;
                sim.ship.life_support = 0.58;
                sim.ship.spare_parts = 3;
                self.chronicle.record(crate::chronicle::ChronicleEntry {
                    completed_year: 41,
                    contract_name: "Deep Vein Survey: Karst Belt".to_owned(),
                    objective: "Mining".to_owned(),
                    legacy_id: "wanderers".to_owned(),
                    leader_name: "Sella Voss".to_owned(),
                    generation: 2,
                    score: 0.82,
                    outcome: "Partial".to_owned(),
                    duration_years: 40,
                });
                self.capture_run_secs = Some(2280.0); // 38m — the run just flown
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::Contract;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "contract_active" => {
                // A charter a dozen years in, to show progress + drive assist.
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "adaptors",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                if let Some(template) = self.data.contracts.get("deep_vein_survey") {
                    sim.contract = Some(contract::start_contract(template, &sim));
                }
                sim.resources.food = 1_000_000;
                for _ in 0..12 {
                    sim.pending_event = None;
                    sim.pending_dilemma = None;
                    tick::advance_year(&mut sim, &self.data);
                }
                sim.pending_event = None;
                sim.pending_dilemma = None;
                self.capture_run_secs = Some(1140.0); // 19m into the run (live timer)
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::Contract;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "dilemma" => {
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                sim.pending_dilemma = Some(crate::state::sim::PendingDilemma {
                    dilemma_id: "archive_purge".to_owned(),
                    rolled_month_clock: 0,
                });
                self.state = crate::state::GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "dilemma_combat" => {
                // Wanderer convoy raid with a weapon installed — combat lifts
                // the shown odds.
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "wanderers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                sim.ship.weapon = Some("mass_driver".to_owned());
                sim.pending_dilemma = Some(crate::state::sim::PendingDilemma {
                    dilemma_id: "convoy_raid".to_owned(),
                    rolled_month_clock: 0,
                });
                self.state = crate::state::GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            "chronicle" => {
                // Seed a storied Chronicle and unlock the matching milestones.
                self.achievements =
                    Achievements::from_definitions(crate::achievements::definitions());
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                sim.dynasty.generation = 5;
                sim.month_clock = 120 * 12;
                for i in 0..5 {
                    self.chronicle.record(crate::chronicle::ChronicleEntry {
                        completed_year: 40 + i * 20,
                        contract_name: "Deep Vein Survey: Karst Belt".to_owned(),
                        objective: "Mining".to_owned(),
                        legacy_id: "preservers".to_owned(),
                        leader_name: "Boro Chartwright".to_owned(),
                        generation: i + 1,
                        score: 0.92,
                        outcome: if i % 2 == 0 { "Complete" } else { "Partial" }.to_owned(),
                        duration_years: 40,
                    });
                }
                for id in crate::achievements::evaluate(&sim, &self.chronicle) {
                    self.achievements.unlock(id);
                }
                let mut gameplay = GameplayState::new(sim);
                gameplay.screen = Screen::Chronicle;
                self.state = crate::state::GameState::Gameplay(Box::new(gameplay));
            }
            "gameover" => {
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                sim.month_clock = 148 * 12;
                sim.dynasty.generation = 6;
                sim.legacy.tradition_points = 210;
                sim.dynasty.extinct = true;
                self.state = crate::state::GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
            // "gameplay" and anything else: a fresh campaign on the dashboard.
            _ => {
                let mut sim = SimState::new_campaign(
                    &self.data,
                    "preservers",
                    0xC0FFEE,
                    &crate::state::sim::founding_faction_ids(&self.data),
                );
                if let Some(template) = self.data.contracts.get("founding_colony") {
                    sim.contract = Some(contract::start_contract(template, &sim));
                }
                self.state = crate::state::GameState::Gameplay(Box::new(GameplayState::new(sim)));
            }
        }
    }
}
