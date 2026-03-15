use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a world location.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocationId(pub String);

impl fmt::Display for LocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for LocationId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// The type/category of a location, affecting gameplay behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LocationType {
    Hub,
    Garden,
    Dungeon { floor: u32, difficulty: u32 },
    Town,
}

/// What type of entity spawns at this point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpawnType {
    PlayerArrival,
    Enemy { template: String },
    Item { item_id: String },
    Npc { npc_id: String },
}

/// A named point in a location where entities can spawn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpawnPoint {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: f32,
    pub spawn_type: SpawnType,
}

/// Defines a connection from this location to another.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExitDef {
    pub target_location: LocationId,
    pub exit_node_name: String,
    pub arrival_spawn: String,
}

/// Full definition of a world location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocationDef {
    pub id: LocationId,
    pub display_name: String,
    pub scene_path: String,
    pub location_type: LocationType,
    pub spawn_points: Vec<SpawnPoint>,
    pub exits: Vec<ExitDef>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_location_id_equality() {
        let a = LocationId::from("hub");
        let b = LocationId("hub".to_string());
        assert_eq!(a, b);
    }

    #[test]
    fn test_location_id_display() {
        let id = LocationId::from("dungeon_floor_1");
        assert_eq!(format!("{id}"), "dungeon_floor_1");
    }

    #[test]
    fn test_location_def_serde_roundtrip() {
        let loc = LocationDef {
            id: LocationId::from("hub_town"),
            display_name: "Willowmere".to_string(),
            scene_path: "assets/scenes/hub_town.gltf".to_string(),
            location_type: LocationType::Hub,
            spawn_points: vec![SpawnPoint {
                name: "from_garden".to_string(),
                position: [5.0, 0.0, 3.0],
                rotation: 0.0,
                spawn_type: SpawnType::PlayerArrival,
            }],
            exits: vec![ExitDef {
                target_location: LocationId::from("garden"),
                exit_node_name: "exit_to_garden".to_string(),
                arrival_spawn: "from_hub".to_string(),
            }],
        };

        let ron_str = ron::to_string(&loc).unwrap();
        let deserialized: LocationDef = ron::from_str(&ron_str).unwrap();
        assert_eq!(loc, deserialized);
    }

    #[test]
    fn test_dungeon_location_type() {
        let loc_type = LocationType::Dungeon {
            floor: 3,
            difficulty: 5,
        };
        let ron_str = ron::to_string(&loc_type).unwrap();
        let deserialized: LocationType = ron::from_str(&ron_str).unwrap();
        assert_eq!(loc_type, deserialized);
    }

    #[test]
    fn test_spawn_types_serde() {
        let spawns = vec![
            SpawnType::PlayerArrival,
            SpawnType::Enemy {
                template: "goblin".to_string(),
            },
            SpawnType::Item {
                item_id: "potion_heal".to_string(),
            },
            SpawnType::Npc {
                npc_id: "herbalist".to_string(),
            },
        ];

        for spawn in spawns {
            let ron_str = ron::to_string(&spawn).unwrap();
            let deserialized: SpawnType = ron::from_str(&ron_str).unwrap();
            assert_eq!(spawn, deserialized);
        }
    }
}
