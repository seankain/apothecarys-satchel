use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;

/// A placed object in the map editor scene.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlacedObject {
    /// Unique identifier within the placement file.
    pub id: u32,
    /// Path to the mesh asset relative to assets/.
    pub asset_path: String,
    /// World-space position [x, y, z].
    pub position: [f32; 3],
    /// Euler rotation in radians [x, y, z].
    pub rotation: [f32; 3],
    /// Scale factor [x, y, z].
    pub scale: [f32; 3],
    /// Optional component annotations for gameplay systems.
    pub components: Vec<ObjectComponent>,
}

/// A component attached to a placed object for gameplay purposes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ObjectComponent {
    /// This object is interactable (NPC, item, exit, etc.).
    Interactable {
        interaction_type: String,
        display_name: String,
    },
    /// This object is a spawn point.
    SpawnPoint {
        name: String,
        spawn_type: String,
    },
    /// This object is an exit trigger to another location.
    ExitTrigger {
        target_location: String,
        arrival_spawn: String,
    },
}

/// A complete placement file describing all objects in a scene.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlacementData {
    /// Location identifier this placement belongs to.
    pub location_id: String,
    /// All placed objects.
    pub objects: Vec<PlacedObject>,
    /// Next available object ID.
    pub next_id: u32,
}

impl PlacementData {
    /// Create a new empty placement for a location.
    pub fn new(location_id: impl Into<String>) -> Self {
        Self {
            location_id: location_id.into(),
            objects: Vec::new(),
            next_id: 1,
        }
    }

    /// Add an object and return its assigned ID.
    pub fn add_object(&mut self, asset_path: String, position: [f32; 3]) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.objects.push(PlacedObject {
            id,
            asset_path,
            position,
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            components: Vec::new(),
        });
        id
    }

    /// Remove an object by ID. Returns true if found and removed.
    pub fn remove_object(&mut self, id: u32) -> bool {
        let len_before = self.objects.len();
        self.objects.retain(|obj| obj.id != id);
        self.objects.len() < len_before
    }

    /// Find a mutable reference to an object by ID.
    pub fn get_object_mut(&mut self, id: u32) -> Option<&mut PlacedObject> {
        self.objects.iter_mut().find(|obj| obj.id == id)
    }

    /// Find an object by ID.
    pub fn get_object(&self, id: u32) -> Option<&PlacedObject> {
        self.objects.iter().find(|obj| obj.id == id)
    }

    /// Save placement data to a RON file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), MapEditorError> {
        let ron_str = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| MapEditorError::Serialize(e.to_string()))?;
        std::fs::write(path, ron_str)?;
        Ok(())
    }

    /// Load placement data from a RON file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, MapEditorError> {
        let contents = std::fs::read_to_string(path)?;
        let data: PlacementData =
            ron::from_str(&contents).map_err(|e| MapEditorError::Deserialize(e.to_string()))?;
        Ok(data)
    }
}

/// Snap a position to a grid.
pub fn snap_to_grid(position: [f32; 3], grid_size: f32) -> [f32; 3] {
    [
        (position[0] / grid_size).round() * grid_size,
        (position[1] / grid_size).round() * grid_size,
        (position[2] / grid_size).round() * grid_size,
    ]
}

/// Errors for the map editor.
#[derive(Debug)]
pub enum MapEditorError {
    Io(std::io::Error),
    Serialize(String),
    Deserialize(String),
}

impl std::fmt::Display for MapEditorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapEditorError::Io(e) => write!(f, "IO error: {e}"),
            MapEditorError::Serialize(e) => write!(f, "Serialize error: {e}"),
            MapEditorError::Deserialize(e) => write!(f, "Deserialize error: {e}"),
        }
    }
}

impl std::error::Error for MapEditorError {}

impl From<std::io::Error> for MapEditorError {
    fn from(e: std::io::Error) -> Self {
        MapEditorError::Io(e)
    }
}

/// An action that can be undone/redone in the editor.
#[derive(Debug, Clone)]
pub enum EditorAction {
    AddObject {
        object: PlacedObject,
    },
    RemoveObject {
        object: PlacedObject,
    },
    MoveObject {
        id: u32,
        old_position: [f32; 3],
        new_position: [f32; 3],
    },
    RotateObject {
        id: u32,
        old_rotation: [f32; 3],
        new_rotation: [f32; 3],
    },
    ScaleObject {
        id: u32,
        old_scale: [f32; 3],
        new_scale: [f32; 3],
    },
}

/// Undo/redo stack using the command pattern.
#[derive(Debug)]
pub struct UndoStack {
    undo_stack: VecDeque<EditorAction>,
    redo_stack: Vec<EditorAction>,
    max_history: usize,
}

impl UndoStack {
    pub fn new(max_history: usize) -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            max_history,
        }
    }

    /// Push an action onto the undo stack, clearing the redo stack.
    pub fn push(&mut self, action: EditorAction) {
        self.redo_stack.clear();
        self.undo_stack.push_back(action);
        while self.undo_stack.len() > self.max_history {
            self.undo_stack.pop_front();
        }
    }

    /// Pop the last action for undo. Returns None if nothing to undo.
    pub fn undo(&mut self) -> Option<EditorAction> {
        let action = self.undo_stack.pop_back()?;
        self.redo_stack.push(action.clone());
        Some(action)
    }

    /// Pop the last undone action for redo. Returns None if nothing to redo.
    pub fn redo(&mut self) -> Option<EditorAction> {
        let action = self.redo_stack.pop()?;
        self.undo_stack.push_back(action.clone());
        Some(action)
    }

    /// Whether there are actions available to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there are actions available to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

/// Apply an editor action forward on placement data.
pub fn apply_action(data: &mut PlacementData, action: &EditorAction) {
    match action {
        EditorAction::AddObject { object } => {
            data.objects.push(object.clone());
            if object.id >= data.next_id {
                data.next_id = object.id + 1;
            }
        }
        EditorAction::RemoveObject { object } => {
            data.objects.retain(|o| o.id != object.id);
        }
        EditorAction::MoveObject {
            id, new_position, ..
        } => {
            if let Some(obj) = data.get_object_mut(*id) {
                obj.position = *new_position;
            }
        }
        EditorAction::RotateObject {
            id, new_rotation, ..
        } => {
            if let Some(obj) = data.get_object_mut(*id) {
                obj.rotation = *new_rotation;
            }
        }
        EditorAction::ScaleObject { id, new_scale, .. } => {
            if let Some(obj) = data.get_object_mut(*id) {
                obj.scale = *new_scale;
            }
        }
    }
}

/// Apply an editor action in reverse (undo).
pub fn reverse_action(data: &mut PlacementData, action: &EditorAction) {
    match action {
        EditorAction::AddObject { object } => {
            data.objects.retain(|o| o.id != object.id);
        }
        EditorAction::RemoveObject { object } => {
            data.objects.push(object.clone());
        }
        EditorAction::MoveObject {
            id, old_position, ..
        } => {
            if let Some(obj) = data.get_object_mut(*id) {
                obj.position = *old_position;
            }
        }
        EditorAction::RotateObject {
            id, old_rotation, ..
        } => {
            if let Some(obj) = data.get_object_mut(*id) {
                obj.rotation = *old_rotation;
            }
        }
        EditorAction::ScaleObject { id, old_scale, .. } => {
            if let Some(obj) = data.get_object_mut(*id) {
                obj.scale = *old_scale;
            }
        }
    }
}

/// Scan a directory for model assets (.glb, .fbx, .gltf).
pub fn scan_assets(dir: impl AsRef<Path>) -> Vec<String> {
    let mut assets = Vec::new();
    scan_assets_recursive(dir.as_ref(), dir.as_ref(), &mut assets);
    assets.sort();
    assets
}

fn scan_assets_recursive(base: &Path, dir: &Path, assets: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_assets_recursive(base, &path, assets);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if ext_lower == "glb" || ext_lower == "gltf" || ext_lower == "fbx" {
                if let Ok(rel) = path.strip_prefix(base) {
                    assets.push(rel.to_string_lossy().to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placement_data_new() {
        let data = PlacementData::new("hub_town");
        assert_eq!(data.location_id, "hub_town");
        assert!(data.objects.is_empty());
        assert_eq!(data.next_id, 1);
    }

    #[test]
    fn test_add_and_remove_object() {
        let mut data = PlacementData::new("test");
        let id1 = data.add_object("model.glb".to_string(), [1.0, 0.0, 2.0]);
        let id2 = data.add_object("tree.glb".to_string(), [5.0, 0.0, 5.0]);

        assert_eq!(data.objects.len(), 2);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        assert!(data.remove_object(id1));
        assert_eq!(data.objects.len(), 1);
        assert_eq!(data.objects[0].id, id2);

        assert!(!data.remove_object(999));
    }

    #[test]
    fn test_get_object() {
        let mut data = PlacementData::new("test");
        let id = data.add_object("rock.glb".to_string(), [3.0, 0.0, 4.0]);

        let obj = data.get_object(id).unwrap();
        assert_eq!(obj.asset_path, "rock.glb");
        assert_eq!(obj.position, [3.0, 0.0, 4.0]);
        assert_eq!(obj.scale, [1.0, 1.0, 1.0]);

        assert!(data.get_object(999).is_none());
    }

    #[test]
    fn test_modify_object() {
        let mut data = PlacementData::new("test");
        let id = data.add_object("model.glb".to_string(), [0.0, 0.0, 0.0]);

        let obj = data.get_object_mut(id).unwrap();
        obj.position = [10.0, 0.0, 10.0];
        obj.rotation = [0.0, 1.57, 0.0];
        obj.scale = [2.0, 2.0, 2.0];

        let obj = data.get_object(id).unwrap();
        assert_eq!(obj.position, [10.0, 0.0, 10.0]);
        assert_eq!(obj.rotation, [0.0, 1.57, 0.0]);
        assert_eq!(obj.scale, [2.0, 2.0, 2.0]);
    }

    #[test]
    fn test_object_components() {
        let mut data = PlacementData::new("test");
        let id = data.add_object("npc.glb".to_string(), [0.0, 0.0, 0.0]);

        let obj = data.get_object_mut(id).unwrap();
        obj.components.push(ObjectComponent::Interactable {
            interaction_type: "npc".to_string(),
            display_name: "Herbalist".to_string(),
        });
        obj.components.push(ObjectComponent::SpawnPoint {
            name: "npc_spawn".to_string(),
            spawn_type: "Npc".to_string(),
        });

        assert_eq!(obj.components.len(), 2);
    }

    #[test]
    fn test_snap_to_grid() {
        assert_eq!(snap_to_grid([0.3, 0.7, 1.2], 0.5), [0.5, 0.5, 1.0]);
        assert_eq!(snap_to_grid([1.0, 2.0, 3.0], 1.0), [1.0, 2.0, 3.0]);
        assert_eq!(snap_to_grid([0.24, 0.0, 0.76], 0.5), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_undo_stack() {
        let mut stack = UndoStack::new(10);
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());

        stack.push(EditorAction::MoveObject {
            id: 1,
            old_position: [0.0, 0.0, 0.0],
            new_position: [5.0, 0.0, 5.0],
        });
        assert!(stack.can_undo());

        let action = stack.undo().unwrap();
        assert!(matches!(action, EditorAction::MoveObject { .. }));
        assert!(!stack.can_undo());
        assert!(stack.can_redo());

        let action = stack.redo().unwrap();
        assert!(matches!(action, EditorAction::MoveObject { .. }));
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_undo_clears_redo_on_new_action() {
        let mut stack = UndoStack::new(10);
        stack.push(EditorAction::MoveObject {
            id: 1,
            old_position: [0.0, 0.0, 0.0],
            new_position: [5.0, 0.0, 5.0],
        });
        stack.undo();
        assert!(stack.can_redo());

        // New action should clear redo
        stack.push(EditorAction::MoveObject {
            id: 2,
            old_position: [0.0, 0.0, 0.0],
            new_position: [1.0, 0.0, 1.0],
        });
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_undo_max_history() {
        let mut stack = UndoStack::new(3);
        for i in 0..5 {
            stack.push(EditorAction::MoveObject {
                id: i,
                old_position: [0.0, 0.0, 0.0],
                new_position: [i as f32, 0.0, 0.0],
            });
        }
        // Only 3 most recent should be in history
        let mut count = 0;
        while stack.undo().is_some() {
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn test_apply_and_reverse_action() {
        let mut data = PlacementData::new("test");

        // Test add
        let add_action = EditorAction::AddObject {
            object: PlacedObject {
                id: 1,
                asset_path: "tree.glb".to_string(),
                position: [5.0, 0.0, 5.0],
                rotation: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                components: Vec::new(),
            },
        };
        apply_action(&mut data, &add_action);
        assert_eq!(data.objects.len(), 1);

        reverse_action(&mut data, &add_action);
        assert!(data.objects.is_empty());

        // Test move
        apply_action(&mut data, &add_action);
        let move_action = EditorAction::MoveObject {
            id: 1,
            old_position: [5.0, 0.0, 5.0],
            new_position: [10.0, 0.0, 10.0],
        };
        apply_action(&mut data, &move_action);
        assert_eq!(data.get_object(1).unwrap().position, [10.0, 0.0, 10.0]);

        reverse_action(&mut data, &move_action);
        assert_eq!(data.get_object(1).unwrap().position, [5.0, 0.0, 5.0]);
    }

    #[test]
    fn test_placement_ron_roundtrip() {
        let mut data = PlacementData::new("hub_town");
        let id = data.add_object("house.glb".to_string(), [10.0, 0.0, 5.0]);
        let obj = data.get_object_mut(id).unwrap();
        obj.rotation = [0.0, std::f32::consts::FRAC_PI_2, 0.0];
        obj.components.push(ObjectComponent::ExitTrigger {
            target_location: "garden".to_string(),
            arrival_spawn: "from_hub".to_string(),
        });

        let ron_str =
            ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default()).unwrap();
        let loaded: PlacementData = ron::from_str(&ron_str).unwrap();

        assert_eq!(data, loaded);
    }

    #[test]
    fn test_scan_assets_empty_dir() {
        // Scanning a non-existent directory should return empty list
        let assets = scan_assets("/tmp/nonexistent_apothecarys_test_dir");
        assert!(assets.is_empty());
    }
}
