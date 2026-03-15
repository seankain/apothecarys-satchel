use crate::location::{LocationDef, SpawnPoint, SpawnType};

/// Data needed to place an entity in a scene.
#[derive(Debug, Clone)]
pub struct EntityPlacement {
    pub position: [f32; 3],
    pub rotation: f32,
    pub kind: PlacementKind,
}

/// What kind of entity to place.
#[derive(Debug, Clone)]
pub enum PlacementKind {
    Player,
    Enemy { template: String },
    Item { item_id: String },
    Npc { npc_id: String },
}

/// Extract all entity placements from a location's spawn points.
pub fn get_placements(location: &LocationDef) -> Vec<EntityPlacement> {
    location
        .spawn_points
        .iter()
        .filter_map(spawn_to_placement)
        .collect()
}

/// Get the player arrival placement at a named spawn point.
pub fn get_player_placement(location: &LocationDef, spawn_name: &str) -> Option<EntityPlacement> {
    location
        .spawn_points
        .iter()
        .find(|sp| sp.name == spawn_name && matches!(sp.spawn_type, SpawnType::PlayerArrival))
        .map(|sp| EntityPlacement {
            position: sp.position,
            rotation: sp.rotation,
            kind: PlacementKind::Player,
        })
}

/// Get all non-player entity placements (enemies, items, NPCs).
pub fn get_entity_placements(location: &LocationDef) -> Vec<EntityPlacement> {
    location
        .spawn_points
        .iter()
        .filter(|sp| !matches!(sp.spawn_type, SpawnType::PlayerArrival))
        .filter_map(spawn_to_placement)
        .collect()
}

fn spawn_to_placement(sp: &SpawnPoint) -> Option<EntityPlacement> {
    let kind = match &sp.spawn_type {
        SpawnType::PlayerArrival => PlacementKind::Player,
        SpawnType::Enemy { template } => PlacementKind::Enemy {
            template: template.clone(),
        },
        SpawnType::Item { item_id } => PlacementKind::Item {
            item_id: item_id.clone(),
        },
        SpawnType::Npc { npc_id } => PlacementKind::Npc {
            npc_id: npc_id.clone(),
        },
    };

    Some(EntityPlacement {
        position: sp.position,
        rotation: sp.rotation,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::*;

    fn test_location() -> LocationDef {
        LocationDef {
            id: LocationId::from("dungeon_f1"),
            display_name: "Dungeon Floor 1".to_string(),
            scene_path: "dungeon_f1.gltf".to_string(),
            location_type: LocationType::Dungeon {
                floor: 1,
                difficulty: 1,
            },
            spawn_points: vec![
                SpawnPoint {
                    name: "entrance".to_string(),
                    position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::PlayerArrival,
                },
                SpawnPoint {
                    name: "goblin_1".to_string(),
                    position: [10.0, 0.0, 5.0],
                    rotation: 1.5,
                    spawn_type: SpawnType::Enemy {
                        template: "goblin_warrior".to_string(),
                    },
                },
                SpawnPoint {
                    name: "potion_spawn".to_string(),
                    position: [8.0, 0.0, 3.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::Item {
                        item_id: "potion_heal_minor".to_string(),
                    },
                },
                SpawnPoint {
                    name: "herbalist".to_string(),
                    position: [15.0, 0.0, 10.0],
                    rotation: std::f32::consts::PI,
                    spawn_type: SpawnType::Npc {
                        npc_id: "npc_herbalist".to_string(),
                    },
                },
            ],
            exits: vec![],
        }
    }

    #[test]
    fn test_get_all_placements() {
        let loc = test_location();
        let placements = get_placements(&loc);
        assert_eq!(placements.len(), 4);
    }

    #[test]
    fn test_get_player_placement() {
        let loc = test_location();
        let player = get_player_placement(&loc, "entrance").unwrap();
        assert!(matches!(player.kind, PlacementKind::Player));
        assert_eq!(player.position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_get_player_placement_wrong_name() {
        let loc = test_location();
        assert!(get_player_placement(&loc, "nonexistent").is_none());
    }

    #[test]
    fn test_get_entity_placements_excludes_player() {
        let loc = test_location();
        let entities = get_entity_placements(&loc);
        assert_eq!(entities.len(), 3);
        for e in &entities {
            assert!(!matches!(e.kind, PlacementKind::Player));
        }
    }

    #[test]
    fn test_enemy_placement() {
        let loc = test_location();
        let entities = get_entity_placements(&loc);
        let enemy = entities
            .iter()
            .find(|e| matches!(&e.kind, PlacementKind::Enemy { template } if template == "goblin_warrior"))
            .unwrap();
        assert_eq!(enemy.position, [10.0, 0.0, 5.0]);
    }
}
