use serde::{Deserialize, Serialize};

/// Top-level game configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GameConfig {
    pub window_title: String,
    pub window_width: u32,
    pub window_height: u32,
    pub target_fps: u32,
    pub vsync: bool,
    pub shadow_quality: ShadowQuality,
    pub ssao_enabled: bool,
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShadowQuality {
    Off,
    Low,
    Medium,
    High,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            window_title: "The Apothecary's Satchel".to_string(),
            window_width: 1280,
            window_height: 720,
            target_fps: 60,
            vsync: true,
            shadow_quality: ShadowQuality::Medium,
            ssao_enabled: true,
            master_volume: 1.0,
            music_volume: 0.7,
            sfx_volume: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = GameConfig::default();
        assert_eq!(config.window_title, "The Apothecary's Satchel");
        assert_eq!(config.target_fps, 60);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = GameConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: GameConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }
}
