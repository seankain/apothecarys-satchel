use std::sync::Arc;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ServerInfo};
use rmcp::schemars;
use rmcp::tool;
use rmcp::{tool_handler, tool_router};
use rmcp::{ErrorData, ServerHandler};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::state::ServerState;

#[derive(Clone)]
pub struct FyroxMcpServer {
    state: Arc<Mutex<ServerState>>,
    tool_router: ToolRouter<Self>,
}

// Parameter structs for tools

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateSceneParams {
    /// The unique location identifier for the new scene
    pub location_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LoadSceneParams {
    /// The location identifier of the scene to load
    pub location_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SaveSceneParams {
    /// The location identifier of the scene to save
    pub location_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct DeleteSceneParams {
    /// The location identifier of the scene to delete
    pub location_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddObjectParams {
    /// The location identifier of the loaded scene
    pub location_id: String,
    /// Path to the mesh asset relative to assets/ (e.g. "models/tree.glb")
    pub asset_path: String,
    /// World-space position [x, y, z]
    pub position: [f32; 3],
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ObjectIdParams {
    /// The location identifier of the loaded scene
    pub location_id: String,
    /// The unique object ID within the scene
    pub object_id: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListObjectsParams {
    /// The location identifier of the loaded scene
    pub location_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetTransformParams {
    /// The location identifier of the loaded scene
    pub location_id: String,
    /// The unique object ID within the scene
    pub object_id: u32,
    /// New position [x, y, z] (optional)
    pub position: Option<[f32; 3]>,
    /// New euler rotation in radians [x, y, z] (optional)
    pub rotation: Option<[f32; 3]>,
    /// New scale [x, y, z] (optional)
    pub scale: Option<[f32; 3]>,
    /// Grid size for position snapping (optional)
    pub snap_grid: Option<f32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddComponentParams {
    /// The location identifier of the loaded scene
    pub location_id: String,
    /// The unique object ID within the scene
    pub object_id: u32,
    /// The component to add as a JSON object. Supported variants:
    /// {"Interactable": {"interaction_type": "...", "display_name": "..."}},
    /// {"SpawnPoint": {"name": "...", "spawn_type": "..."}},
    /// {"ExitTrigger": {"target_location": "...", "arrival_spawn": "..."}}
    pub component: serde_json::Value,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RemoveComponentParams {
    /// The location identifier of the loaded scene
    pub location_id: String,
    /// The unique object ID within the scene
    pub object_id: u32,
    /// The index of the component to remove
    pub component_index: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListAssetsParams {
    /// Optional subdirectory within assets/ to scan
    pub subdirectory: Option<String>,
}

/// Helper to create a success result with JSON text content
fn ok_result(value: &serde_json::Value) -> CallToolResult {
    CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string()),
    )])
}

/// Helper to create an error result
fn err_result(err: &crate::error::McpError) -> CallToolResult {
    CallToolResult::error(vec![Content::text(err.to_string())])
}

impl FyroxMcpServer {
    pub fn new(state: ServerState) -> Self {
        let state = Arc::new(Mutex::new(state));
        let tool_router = Self::tool_router();
        Self {
            state,
            tool_router,
        }
    }
}

#[tool_router]
impl FyroxMcpServer {
    #[tool(description = "Create a new empty scene/placement file for a location")]
    async fn create_scene(
        &self,
        params: Parameters<CreateSceneParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        match crate::tools::scene::create_scene(&mut state, &params.0.location_id) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "List all placement files (.placement.ron) in the data directory")]
    async fn list_scenes(&self) -> Result<CallToolResult, ErrorData> {
        let state = self.state.lock().await;
        match crate::tools::scene::list_scenes(&state) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "Load a scene into memory for editing")]
    async fn load_scene(
        &self,
        params: Parameters<LoadSceneParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        match crate::tools::scene::load_scene(&mut state, &params.0.location_id) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "Save an in-memory scene to its RON file on disk")]
    async fn save_scene(
        &self,
        params: Parameters<SaveSceneParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let state = self.state.lock().await;
        match crate::tools::scene::save_scene(&state, &params.0.location_id) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "Delete a placement file from disk")]
    async fn delete_scene(
        &self,
        params: Parameters<DeleteSceneParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        match crate::tools::scene::delete_scene(&mut state, &params.0.location_id) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "Add a new object to a loaded scene with an asset path and position")]
    async fn add_object(
        &self,
        params: Parameters<AddObjectParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        match crate::tools::objects::add_object(
            &mut state,
            &params.0.location_id,
            &params.0.asset_path,
            params.0.position,
        ) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "Remove an object from a loaded scene by its ID")]
    async fn remove_object(
        &self,
        params: Parameters<ObjectIdParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        match crate::tools::objects::remove_object(
            &mut state,
            &params.0.location_id,
            params.0.object_id,
        ) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "Get the full details of a specific object in a loaded scene")]
    async fn get_object(
        &self,
        params: Parameters<ObjectIdParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let state = self.state.lock().await;
        match crate::tools::objects::get_object(&state, &params.0.location_id, params.0.object_id) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "List all objects in a loaded scene with summary info")]
    async fn list_objects(
        &self,
        params: Parameters<ListObjectsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let state = self.state.lock().await;
        match crate::tools::objects::list_objects(&state, &params.0.location_id) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(
        description = "Set position, rotation, and/or scale on an object. Each transform field is optional."
    )]
    async fn set_transform(
        &self,
        params: Parameters<SetTransformParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        let p = params.0;
        match crate::tools::transforms::set_transform(
            &mut state,
            &p.location_id,
            p.object_id,
            p.position,
            p.rotation,
            p.scale,
            p.snap_grid,
        ) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(
        description = "Add a gameplay component to an object. Component must be one of: {\"Interactable\": {\"interaction_type\": \"...\", \"display_name\": \"...\"}}, {\"SpawnPoint\": {\"name\": \"...\", \"spawn_type\": \"...\"}}, or {\"ExitTrigger\": {\"target_location\": \"...\", \"arrival_spawn\": \"...\"}}"
    )]
    async fn add_component(
        &self,
        params: Parameters<AddComponentParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let p = params.0;
        // Deserialize component from JSON value
        let component: apothecarys_tools::map_editor::ObjectComponent =
            match serde_json::from_value(p.component) {
                Ok(c) => c,
                Err(e) => {
                    return Ok(CallToolResult::error(vec![Content::text(format!(
                        "Invalid component: {e}"
                    ))]));
                }
            };
        let mut state = self.state.lock().await;
        match crate::tools::components::add_component(
            &mut state,
            &p.location_id,
            p.object_id,
            component,
        ) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "Remove a component from an object by its index")]
    async fn remove_component(
        &self,
        params: Parameters<RemoveComponentParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut state = self.state.lock().await;
        match crate::tools::components::remove_component(
            &mut state,
            &params.0.location_id,
            params.0.object_id,
            params.0.component_index,
        ) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }

    #[tool(description = "Scan the assets directory for available model files (.glb, .gltf, .fbx)")]
    async fn list_assets(
        &self,
        params: Parameters<ListAssetsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let state = self.state.lock().await;
        match crate::tools::assets::list_assets(&state, params.0.subdirectory.as_deref()) {
            Ok(v) => Ok(ok_result(&v)),
            Err(e) => Ok(err_result(&e)),
        }
    }
}

#[tool_handler]
impl ServerHandler for FyroxMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::default()
            .with_instructions(
                "Fyrox scene editor MCP server for The Apothecary's Satchel. \
                 Create, load, edit, and save scene placement files (.placement.ron). \
                 Objects can be placed with 3D transforms and annotated with gameplay components.",
            )
    }
}
