use apothecarys_world::location::{ExitDef, LocationDef, LocationId, LocationType, SpawnPoint, SpawnType};
use apothecarys_world::map_graph::{WorldGraph, WorldGraphError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A 2D position for a location node in the graph editor canvas.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32,
}

impl NodePosition {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A location node as displayed in the connection editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorNode {
    pub location: LocationDef,
    pub canvas_position: NodePosition,
}

/// State of the connection editor.
#[derive(Debug)]
pub struct ConnectionEditorState {
    nodes: Vec<EditorNode>,
    /// Camera pan offset.
    pub pan_offset: NodePosition,
    /// Zoom level (1.0 = default).
    pub zoom: f32,
    /// Index of selected node, if any.
    pub selected_node: Option<usize>,
}

impl ConnectionEditorState {
    /// Create a new empty editor state.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            pan_offset: NodePosition::new(0.0, 0.0),
            zoom: 1.0,
            selected_node: None,
        }
    }

    /// Load locations from a RON string and arrange them in a grid.
    pub fn load_from_ron(&mut self, source: &str) -> Result<(), WorldGraphError> {
        let defs: Vec<LocationDef> =
            ron::from_str(source).map_err(WorldGraphError::Ron)?;
        self.nodes.clear();
        for (i, def) in defs.into_iter().enumerate() {
            let col = (i % 4) as f32;
            let row = (i / 4) as f32;
            self.nodes.push(EditorNode {
                location: def,
                canvas_position: NodePosition::new(col * 250.0 + 50.0, row * 200.0 + 50.0),
            });
        }
        Ok(())
    }

    /// Load locations from a RON file.
    pub fn load_from_file(&mut self, path: impl AsRef<Path>) -> Result<(), ConnectionEditorError> {
        let contents = std::fs::read_to_string(path)?;
        self.load_from_ron(&contents)?;
        Ok(())
    }

    /// Add a new location node at the given canvas position.
    pub fn add_node(&mut self, id: &str, display_name: &str, location_type: LocationType, position: NodePosition) {
        self.nodes.push(EditorNode {
            location: LocationDef {
                id: LocationId::from(id),
                display_name: display_name.to_string(),
                scene_path: format!("assets/scenes/{id}.gltf"),
                location_type,
                spawn_points: Vec::new(),
                exits: Vec::new(),
            },
            canvas_position: position,
        });
    }

    /// Remove a node by index.
    pub fn remove_node(&mut self, index: usize) -> Option<EditorNode> {
        if index >= self.nodes.len() {
            return None;
        }
        let removed = self.nodes.remove(index);

        // Clean up dangling exits that reference the removed location
        let removed_id = removed.location.id.clone();
        for node in &mut self.nodes {
            node.location
                .exits
                .retain(|exit| exit.target_location != removed_id);
        }

        if self.selected_node == Some(index) {
            self.selected_node = None;
        } else if let Some(sel) = self.selected_node {
            if sel > index {
                self.selected_node = Some(sel - 1);
            }
        }

        Some(removed)
    }

    /// Add a connection (exit) from one location to another.
    pub fn add_connection(
        &mut self,
        from_index: usize,
        to_index: usize,
        exit_node_name: &str,
        arrival_spawn: &str,
    ) -> bool {
        if from_index >= self.nodes.len() || to_index >= self.nodes.len() || from_index == to_index
        {
            return false;
        }
        let target_id = self.nodes[to_index].location.id.clone();
        self.nodes[from_index].location.exits.push(ExitDef {
            target_location: target_id,
            exit_node_name: exit_node_name.to_string(),
            arrival_spawn: arrival_spawn.to_string(),
        });
        true
    }

    /// Remove a connection from a node by exit index.
    pub fn remove_connection(&mut self, node_index: usize, exit_index: usize) -> bool {
        if node_index >= self.nodes.len() {
            return false;
        }
        let node = &mut self.nodes[node_index];
        if exit_index >= node.location.exits.len() {
            return false;
        }
        node.location.exits.remove(exit_index);
        true
    }

    /// Add a spawn point to a location.
    pub fn add_spawn_point(&mut self, node_index: usize, name: &str, position: [f32; 3]) -> bool {
        if node_index >= self.nodes.len() {
            return false;
        }
        self.nodes[node_index]
            .location
            .spawn_points
            .push(SpawnPoint {
                name: name.to_string(),
                position,
                rotation: 0.0,
                spawn_type: SpawnType::PlayerArrival,
            });
        true
    }

    /// Get all editor nodes.
    pub fn nodes(&self) -> &[EditorNode] {
        &self.nodes
    }

    /// Get a mutable reference to a node.
    pub fn node_mut(&mut self, index: usize) -> Option<&mut EditorNode> {
        self.nodes.get_mut(index)
    }

    /// Find which node a canvas point falls within (for click detection).
    /// Uses a fixed node size for hit testing.
    pub fn hit_test(&self, canvas_x: f32, canvas_y: f32, node_width: f32, node_height: f32) -> Option<usize> {
        for (i, node) in self.nodes.iter().enumerate().rev() {
            let nx = node.canvas_position.x;
            let ny = node.canvas_position.y;
            if canvas_x >= nx
                && canvas_x <= nx + node_width
                && canvas_y >= ny
                && canvas_y <= ny + node_height
            {
                return Some(i);
            }
        }
        None
    }

    /// Validate the current graph. Returns a list of error messages.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let defs: Vec<LocationDef> = self.nodes.iter().map(|n| n.location.clone()).collect();

        match WorldGraph::from_locations(defs) {
            Ok(_) => {}
            Err(e) => errors.push(e.to_string()),
        }

        // Additional checks
        for node in &self.nodes {
            if node.location.id.0.is_empty() {
                errors.push(format!(
                    "Location '{}' has an empty ID",
                    node.location.display_name
                ));
            }
            if node.location.display_name.is_empty() {
                errors.push(format!("Location '{}' has an empty display name", node.location.id));
            }
        }

        errors
    }

    /// Export the current state as a RON string of location definitions.
    pub fn export_ron(&self) -> Result<String, ConnectionEditorError> {
        let defs: Vec<&LocationDef> = self.nodes.iter().map(|n| &n.location).collect();
        let ron_str = ron::ser::to_string_pretty(&defs, ron::ser::PrettyConfig::default())
            .map_err(|e| ConnectionEditorError::Serialize(e.to_string()))?;
        Ok(ron_str)
    }

    /// Save to a RON file.
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), ConnectionEditorError> {
        let ron_str = self.export_ron()?;
        std::fs::write(path, ron_str)?;
        Ok(())
    }

    /// Get the number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for ConnectionEditorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors for the connection editor.
#[derive(Debug)]
pub enum ConnectionEditorError {
    Io(std::io::Error),
    World(WorldGraphError),
    Serialize(String),
}

impl std::fmt::Display for ConnectionEditorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionEditorError::Io(e) => write!(f, "IO error: {e}"),
            ConnectionEditorError::World(e) => write!(f, "World graph error: {e}"),
            ConnectionEditorError::Serialize(e) => write!(f, "Serialize error: {e}"),
        }
    }
}

impl std::error::Error for ConnectionEditorError {}

impl From<std::io::Error> for ConnectionEditorError {
    fn from(e: std::io::Error) -> Self {
        ConnectionEditorError::Io(e)
    }
}

impl From<WorldGraphError> for ConnectionEditorError {
    fn from(e: WorldGraphError) -> Self {
        ConnectionEditorError::World(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ron() -> String {
        let defs = vec![
            LocationDef {
                id: LocationId::from("hub"),
                display_name: "Hub Town".to_string(),
                scene_path: "assets/scenes/hub.gltf".to_string(),
                location_type: LocationType::Hub,
                spawn_points: vec![SpawnPoint {
                    name: "from_garden".to_string(),
                    position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::PlayerArrival,
                }],
                exits: vec![ExitDef {
                    target_location: LocationId::from("garden"),
                    exit_node_name: "exit_garden".to_string(),
                    arrival_spawn: "from_hub".to_string(),
                }],
            },
            LocationDef {
                id: LocationId::from("garden"),
                display_name: "Garden".to_string(),
                scene_path: "assets/scenes/garden.gltf".to_string(),
                location_type: LocationType::Garden,
                spawn_points: vec![SpawnPoint {
                    name: "from_hub".to_string(),
                    position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::PlayerArrival,
                }],
                exits: vec![ExitDef {
                    target_location: LocationId::from("hub"),
                    exit_node_name: "exit_hub".to_string(),
                    arrival_spawn: "from_garden".to_string(),
                }],
            },
        ];
        ron::ser::to_string_pretty(&defs, ron::ser::PrettyConfig::default()).unwrap()
    }

    #[test]
    fn test_new_editor_state() {
        let state = ConnectionEditorState::new();
        assert_eq!(state.node_count(), 0);
        assert!(state.selected_node.is_none());
        assert!((state.zoom - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_load_from_ron() {
        let mut state = ConnectionEditorState::new();
        state.load_from_ron(&sample_ron()).unwrap();
        assert_eq!(state.node_count(), 2);
        assert_eq!(state.nodes()[0].location.display_name, "Hub Town");
        assert_eq!(state.nodes()[1].location.display_name, "Garden");
    }

    #[test]
    fn test_add_and_remove_node() {
        let mut state = ConnectionEditorState::new();
        state.add_node("town", "Test Town", LocationType::Town, NodePosition::new(100.0, 100.0));
        assert_eq!(state.node_count(), 1);
        assert_eq!(state.nodes()[0].location.id, LocationId::from("town"));

        let removed = state.remove_node(0).unwrap();
        assert_eq!(removed.location.id, LocationId::from("town"));
        assert_eq!(state.node_count(), 0);
    }

    #[test]
    fn test_remove_node_cleans_dangling_exits() {
        let mut state = ConnectionEditorState::new();
        state.add_node("a", "Location A", LocationType::Hub, NodePosition::new(0.0, 0.0));
        state.add_node("b", "Location B", LocationType::Town, NodePosition::new(200.0, 0.0));
        state.add_spawn_point(1, "from_a", [0.0, 0.0, 0.0]);
        state.add_connection(0, 1, "exit_b", "from_a");

        assert_eq!(state.nodes()[0].location.exits.len(), 1);

        // Removing node B should clean up the exit from A
        state.remove_node(1);
        assert!(state.nodes()[0].location.exits.is_empty());
    }

    #[test]
    fn test_add_connection() {
        let mut state = ConnectionEditorState::new();
        state.add_node("a", "A", LocationType::Hub, NodePosition::new(0.0, 0.0));
        state.add_node("b", "B", LocationType::Town, NodePosition::new(200.0, 0.0));

        assert!(state.add_connection(0, 1, "exit_b", "from_a"));
        assert_eq!(state.nodes()[0].location.exits.len(), 1);
        assert_eq!(
            state.nodes()[0].location.exits[0].target_location,
            LocationId::from("b")
        );

        // Can't connect to self
        assert!(!state.add_connection(0, 0, "exit_self", "self"));
    }

    #[test]
    fn test_remove_connection() {
        let mut state = ConnectionEditorState::new();
        state.add_node("a", "A", LocationType::Hub, NodePosition::new(0.0, 0.0));
        state.add_node("b", "B", LocationType::Town, NodePosition::new(200.0, 0.0));
        state.add_connection(0, 1, "exit_b", "from_a");

        assert!(state.remove_connection(0, 0));
        assert!(state.nodes()[0].location.exits.is_empty());
        assert!(!state.remove_connection(0, 0)); // already removed
    }

    #[test]
    fn test_add_spawn_point() {
        let mut state = ConnectionEditorState::new();
        state.add_node("a", "A", LocationType::Hub, NodePosition::new(0.0, 0.0));
        assert!(state.add_spawn_point(0, "entrance", [5.0, 0.0, 3.0]));
        assert_eq!(state.nodes()[0].location.spawn_points.len(), 1);
        assert_eq!(state.nodes()[0].location.spawn_points[0].name, "entrance");
    }

    #[test]
    fn test_hit_test() {
        let mut state = ConnectionEditorState::new();
        state.add_node("a", "A", LocationType::Hub, NodePosition::new(100.0, 100.0));
        state.add_node("b", "B", LocationType::Town, NodePosition::new(400.0, 100.0));

        // Click inside node A
        assert_eq!(state.hit_test(150.0, 120.0, 200.0, 60.0), Some(0));
        // Click inside node B
        assert_eq!(state.hit_test(450.0, 120.0, 200.0, 60.0), Some(1));
        // Click outside both
        assert_eq!(state.hit_test(0.0, 0.0, 200.0, 60.0), None);
    }

    #[test]
    fn test_validate_valid_graph() {
        let mut state = ConnectionEditorState::new();
        state.load_from_ron(&sample_ron()).unwrap();
        let errors = state.validate();
        assert!(errors.is_empty(), "Expected no errors, got: {errors:?}");
    }

    #[test]
    fn test_validate_dangling_exit() {
        let mut state = ConnectionEditorState::new();
        state.add_node("a", "A", LocationType::Hub, NodePosition::new(0.0, 0.0));
        // Manually add an exit to a non-existent location
        state.nodes[0].location.exits.push(ExitDef {
            target_location: LocationId::from("nonexistent"),
            exit_node_name: "exit".to_string(),
            arrival_spawn: "spawn".to_string(),
        });

        let errors = state.validate();
        assert!(!errors.is_empty());
        assert!(errors[0].contains("nonexistent"));
    }

    #[test]
    fn test_export_ron() {
        let mut state = ConnectionEditorState::new();
        state.load_from_ron(&sample_ron()).unwrap();
        let exported = state.export_ron().unwrap();
        assert!(exported.contains("hub"));
        assert!(exported.contains("garden"));
        assert!(exported.contains("Hub Town"));
    }

    #[test]
    fn test_node_position() {
        let pos = NodePosition::new(100.0, 200.0);
        assert_eq!(pos.x, 100.0);
        assert_eq!(pos.y, 200.0);
    }

    #[test]
    fn test_zoom_and_pan() {
        let mut state = ConnectionEditorState::new();
        state.zoom = 2.0;
        state.pan_offset = NodePosition::new(50.0, 30.0);
        assert!((state.zoom - 2.0).abs() < f32::EPSILON);
        assert_eq!(state.pan_offset.x, 50.0);
        assert_eq!(state.pan_offset.y, 30.0);
    }

    #[test]
    fn test_selected_node_updates_on_remove() {
        let mut state = ConnectionEditorState::new();
        state.add_node("a", "A", LocationType::Hub, NodePosition::new(0.0, 0.0));
        state.add_node("b", "B", LocationType::Town, NodePosition::new(200.0, 0.0));
        state.add_node("c", "C", LocationType::Garden, NodePosition::new(400.0, 0.0));

        state.selected_node = Some(2);
        state.remove_node(1);
        // Selected node index should shift down
        assert_eq!(state.selected_node, Some(1));

        state.selected_node = Some(0);
        state.remove_node(0);
        // Selected was the removed node
        assert!(state.selected_node.is_none());
    }
}
