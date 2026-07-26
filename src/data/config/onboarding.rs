//! First-run guidance: the welcome screen and the launch-flow checklist.

use serde::{Deserialize, Serialize};

/// One step of the first-voyage checklist. The `id` binds it to a completion
/// check in the PREP screen; label and tip are authored content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialStep {
    pub id: String,
    pub label: String,
    pub tip: String,
}
/// First-voyage tutorial content. Shown only until the Chronicle records a
/// mission (or the player dismisses it); all text is data, per the hard rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TutorialConfig {
    /// One-line hint over the drydock charter list on a first voyage.
    pub drydock_hint: String,
    /// The same line's everyday text once the tutorial is over.
    pub drydock_refit_hint: String,
    /// Explains the legacy column on the new-game screen (what the choice does).
    pub legacy_intro: String,
    /// Explains the founding-peoples column on the new-game screen.
    pub factions_intro: String,
    /// Ordered pre-launch checklist steps for the PREP screen.
    pub steps: Vec<TutorialStep>,
}
/// One captioned block of the first-run welcome overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeSection {
    pub heading: String,
    pub body: String,
}
/// First-run welcome overlay content (shown once, gated on a saved flag). All
/// text is data, per the hard rule; the overlay itself lives in `ui::welcome`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeConfig {
    pub title: String,
    pub intro: String,
    pub sections: Vec<WelcomeSection>,
    pub dismiss_label: String,
}
