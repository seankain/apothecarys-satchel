use apothecarys_tools::map_editor::snap_to_grid;

use crate::error::McpError;
use crate::state::ServerState;

pub fn set_transform(
    state: &mut ServerState,
    location_id: &str,
    object_id: u32,
    position: Option<[f32; 3]>,
    rotation: Option<[f32; 3]>,
    scale: Option<[f32; 3]>,
    snap_grid: Option<f32>,
) -> Result<serde_json::Value, McpError> {
    let data = state.get_scene_mut(location_id)?;
    let obj = data
        .get_object_mut(object_id)
        .ok_or(McpError::ObjectNotFound {
            scene: location_id.to_string(),
            id: object_id,
        })?;

    if let Some(mut pos) = position {
        if let Some(grid) = snap_grid {
            pos = snap_to_grid(pos, grid);
        }
        obj.position = pos;
    }
    if let Some(rot) = rotation {
        obj.rotation = rot;
    }
    if let Some(sc) = scale {
        obj.scale = sc;
    }

    serde_json::to_value(&*obj).map_err(|e| McpError::SerializationError(e.to_string()))
}
