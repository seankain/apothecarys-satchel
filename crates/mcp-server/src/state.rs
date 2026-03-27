use apothecarys_tools::map_editor::PlacementData;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::McpError;

pub struct ServerState {
    pub project_root: PathBuf,
    pub placements_dir: PathBuf,
    pub assets_dir: PathBuf,
    pub scenes: HashMap<String, PlacementData>,
}

impl ServerState {
    pub fn new(project_root: PathBuf) -> std::io::Result<Self> {
        let placements_dir = project_root.join("data").join("placements");
        let assets_dir = project_root.join("assets");
        std::fs::create_dir_all(&placements_dir)?;
        Ok(Self {
            project_root,
            placements_dir,
            assets_dir,
            scenes: HashMap::new(),
        })
    }

    pub fn get_scene(&self, location_id: &str) -> Result<&PlacementData, McpError> {
        self.scenes
            .get(location_id)
            .ok_or_else(|| McpError::SceneNotLoaded(location_id.to_string()))
    }

    pub fn get_scene_mut(&mut self, location_id: &str) -> Result<&mut PlacementData, McpError> {
        self.scenes
            .get_mut(location_id)
            .ok_or_else(|| McpError::SceneNotLoaded(location_id.to_string()))
    }

    pub fn placement_path(&self, location_id: &str) -> PathBuf {
        self.placements_dir
            .join(format!("{location_id}.placement.ron"))
    }
}
