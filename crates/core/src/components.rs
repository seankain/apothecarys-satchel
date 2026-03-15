use serde::{Deserialize, Serialize};

/// Types of interactions available on objects in the world.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InteractionType {
    /// Pick up an item from the ground
    Pickup,
    /// Examine an object for information
    Examine,
    /// Exit to another location
    Exit,
    /// Talk to an NPC
    Npc,
    /// Interact with a garden plot
    GardenPlot,
    /// Use a crafting station
    CraftingStation,
}

/// Component marking a scene node as interactable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Interactable {
    /// Display name shown on hover tooltip
    pub name: String,
    /// What kind of interaction this object supports
    pub interaction_type: InteractionType,
    /// Maximum distance from which the player can interact (world units)
    pub interaction_range: f32,
    /// Whether this interactable is currently enabled
    pub enabled: bool,
}

impl Interactable {
    pub fn new(name: impl Into<String>, interaction_type: InteractionType) -> Self {
        Self {
            name: name.into(),
            interaction_type,
            interaction_range: 2.0,
            enabled: true,
        }
    }

    pub fn with_range(mut self, range: f32) -> Self {
        self.interaction_range = range;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interactable_creation() {
        let inter = Interactable::new("Healing Herb", InteractionType::Pickup).with_range(3.0);
        assert_eq!(inter.name, "Healing Herb");
        assert_eq!(inter.interaction_type, InteractionType::Pickup);
        assert!((inter.interaction_range - 3.0).abs() < f32::EPSILON);
        assert!(inter.enabled);
    }

    #[test]
    fn test_interactable_serde_roundtrip() {
        let inter = Interactable::new("Dungeon Exit", InteractionType::Exit);
        let json = serde_json::to_string(&inter).unwrap();
        let deserialized: Interactable = serde_json::from_str(&json).unwrap();
        assert_eq!(inter, deserialized);
    }
}
