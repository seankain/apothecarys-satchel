use crate::error::McpError;
use crate::state::ServerState;

pub fn add_object(
    state: &mut ServerState,
    location_id: &str,
    asset_path: &str,
    position: [f32; 3],
) -> Result<serde_json::Value, McpError> {
    let data = state.get_scene_mut(location_id)?;
    let id = data.add_object(asset_path.to_string(), position);
    let obj = data.get_object(id).unwrap();
    Ok(serde_json::json!({
        "id": id,
        "asset_path": obj.asset_path,
        "position": obj.position,
        "rotation": obj.rotation,
        "scale": obj.scale,
    }))
}

pub fn remove_object(
    state: &mut ServerState,
    location_id: &str,
    object_id: u32,
) -> Result<serde_json::Value, McpError> {
    let data = state.get_scene_mut(location_id)?;
    let removed = data.remove_object(object_id);
    if !removed {
        return Err(McpError::ObjectNotFound {
            scene: location_id.to_string(),
            id: object_id,
        });
    }
    Ok(serde_json::json!({ "removed": true, "id": object_id }))
}

pub fn get_object(
    state: &ServerState,
    location_id: &str,
    object_id: u32,
) -> Result<serde_json::Value, McpError> {
    let data = state.get_scene(location_id)?;
    let obj = data.get_object(object_id).ok_or(McpError::ObjectNotFound {
        scene: location_id.to_string(),
        id: object_id,
    })?;
    serde_json::to_value(obj).map_err(|e| McpError::SerializationError(e.to_string()))
}

pub fn list_objects(
    state: &ServerState,
    location_id: &str,
) -> Result<serde_json::Value, McpError> {
    let data = state.get_scene(location_id)?;
    let summaries: Vec<serde_json::Value> = data
        .objects
        .iter()
        .map(|obj| {
            serde_json::json!({
                "id": obj.id,
                "asset_path": obj.asset_path,
                "position": obj.position,
                "component_count": obj.components.len(),
            })
        })
        .collect();
    Ok(serde_json::json!(summaries))
}
