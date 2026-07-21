//! UiAction dispatch: the pure-view UI returns intents (`UiAction`); this
//! module interprets each one against the sim, applies persistence side
//! effects, and surfaces notifications (CODE_STANDARDS §7). Split out of
//! `game.rs` so the state-machine core stays lean.

use super::Game;
use crate::chronicle::ChronicleEntry;
use crate::data::ship_components::ComponentKind;
use crate::save;
use crate::simulation::{contract, crew, event_resolver, legacy, market, tick};
use crate::state::{GameState, MenuState, StateTransition};
use crate::ui::UiAction;
use macroquad::prelude::get_time;
use macroquad_toolkit::persistence::delete_slot;
use macroquad_toolkit::rng;

impl Game {
    pub(super) fn apply_action(&mut self, action: UiAction) -> Option<StateTransition> {
        match action {
            // ---- Menu ----
            UiAction::SelectLegacy(index) => {
                if let GameState::Menu(menu) = &mut self.state {
                    menu.selected_legacy = index.min(self.legacy_ids.len().saturating_sub(1));
                }
                None
            }
            UiAction::StartNewGame => {
                let legacy_id = match &self.state {
                    GameState::Menu(menu) => self
                        .legacy_ids
                        .get(menu.selected_legacy)
                        .cloned()
                        .unwrap_or_else(|| "preservers".to_owned()),
                    _ => "preservers".to_owned(),
                };
                // Seed is random per campaign; determinism holds *within* a
                // campaign because the seed is stored in the save (GDD §5.6).
                Some(StateTransition::NewCampaign {
                    legacy_id,
                    seed: rng::random_u64(),
                })
            }
            UiAction::ContinueGame => Some(StateTransition::LoadCampaign),
            UiAction::DeleteSave => {
                match delete_slot(&self.data.config.game_name, &self.data.config.save_slot) {
                    Ok(()) => self.notifications.info("Save slot cleared."),
                    Err(err) => self.notifications.danger(format!("Delete failed: {err}")),
                }
                if let GameState::Menu(menu) = &mut self.state {
                    menu.save_exists = save::save_exists(&self.data.config);
                }
                None
            }

            // ---- Global ----
            UiAction::SaveGame => {
                if let GameState::Gameplay(gameplay) = &self.state {
                    match save::save_campaign(&self.data.config, &gameplay.sim) {
                        Ok(()) => self.notifications.success("Voyage saved."),
                        Err(err) => self.notifications.danger(format!("Save failed: {err}")),
                    }
                }
                None
            }
            UiAction::ToMenu => Some(StateTransition::ToMenu),
            UiAction::RetireVoyage => {
                // Clear the dead campaign so it can't be resumed (no autosave),
                // then return to the menu. The Chronicle persists separately.
                if let Err(err) =
                    delete_slot(&self.data.config.game_name, &self.data.config.save_slot)
                {
                    self.notifications
                        .warning(format!("Save clear failed: {err}"));
                }
                self.state = GameState::Menu(MenuState::new(save::save_exists(&self.data.config)));
                self.mission_started = None;
                self.last_mission_real_secs = None;
                self.notifications
                    .info("Voyage retired. The Chronicle remembers.");
                None
            }
            UiAction::SelectScreen(screen) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    gameplay.screen = screen;
                }
                None
            }

            // ---- Gameplay ----
            UiAction::Advance => {
                self.advance();
                self.check_achievements();
                None
            }
            UiAction::SetSpeed(step) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    gameplay.sim.speed = step;
                }
                None
            }
            UiAction::AbortMission => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    let sim = &mut gameplay.sim;
                    // The council turns the ship for home; pay will be prorated
                    // to whatever objective progress was banked (W2).
                    if contract::jump_to_return(sim) {
                        sim.push_log("The council votes to turn back.");
                        self.notifications
                            .warning("Turning for home — pay will be prorated.");
                    }
                }
                None
            }
            UiAction::ResolveEvent(index) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    let sim = &mut gameplay.sim;
                    let template = sim
                        .pending_event
                        .as_ref()
                        .and_then(|p| self.data.events.get(&p.template_id))
                        .cloned();
                    if let Some(template) = template {
                        event_resolver::apply_outcome(sim, &template, index);
                    } else {
                        sim.pending_event = None;
                    }
                }
                None
            }
            UiAction::ResolveDilemma(index) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    legacy::resolve_dilemma(&mut gameplay.sim, &self.data, index);
                }
                None
            }
            UiAction::RecruitCrew(archetype_id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crew::recruit(&mut gameplay.sim, &self.data, &archetype_id) {
                        Ok(name) => self.notifications.success(format!("{name} signed on.")),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::TrainCrew(archetype_id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crew::train(&mut gameplay.sim, &self.data, &archetype_id) {
                        Ok(name) => self
                            .notifications
                            .success(format!("{name} completed training.")),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::SelectHeir(member_id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    let sim = &mut gameplay.sim;
                    let name = sim
                        .dynasty
                        .members
                        .iter()
                        .find(|m| m.id == member_id && !m.is_leader)
                        .map(|m| m.name.clone());
                    if let Some(name) = name {
                        sim.dynasty.designated_heir = Some(member_id);
                        sim.push_log(format!("The council named {name} heir designate."));
                        self.notifications.success(format!("{name} named heir."));
                    }
                }
                None
            }
            UiAction::AcceptContract(id) => {
                let mut accepted = false;
                // Charter tiering (PLAN M4.8): richer charters need renown.
                let renown = crate::heritage::renown(&self.chronicle);
                if let (GameState::Gameplay(gameplay), Some(template)) =
                    (&mut self.state, self.data.contracts.get(&id))
                {
                    let sim = &mut gameplay.sim;
                    if sim.contract.is_none() && renown >= template.min_renown {
                        sim.contract = Some(contract::start_contract(template, sim));
                        sim.push_log(format!("Charter accepted: {}", template.name));
                        accepted = true;
                    }
                }
                if accepted {
                    // Start the cosmetic run timer for this mission (PLAN M4.7).
                    self.mission_started = Some(get_time());
                    self.last_mission_real_secs = None;
                    self.notifications.success("Charter accepted.");
                }
                None
            }
            UiAction::PurchaseComponent(kind, id) => {
                self.purchase_component(kind, &id);
                None
            }
            UiAction::FieldRepair(kind) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crate::simulation::ship::field_repair(
                        &mut gameplay.sim,
                        &self.data.config,
                        kind,
                    ) {
                        Ok(()) => self.notifications.success("Field repair complete."),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::FullRepair => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crate::simulation::ship::full_repair(&mut gameplay.sim, &self.data.config)
                    {
                        Ok(()) => self.notifications.success("Full refit complete."),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::InstallSalvage(id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crate::simulation::ship::install_salvage(
                        &mut gameplay.sim,
                        &self.data,
                        &id,
                    ) {
                        Ok(()) => self.notifications.success("Salvage installed."),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::CommissionShip(id) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match crate::simulation::ship::commission_ship(
                        &mut gameplay.sim,
                        &self.data,
                        &id,
                    ) {
                        Ok(()) => self.notifications.success("New ship commissioned."),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::Buy(resource, amount) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match market::buy(&mut gameplay.sim, resource, amount) {
                        Ok(()) => self
                            .notifications
                            .success(format!("Bought {amount} {}", resource.label())),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::Sell(resource, amount) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    match market::sell(&mut gameplay.sim, resource, amount) {
                        Ok(()) => self
                            .notifications
                            .success(format!("Sold {amount} {}", resource.label())),
                        Err(err) => self.notifications.warning(err),
                    }
                }
                None
            }
            UiAction::ToggleDelegation(category) => {
                if let GameState::Gameplay(gameplay) = &mut self.state {
                    gameplay.sim.delegation.toggle(category);
                }
                None
            }
        }
    }

    fn advance(&mut self) {
        let GameState::Gameplay(gameplay) = &mut self.state else {
            return;
        };
        let sim = &mut gameplay.sim;
        if sim.has_pending_decision() || sim.dynasty.extinct {
            return;
        }

        let report = tick::advance(sim, &self.data);

        if report.decision_required {
            self.notifications.warning("The council must decide.");
        }
        if report.dynasty_extinct {
            self.notifications.danger("The dynasty has ended.");
        }
        if let Some((score, level)) = report.contract_completed {
            let entry = ChronicleEntry {
                completed_year: sim.year(),
                contract_name: sim
                    .contract
                    .as_ref()
                    .map(|c| c.name.clone())
                    .unwrap_or_default(),
                objective: sim
                    .contract
                    .as_ref()
                    .map(|c| c.objective.label().to_owned())
                    .unwrap_or_default(),
                legacy_id: sim.legacy.legacy_id.clone(),
                leader_name: sim
                    .dynasty
                    .leader()
                    .map(|l| l.name.clone())
                    .unwrap_or_else(|| "an empty chair".to_owned()),
                generation: sim.dynasty.generation,
                score,
                outcome: level.label().to_owned(),
                duration_years: sim
                    .contract
                    .as_ref()
                    .map(|c| c.months_elapsed / 12)
                    .unwrap_or_default(),
            };
            // Freeze the run timer for the Homecoming (PLAN M4.7).
            self.last_mission_real_secs = self.mission_started.map(|t| (get_time() - t) as f32);
            self.mission_started = None;
            sim.push_log(format!(
                "Contract concluded: {} — {} (score {score:.2}).",
                entry.contract_name, entry.outcome
            ));
            // Pay is strictly proportional to objective completion (W2): a
            // full-term run pays in full, a truncated one pays its fraction, and
            // zero objective progress pays nothing. The failure band no longer
            // zeroes pay by itself — objective progress alone decides it.
            let payout = sim.contract.as_ref().and_then(|c| {
                self.data
                    .contracts
                    .get(&c.template_id)
                    .map(|t| contract::prorated_reward(&t.reward, c.objective_fraction()))
            });
            if let Some(payout) = payout {
                sim.resources.apply(&payout);
            }
            sim.contract = None;

            self.chronicle.record(entry);
            if let Err(err) = self.chronicle.save(
                &self.data.config.game_name,
                &self.data.config.chronicle_slot,
                &self.data.config.version,
            ) {
                self.notifications
                    .danger(format!("Chronicle write failed: {err}"));
            }
            self.notifications
                .success("Contract concluded — see the Chronicle.");
        }
    }

    fn purchase_component(&mut self, kind: ComponentKind, id: &str) {
        let GameState::Gameplay(gameplay) = &mut self.state else {
            return;
        };
        let Some(component) = self.data.ship_components.find(kind, id) else {
            return;
        };
        let sim = &mut gameplay.sim;

        // Loadout changes are a drydock job (PLAN M4.6): only in port.
        if sim.contract.is_some() {
            self.notifications
                .warning("Loadout changes wait for port — you're underway.");
            return;
        }

        let cost = crate::data::ResourceDelta {
            credits: -component.cost.credits,
            energy: -component.cost.energy,
            minerals: -component.cost.minerals,
            food: -component.cost.food,
            influence: -component.cost.influence,
        };
        if !sim.resources.can_afford(&cost) {
            self.notifications.warning("The treasury cannot cover it.");
            return;
        }
        sim.resources.apply(&cost);
        match kind {
            ComponentKind::Hull => sim.ship.hull = component.id.clone(),
            ComponentKind::Engine => sim.ship.engine = component.id.clone(),
            ComponentKind::Weapon => sim.ship.weapon = Some(component.id.clone()),
        }
        sim.push_log(format!("Refit complete: {} installed.", component.name));
        self.notifications
            .success(format!("{} installed.", component.name));
    }
}
