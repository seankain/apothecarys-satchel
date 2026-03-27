use apothecarys_tools::map_editor::scan_assets;

use crate::error::McpError;
use crate::state::ServerState;

pub fn list_assets(
    state: &ServerState,
    subdirectory: Option<&str>,
) -> Result<serde_json::Value, McpError> {
    let dir = match subdirectory {
        Some(sub) => state.assets_dir.join(sub),
        None => state.assets_dir.clone(),
    };
    let assets = scan_assets(&dir);
    Ok(serde_json::json!(assets))
}
