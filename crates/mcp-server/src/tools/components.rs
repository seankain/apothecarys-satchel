use apothecarys_tools::map_editor::ObjectComponent;

use crate::error::McpError;
use crate::state::ServerState;

pub fn add_component(
    state: &mut ServerState,
    location_id: &str,
    object_id: u32,
    component: ObjectComponent,
) -> Result<serde_json::Value, McpError> {
    let data = state.get_scene_mut(location_id)?;
    let obj = data
        .get_object_mut(object_id)
        .ok_or(McpError::ObjectNotFound {
            scene: location_id.to_string(),
            id: object_id,
        })?;
    obj.components.push(component);
    serde_json::to_value(&*obj).map_err(|e| McpError::SerializationError(e.to_string()))
}

pub fn remove_component(
    state: &mut ServerState,
    location_id: &str,
    object_id: u32,
    component_index: usize,
) -> Result<serde_json::Value, McpError> {
    let data = state.get_scene_mut(location_id)?;
    let obj = data
        .get_object_mut(object_id)
        .ok_or(McpError::ObjectNotFound {
            scene: location_id.to_string(),
            id: object_id,
        })?;
    if component_index >= obj.components.len() {
        return Err(McpError::InvalidParameter(format!(
            "Component index {component_index} out of bounds (object has {} components)",
            obj.components.len()
        )));
    }
    obj.components.remove(component_index);
    serde_json::to_value(&*obj).map_err(|e| McpError::SerializationError(e.to_string()))
}
