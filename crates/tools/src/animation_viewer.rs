/// Animation playback state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Information about a single animation clip.
#[derive(Debug, Clone)]
pub struct AnimationClipInfo {
    /// Display name of the clip.
    pub name: String,
    /// Duration in seconds.
    pub duration: f32,
}

/// Configuration for the orbit camera.
#[derive(Debug, Clone)]
pub struct OrbitCameraConfig {
    /// Distance from the target.
    pub distance: f32,
    /// Horizontal angle (radians).
    pub yaw: f32,
    /// Vertical angle (radians).
    pub pitch: f32,
    /// Target point to orbit around.
    pub target: [f32; 3],
    /// Minimum distance.
    pub min_distance: f32,
    /// Maximum distance.
    pub max_distance: f32,
}

impl Default for OrbitCameraConfig {
    fn default() -> Self {
        Self {
            distance: 5.0,
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: 0.3,
            target: [0.0, 1.0, 0.0],
            min_distance: 1.0,
            max_distance: 50.0,
        }
    }
}

impl OrbitCameraConfig {
    /// Calculate the camera position from orbit parameters.
    pub fn calculate_position(&self) -> [f32; 3] {
        let cos_pitch = self.pitch.cos();
        let sin_pitch = self.pitch.sin();
        let cos_yaw = self.yaw.cos();
        let sin_yaw = self.yaw.sin();

        [
            self.target[0] + self.distance * cos_pitch * sin_yaw,
            self.target[1] + self.distance * sin_pitch,
            self.target[2] + self.distance * cos_pitch * cos_yaw,
        ]
    }

    /// Rotate the camera by delta angles.
    pub fn rotate(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(-1.4, 1.4);
    }

    /// Zoom the camera by a delta distance.
    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta).clamp(self.min_distance, self.max_distance);
    }
}

/// State for the animation viewer tool.
#[derive(Debug)]
pub struct AnimationViewerState {
    /// Currently loaded model path, if any.
    pub model_path: Option<String>,
    /// Available animation clips from the loaded model.
    pub clips: Vec<AnimationClipInfo>,
    /// Index of the currently selected/playing clip.
    pub selected_clip: Option<usize>,
    /// Current playback state.
    pub playback_state: PlaybackState,
    /// Playback speed multiplier.
    pub playback_speed: f32,
    /// Whether to loop the animation.
    pub looping: bool,
    /// Current playback time in seconds.
    pub current_time: f32,
    /// Orbit camera configuration.
    pub camera: OrbitCameraConfig,
}

impl AnimationViewerState {
    pub fn new() -> Self {
        Self {
            model_path: None,
            clips: Vec::new(),
            selected_clip: None,
            playback_state: PlaybackState::Stopped,
            playback_speed: 1.0,
            looping: true,
            current_time: 0.0,
            camera: OrbitCameraConfig::default(),
        }
    }

    /// Set the loaded model info.
    pub fn set_model(&mut self, path: String, clips: Vec<AnimationClipInfo>) {
        self.model_path = Some(path);
        self.clips = clips;
        self.selected_clip = if self.clips.is_empty() { None } else { Some(0) };
        self.playback_state = PlaybackState::Stopped;
        self.current_time = 0.0;
    }

    /// Select a clip by index and start playing.
    pub fn select_and_play(&mut self, index: usize) -> bool {
        if index >= self.clips.len() {
            return false;
        }
        self.selected_clip = Some(index);
        self.current_time = 0.0;
        self.playback_state = PlaybackState::Playing;
        true
    }

    /// Toggle play/pause.
    pub fn toggle_playback(&mut self) {
        self.playback_state = match self.playback_state {
            PlaybackState::Playing => PlaybackState::Paused,
            PlaybackState::Paused | PlaybackState::Stopped => PlaybackState::Playing,
        };
    }

    /// Stop playback and reset time.
    pub fn stop(&mut self) {
        self.playback_state = PlaybackState::Stopped;
        self.current_time = 0.0;
    }

    /// Update playback time by delta seconds. Returns true if time changed.
    pub fn update(&mut self, dt: f32) -> bool {
        if self.playback_state != PlaybackState::Playing {
            return false;
        }

        let clip_index = match self.selected_clip {
            Some(i) => i,
            None => return false,
        };

        let duration = self.clips[clip_index].duration;
        if duration <= 0.0 {
            return false;
        }

        self.current_time += dt * self.playback_speed;

        if self.current_time >= duration {
            if self.looping {
                self.current_time %= duration;
            } else {
                self.current_time = duration;
                self.playback_state = PlaybackState::Stopped;
            }
        }

        true
    }

    /// Set playback time (scrubbing). Clamps to clip duration.
    pub fn set_time(&mut self, time: f32) {
        if let Some(clip_index) = self.selected_clip {
            let duration = self.clips[clip_index].duration;
            self.current_time = time.clamp(0.0, duration);
        }
    }

    /// Get the current clip's duration, if any.
    pub fn current_clip_duration(&self) -> Option<f32> {
        self.selected_clip.map(|i| self.clips[i].duration)
    }

    /// Get the current playback progress as a fraction [0.0, 1.0].
    pub fn progress(&self) -> f32 {
        match self.selected_clip {
            Some(i) => {
                let duration = self.clips[i].duration;
                if duration > 0.0 {
                    self.current_time / duration
                } else {
                    0.0
                }
            }
            None => 0.0,
        }
    }
}

impl Default for AnimationViewerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_clips() -> Vec<AnimationClipInfo> {
        vec![
            AnimationClipInfo {
                name: "idle".to_string(),
                duration: 2.0,
            },
            AnimationClipInfo {
                name: "walk".to_string(),
                duration: 1.0,
            },
            AnimationClipInfo {
                name: "attack".to_string(),
                duration: 0.5,
            },
        ]
    }

    #[test]
    fn test_new_state() {
        let state = AnimationViewerState::new();
        assert!(state.model_path.is_none());
        assert!(state.clips.is_empty());
        assert!(state.selected_clip.is_none());
        assert_eq!(state.playback_state, PlaybackState::Stopped);
        assert!((state.playback_speed - 1.0).abs() < f32::EPSILON);
        assert!(state.looping);
    }

    #[test]
    fn test_set_model() {
        let mut state = AnimationViewerState::new();
        state.set_model("character.glb".to_string(), sample_clips());

        assert_eq!(state.model_path.as_deref(), Some("character.glb"));
        assert_eq!(state.clips.len(), 3);
        assert_eq!(state.selected_clip, Some(0));
        assert_eq!(state.playback_state, PlaybackState::Stopped);
    }

    #[test]
    fn test_set_model_empty_clips() {
        let mut state = AnimationViewerState::new();
        state.set_model("static.glb".to_string(), Vec::new());
        assert!(state.selected_clip.is_none());
    }

    #[test]
    fn test_select_and_play() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());

        assert!(state.select_and_play(1));
        assert_eq!(state.selected_clip, Some(1));
        assert_eq!(state.playback_state, PlaybackState::Playing);
        assert_eq!(state.current_time, 0.0);

        assert!(!state.select_and_play(99));
    }

    #[test]
    fn test_toggle_playback() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());

        state.toggle_playback();
        assert_eq!(state.playback_state, PlaybackState::Playing);

        state.toggle_playback();
        assert_eq!(state.playback_state, PlaybackState::Paused);

        state.toggle_playback();
        assert_eq!(state.playback_state, PlaybackState::Playing);
    }

    #[test]
    fn test_stop() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());
        state.select_and_play(0);
        state.update(1.0);

        state.stop();
        assert_eq!(state.playback_state, PlaybackState::Stopped);
        assert_eq!(state.current_time, 0.0);
    }

    #[test]
    fn test_update_playing() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());
        state.select_and_play(0); // idle: 2.0s duration

        assert!(state.update(0.5));
        assert!((state.current_time - 0.5).abs() < f32::EPSILON);

        assert!(state.update(0.5));
        assert!((state.current_time - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_update_looping() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());
        state.select_and_play(1); // walk: 1.0s duration
        state.looping = true;

        state.update(1.5);
        assert!(state.current_time < 1.0);
        assert_eq!(state.playback_state, PlaybackState::Playing);
    }

    #[test]
    fn test_update_no_loop() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());
        state.select_and_play(2); // attack: 0.5s duration
        state.looping = false;

        state.update(1.0);
        assert!((state.current_time - 0.5).abs() < f32::EPSILON);
        assert_eq!(state.playback_state, PlaybackState::Stopped);
    }

    #[test]
    fn test_update_paused() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());
        state.select_and_play(0);
        state.playback_state = PlaybackState::Paused;

        assert!(!state.update(1.0));
        assert_eq!(state.current_time, 0.0);
    }

    #[test]
    fn test_set_time() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());
        state.selected_clip = Some(0); // idle: 2.0s

        state.set_time(1.5);
        assert!((state.current_time - 1.5).abs() < f32::EPSILON);

        state.set_time(10.0); // clamp
        assert!((state.current_time - 2.0).abs() < f32::EPSILON);

        state.set_time(-1.0); // clamp
        assert_eq!(state.current_time, 0.0);
    }

    #[test]
    fn test_progress() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());
        state.selected_clip = Some(0); // idle: 2.0s

        assert_eq!(state.progress(), 0.0);

        state.current_time = 1.0;
        assert!((state.progress() - 0.5).abs() < f32::EPSILON);

        state.current_time = 2.0;
        assert!((state.progress() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_progress_no_clip() {
        let state = AnimationViewerState::new();
        assert_eq!(state.progress(), 0.0);
    }

    #[test]
    fn test_playback_speed() {
        let mut state = AnimationViewerState::new();
        state.set_model("char.glb".to_string(), sample_clips());
        state.select_and_play(1); // walk: 1.0s
        state.playback_speed = 2.0;

        state.update(0.25);
        assert!((state.current_time - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_orbit_camera_position() {
        let camera = OrbitCameraConfig::default();
        let pos = camera.calculate_position();
        // With default params, camera should be offset from target
        assert!(pos[0] != camera.target[0] || pos[1] != camera.target[1] || pos[2] != camera.target[2]);
    }

    #[test]
    fn test_orbit_camera_rotate() {
        let mut camera = OrbitCameraConfig::default();
        let initial_yaw = camera.yaw;
        camera.rotate(0.5, 0.0);
        assert!((camera.yaw - initial_yaw - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_orbit_camera_pitch_clamp() {
        let mut camera = OrbitCameraConfig::default();
        camera.rotate(0.0, 10.0);
        assert!(camera.pitch <= 1.4);
        camera.rotate(0.0, -20.0);
        assert!(camera.pitch >= -1.4);
    }

    #[test]
    fn test_orbit_camera_zoom() {
        let mut camera = OrbitCameraConfig::default();
        let initial = camera.distance;
        camera.zoom(1.0);
        assert!((camera.distance - (initial - 1.0)).abs() < f32::EPSILON);

        camera.zoom(100.0);
        assert_eq!(camera.distance, camera.min_distance);

        camera.zoom(-200.0);
        assert_eq!(camera.distance, camera.max_distance);
    }

    #[test]
    fn test_current_clip_duration() {
        let mut state = AnimationViewerState::new();
        assert!(state.current_clip_duration().is_none());

        state.set_model("char.glb".to_string(), sample_clips());
        state.selected_clip = Some(1);
        assert_eq!(state.current_clip_duration(), Some(1.0));
    }
}
