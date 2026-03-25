use apothecarys_tools::map_editor::PlacementData;

use crate::error::McpError;
use crate::state::ServerState;

pub fn create_scene(state: &mut ServerState, location_id: &str) -> Result<serde_json::Value, McpError> {
    let data = PlacementData::new(location_id);
    let path = state.placement_path(location_id);
    data.save(&path)?;
    state.scenes.insert(location_id.to_string(), data);
    Ok(serde_json::json!({
        "location_id": location_id,
        "path": path.to_string_lossy(),
        "object_count": 0
    }))
}

pub fn list_scenes(state: &ServerState) -> Result<serde_json::Value, McpError> {
    let mut scenes = Vec::new();
    let entries = std::fs::read_dir(&state.placements_dir)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.ends_with(".placement.ron") {
                let location_id = name.trim_end_matches(".placement.ron");
                let object_count = if let Some(data) = state.scenes.get(location_id) {
                    data.objects.len()
                } else if let Ok(data) = PlacementData::load(&path) {
                    data.objects.len()
                } else {
                    0
                };
                scenes.push(serde_json::json!({
                    "location_id": location_id,
                    "path": path.to_string_lossy(),
                    "object_count": object_count
                }));
            }
        }
    }
    Ok(serde_json::json!(scenes))
}

pub fn load_scene(state: &mut ServerState, location_id: &str) -> Result<serde_json::Value, McpError> {
    let path = state.placement_path(location_id);
    let data = PlacementData::load(&path)?;
    let json = serde_json::to_value(&data)
        .map_err(|e| McpError::SerializationError(e.to_string()))?;
    state.scenes.insert(location_id.to_string(), data);
    Ok(json)
}

pub fn save_scene(state: &ServerState, location_id: &str) -> Result<serde_json::Value, McpError> {
    let data = state.get_scene(location_id)?;
    let path = state.placement_path(location_id);
    data.save(&path)?;
    Ok(serde_json::json!({
        "location_id": location_id,
        "path": path.to_string_lossy(),
        "saved": true
    }))
}

pub fn delete_scene(state: &mut ServerState, location_id: &str) -> Result<serde_json::Value, McpError> {
    let path = state.placement_path(location_id);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    state.scenes.remove(location_id);
    Ok(serde_json::json!({
        "location_id": location_id,
        "deleted": true
    }))
}
