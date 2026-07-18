//! Main-menu state: legacy selection for a new voyage, continue, delete save.

#[derive(Debug, Clone)]
pub struct MenuState {
    /// Index into the sorted legacy id list shown by the menu UI.
    pub selected_legacy: usize,
    pub save_exists: bool,
}

impl MenuState {
    pub fn new(save_exists: bool) -> Self {
        Self {
            selected_legacy: 0,
            save_exists,
        }
    }
}
