//! Main-menu state: the title/main screen (continue, new game, settings, exit)
//! and the new-game screen (legacy + founding-faction selection).

/// Which menu screen is showing. After the boot log the menu opens on `Main`
/// (the title screen with the four options); choosing NEW GAME steps into the
/// `NewGame` picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuPhase {
    Main,
    NewGame,
}

#[derive(Debug, Clone)]
pub struct MenuState {
    /// Which menu screen is showing.
    pub phase: MenuPhase,
    /// Index into the sorted legacy id list shown by the menu UI.
    pub selected_legacy: usize,
    /// Founding factions the player has toggled on (W7). START enables only
    /// when exactly `config.factions.starting_count` are chosen.
    pub selected_factions: Vec<String>,
    pub save_exists: bool,
}

impl MenuState {
    pub fn new(save_exists: bool) -> Self {
        Self {
            phase: MenuPhase::Main,
            selected_legacy: 0,
            selected_factions: Vec::new(),
            save_exists,
        }
    }

    /// Toggle a faction on/off in the founding selection (W7).
    pub fn toggle_faction(&mut self, id: &str) {
        if let Some(pos) = self.selected_factions.iter().position(|f| f == id) {
            self.selected_factions.remove(pos);
        } else {
            self.selected_factions.push(id.to_owned());
        }
    }
}
