//! Ship component catalog: hulls, engines, weapons (GDD §6).

use crate::data::ResourceDelta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentKind {
    Hull,
    Engine,
    Weapon,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ComponentStats {
    pub cargo: i32,
    pub crew_capacity: i32,
    pub speed: i32,
    pub combat: i32,
    pub fuel_regen: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipComponent {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub cost: ResourceDelta,
    #[serde(default)]
    pub stats: ComponentStats,
    /// Can this part be fitted underway by a skilled engineer, or does it need a
    /// drydock (PLAN M4.4)? Engines/weapons are modular; hulls are structural.
    #[serde(default)]
    pub field_installable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipComponentCatalog {
    pub hulls: Vec<ShipComponent>,
    pub engines: Vec<ShipComponent>,
    pub weapons: Vec<ShipComponent>,
}

impl ShipComponentCatalog {
    pub fn list(&self, kind: ComponentKind) -> &[ShipComponent] {
        match kind {
            ComponentKind::Hull => &self.hulls,
            ComponentKind::Engine => &self.engines,
            ComponentKind::Weapon => &self.weapons,
        }
    }

    pub fn find(&self, kind: ComponentKind, id: &str) -> Option<&ShipComponent> {
        self.list(kind).iter().find(|c| c.id == id)
    }

    /// Find a component by id across all three slots (PLAN M4.4: a salvaged
    /// part's kind isn't known up front). Returns the slot it belongs in.
    pub fn find_any(&self, id: &str) -> Option<(ComponentKind, &ShipComponent)> {
        for kind in [
            ComponentKind::Hull,
            ComponentKind::Engine,
            ComponentKind::Weapon,
        ] {
            if let Some(component) = self.find(kind, id) {
                return Some((kind, component));
            }
        }
        None
    }
}
