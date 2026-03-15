use std::collections::HashMap;
use std::path::Path;

use crate::location::{ExitDef, LocationDef, LocationId};

/// Errors that can occur when loading or validating a world graph.
#[derive(Debug, thiserror::Error)]
pub enum WorldGraphError {
    #[error("IO error reading world data: {0}")]
    Io(#[from] std::io::Error),
    #[error("RON parse error: {0}")]
    Ron(#[from] ron::error::SpannedError),
    #[error("Duplicate location id: {id}")]
    DuplicateLocation { id: String },
    #[error("Exit in '{from}' references unknown location '{to}'")]
    DanglingExit { from: String, to: String },
    #[error("Exit in '{from}' targets spawn '{spawn}' which doesn't exist in '{to}'")]
    MissingSpawn {
        from: String,
        to: String,
        spawn: String,
    },
}

/// A graph of interconnected world locations loaded from data files.
pub struct WorldGraph {
    locations: HashMap<LocationId, LocationDef>,
}

impl WorldGraph {
    /// Load the world graph from a RON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, WorldGraphError> {
        let contents = std::fs::read_to_string(path)?;
        Self::from_ron(&contents)
    }

    /// Parse the world graph from a RON string.
    pub fn from_ron(source: &str) -> Result<Self, WorldGraphError> {
        let defs: Vec<LocationDef> = ron::from_str(source)?;
        Self::from_locations(defs)
    }

    /// Build the world graph from a list of location definitions.
    pub fn from_locations(defs: Vec<LocationDef>) -> Result<Self, WorldGraphError> {
        let mut locations = HashMap::new();
        for def in defs {
            if locations.contains_key(&def.id) {
                return Err(WorldGraphError::DuplicateLocation {
                    id: def.id.0.clone(),
                });
            }
            locations.insert(def.id.clone(), def);
        }
        let graph = Self { locations };
        graph.validate()?;
        Ok(graph)
    }

    /// Validate that all exits reference existing locations and spawn points.
    fn validate(&self) -> Result<(), WorldGraphError> {
        for loc in self.locations.values() {
            for exit in &loc.exits {
                let target = self.locations.get(&exit.target_location).ok_or_else(|| {
                    WorldGraphError::DanglingExit {
                        from: loc.id.0.clone(),
                        to: exit.target_location.0.clone(),
                    }
                })?;

                let has_spawn = target
                    .spawn_points
                    .iter()
                    .any(|sp| sp.name == exit.arrival_spawn);
                if !has_spawn {
                    return Err(WorldGraphError::MissingSpawn {
                        from: loc.id.0.clone(),
                        to: exit.target_location.0.clone(),
                        spawn: exit.arrival_spawn.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Get a location definition by id.
    pub fn get_location(&self, id: &LocationId) -> Option<&LocationDef> {
        self.locations.get(id)
    }

    /// Get exits from a location.
    pub fn get_exits(&self, id: &LocationId) -> &[ExitDef] {
        self.locations
            .get(id)
            .map(|loc| loc.exits.as_slice())
            .unwrap_or(&[])
    }

    /// Get all location ids directly reachable from a location.
    pub fn get_connected(&self, id: &LocationId) -> Vec<&LocationId> {
        self.get_exits(id)
            .iter()
            .map(|exit| &exit.target_location)
            .collect()
    }

    /// Get all location ids in the graph.
    pub fn location_ids(&self) -> impl Iterator<Item = &LocationId> {
        self.locations.keys()
    }

    /// Get the total number of locations.
    pub fn location_count(&self) -> usize {
        self.locations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::*;

    fn sample_world() -> Vec<LocationDef> {
        vec![
            LocationDef {
                id: LocationId::from("hub_town"),
                display_name: "Willowmere".to_string(),
                scene_path: "assets/scenes/hub_town.gltf".to_string(),
                location_type: LocationType::Hub,
                spawn_points: vec![
                    SpawnPoint {
                        name: "from_garden".to_string(),
                        position: [5.0, 0.0, 3.0],
                        rotation: 0.0,
                        spawn_type: SpawnType::PlayerArrival,
                    },
                    SpawnPoint {
                        name: "from_dungeon_1".to_string(),
                        position: [12.0, 0.0, 8.0],
                        rotation: std::f32::consts::PI,
                        spawn_type: SpawnType::PlayerArrival,
                    },
                ],
                exits: vec![
                    ExitDef {
                        target_location: LocationId::from("garden"),
                        exit_node_name: "exit_to_garden".to_string(),
                        arrival_spawn: "from_hub".to_string(),
                    },
                    ExitDef {
                        target_location: LocationId::from("dungeon_floor_1"),
                        exit_node_name: "exit_to_dungeon".to_string(),
                        arrival_spawn: "entrance".to_string(),
                    },
                ],
            },
            LocationDef {
                id: LocationId::from("garden"),
                display_name: "Apothecary's Garden".to_string(),
                scene_path: "assets/scenes/garden.gltf".to_string(),
                location_type: LocationType::Garden,
                spawn_points: vec![SpawnPoint {
                    name: "from_hub".to_string(),
                    position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::PlayerArrival,
                }],
                exits: vec![ExitDef {
                    target_location: LocationId::from("hub_town"),
                    exit_node_name: "exit_to_hub".to_string(),
                    arrival_spawn: "from_garden".to_string(),
                }],
            },
            LocationDef {
                id: LocationId::from("dungeon_floor_1"),
                display_name: "Darkroot Caverns - Floor 1".to_string(),
                scene_path: "assets/scenes/dungeon_f1.gltf".to_string(),
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
                        name: "from_floor_2".to_string(),
                        position: [20.0, 0.0, 20.0],
                        rotation: std::f32::consts::PI,
                        spawn_type: SpawnType::PlayerArrival,
                    },
                ],
                exits: vec![
                    ExitDef {
                        target_location: LocationId::from("hub_town"),
                        exit_node_name: "exit_to_hub".to_string(),
                        arrival_spawn: "from_dungeon_1".to_string(),
                    },
                    ExitDef {
                        target_location: LocationId::from("dungeon_floor_2"),
                        exit_node_name: "stairs_down".to_string(),
                        arrival_spawn: "from_floor_1".to_string(),
                    },
                ],
            },
            LocationDef {
                id: LocationId::from("dungeon_floor_2"),
                display_name: "Darkroot Caverns - Floor 2".to_string(),
                scene_path: "assets/scenes/dungeon_f2.gltf".to_string(),
                location_type: LocationType::Dungeon {
                    floor: 2,
                    difficulty: 3,
                },
                spawn_points: vec![SpawnPoint {
                    name: "from_floor_1".to_string(),
                    position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::PlayerArrival,
                }],
                exits: vec![ExitDef {
                    target_location: LocationId::from("dungeon_floor_1"),
                    exit_node_name: "stairs_up".to_string(),
                    arrival_spawn: "from_floor_2".to_string(),
                }],
            },
        ]
    }

    #[test]
    fn test_world_graph_loads_from_locations() {
        let graph = WorldGraph::from_locations(sample_world()).unwrap();
        assert_eq!(graph.location_count(), 4);
    }

    #[test]
    fn test_get_location() {
        let graph = WorldGraph::from_locations(sample_world()).unwrap();
        let hub = graph.get_location(&LocationId::from("hub_town")).unwrap();
        assert_eq!(hub.display_name, "Willowmere");
        assert_eq!(hub.location_type, LocationType::Hub);
    }

    #[test]
    fn test_get_exits() {
        let graph = WorldGraph::from_locations(sample_world()).unwrap();
        let exits = graph.get_exits(&LocationId::from("hub_town"));
        assert_eq!(exits.len(), 2);
        assert_eq!(exits[0].target_location, LocationId::from("garden"));
        assert_eq!(exits[1].target_location, LocationId::from("dungeon_floor_1"));
    }

    #[test]
    fn test_get_connected() {
        let graph = WorldGraph::from_locations(sample_world()).unwrap();
        let connected = graph.get_connected(&LocationId::from("hub_town"));
        assert_eq!(connected.len(), 2);
        assert!(connected.contains(&&LocationId::from("garden")));
        assert!(connected.contains(&&LocationId::from("dungeon_floor_1")));
    }

    #[test]
    fn test_nonexistent_location() {
        let graph = WorldGraph::from_locations(sample_world()).unwrap();
        assert!(graph.get_location(&LocationId::from("nowhere")).is_none());
        assert!(graph.get_exits(&LocationId::from("nowhere")).is_empty());
        assert!(graph.get_connected(&LocationId::from("nowhere")).is_empty());
    }

    #[test]
    fn test_duplicate_location_error() {
        let mut defs = sample_world();
        defs.push(defs[0].clone());
        let result = WorldGraph::from_locations(defs);
        assert!(matches!(result, Err(WorldGraphError::DuplicateLocation { .. })));
    }

    #[test]
    fn test_dangling_exit_error() {
        let defs = vec![LocationDef {
            id: LocationId::from("lonely"),
            display_name: "Lonely Place".to_string(),
            scene_path: "assets/scenes/lonely.gltf".to_string(),
            location_type: LocationType::Town,
            spawn_points: vec![],
            exits: vec![ExitDef {
                target_location: LocationId::from("nonexistent"),
                exit_node_name: "exit".to_string(),
                arrival_spawn: "spawn".to_string(),
            }],
        }];
        let result = WorldGraph::from_locations(defs);
        assert!(matches!(result, Err(WorldGraphError::DanglingExit { .. })));
    }

    #[test]
    fn test_missing_spawn_error() {
        let defs = vec![
            LocationDef {
                id: LocationId::from("a"),
                display_name: "A".to_string(),
                scene_path: "a.gltf".to_string(),
                location_type: LocationType::Hub,
                spawn_points: vec![],
                exits: vec![ExitDef {
                    target_location: LocationId::from("b"),
                    exit_node_name: "exit".to_string(),
                    arrival_spawn: "wrong_spawn_name".to_string(),
                }],
            },
            LocationDef {
                id: LocationId::from("b"),
                display_name: "B".to_string(),
                scene_path: "b.gltf".to_string(),
                location_type: LocationType::Town,
                spawn_points: vec![SpawnPoint {
                    name: "correct_spawn".to_string(),
                    position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::PlayerArrival,
                }],
                exits: vec![],
            },
        ];
        let result = WorldGraph::from_locations(defs);
        assert!(matches!(result, Err(WorldGraphError::MissingSpawn { .. })));
    }

    #[test]
    fn test_ron_roundtrip() {
        let defs = sample_world();
        let ron_str = ron::to_string(&defs).unwrap();
        let graph = WorldGraph::from_ron(&ron_str).unwrap();
        assert_eq!(graph.location_count(), 4);

        let hub = graph.get_location(&LocationId::from("hub_town")).unwrap();
        assert_eq!(hub.display_name, "Willowmere");
    }

    #[test]
    fn test_garden_is_connected_to_hub() {
        let graph = WorldGraph::from_locations(sample_world()).unwrap();
        let garden_connected = graph.get_connected(&LocationId::from("garden"));
        assert_eq!(garden_connected.len(), 1);
        assert_eq!(garden_connected[0], &LocationId::from("hub_town"));
    }

    #[test]
    fn test_dungeon_floor_connectivity() {
        let graph = WorldGraph::from_locations(sample_world()).unwrap();
        let f1_connected = graph.get_connected(&LocationId::from("dungeon_floor_1"));
        assert_eq!(f1_connected.len(), 2);
        assert!(f1_connected.contains(&&LocationId::from("hub_town")));
        assert!(f1_connected.contains(&&LocationId::from("dungeon_floor_2")));

        let f2_connected = graph.get_connected(&LocationId::from("dungeon_floor_2"));
        assert_eq!(f2_connected.len(), 1);
        assert_eq!(f2_connected[0], &LocationId::from("dungeon_floor_1"));
    }
}
