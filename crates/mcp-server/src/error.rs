use apothecarys_tools::map_editor::MapEditorError;
use std::fmt;

#[derive(Debug)]
pub enum McpError {
    SceneNotLoaded(String),
    ObjectNotFound { scene: String, id: u32 },
    IoError(std::io::Error),
    SerializationError(String),
    InvalidParameter(String),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::SceneNotLoaded(id) => write!(f, "Scene not loaded: {id}"),
            McpError::ObjectNotFound { scene, id } => {
                write!(f, "Object {id} not found in scene {scene}")
            }
            McpError::IoError(e) => write!(f, "IO error: {e}"),
            McpError::SerializationError(e) => write!(f, "Serialization error: {e}"),
            McpError::InvalidParameter(e) => write!(f, "Invalid parameter: {e}"),
        }
    }
}

impl std::error::Error for McpError {}

impl From<MapEditorError> for McpError {
    fn from(e: MapEditorError) -> Self {
        match e {
            MapEditorError::Io(io) => McpError::IoError(io),
            MapEditorError::Serialize(s) => McpError::SerializationError(s),
            MapEditorError::Deserialize(s) => McpError::SerializationError(s),
        }
    }
}

impl From<std::io::Error> for McpError {
    fn from(e: std::io::Error) -> Self {
        McpError::IoError(e)
    }
}
