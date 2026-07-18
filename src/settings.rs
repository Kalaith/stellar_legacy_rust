//! Player-facing display settings for the CRT monitor look. These are cosmetic
//! and live outside the sim save (their own persistence key), so they never
//! affect determinism (GDD §5.6).

use macroquad_toolkit::fx::CrtStyle;
use macroquad_toolkit::persistence::{load_json_key, save_json_key};
use serde::{Deserialize, Serialize};

use crate::state::sim::DelegationSettings;

/// Persistence key, separate from the campaign save slot.
pub const DISPLAY_KEY: &str = "display";
/// Persistence key for the default per-category council delegation (GDD §5.4)
/// applied to each new voyage.
pub const DELEGATION_KEY: &str = "delegation";

/// Load the persisted default delegation for new campaigns (all-council if
/// never set).
pub fn load_delegation(game_name: &str) -> DelegationSettings {
    load_json_key(game_name, DELEGATION_KEY).unwrap_or_default()
}

/// Persist the default delegation preferences.
pub fn save_delegation(delegation: &DelegationSettings, game_name: &str) -> Result<(), String> {
    save_json_key(game_name, DELEGATION_KEY, delegation)
}

/// Phosphor tube color for the CRT overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Phosphor {
    /// Warm amber (P3).
    #[default]
    Amber,
    /// Cool green (P1).
    Green,
}

/// User's CRT display preferences. All fields `serde(default)` so older or
/// partial blobs load cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    /// Master switch for the whole CRT overlay.
    pub crt_enabled: bool,
    /// Draw scanlines.
    pub scanlines: bool,
    /// Apply the subtle whole-screen flicker.
    pub flicker: bool,
    /// Phosphor tint of the overlay.
    pub phosphor: Phosphor,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            crt_enabled: true,
            scanlines: true,
            flicker: true,
            phosphor: Phosphor::Amber,
        }
    }
}

impl DisplaySettings {
    /// Loads saved preferences, falling back to defaults.
    pub fn load(game_name: &str) -> Self {
        load_json_key(game_name, DISPLAY_KEY).unwrap_or_default()
    }

    /// Persists the current preferences.
    pub fn save(&self, game_name: &str) -> Result<(), String> {
        save_json_key(game_name, DISPLAY_KEY, self)
    }

    /// Build the overlay style these settings describe. `crt_enabled` is honored
    /// separately by the caller (whether to draw at all).
    pub fn crt_style(&self) -> CrtStyle {
        let mut style = match self.phosphor {
            Phosphor::Amber => CrtStyle::amber(),
            Phosphor::Green => CrtStyle::green(),
        };
        if !self.scanlines {
            style.scanline_alpha = 0.0;
        }
        if !self.flicker {
            style.flicker_alpha = 0.0;
        }
        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_reflects_toggles() {
        let mut s = DisplaySettings::default();
        assert!(s.crt_style().scanline_alpha > 0.0);
        s.scanlines = false;
        s.flicker = false;
        let style = s.crt_style();
        assert_eq!(style.scanline_alpha, 0.0);
        assert_eq!(style.flicker_alpha, 0.0);
        // Vignette is unaffected by the scanline/flicker toggles.
        assert!(style.vignette_alpha > 0.0);
    }

    #[test]
    fn green_phosphor_tints_differently() {
        let amber = DisplaySettings {
            phosphor: Phosphor::Amber,
            ..Default::default()
        };
        let green = DisplaySettings {
            phosphor: Phosphor::Green,
            ..Default::default()
        };
        assert_ne!(amber.crt_style().tint, green.crt_style().tint);
    }

    #[test]
    fn partial_json_loads_with_defaults() {
        let s: DisplaySettings = serde_json::from_str(r#"{"scanlines": false}"#).unwrap();
        assert!(!s.scanlines);
        assert!(s.crt_enabled);
        assert_eq!(s.phosphor, Phosphor::Amber);
    }
}
