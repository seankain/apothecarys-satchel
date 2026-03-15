use crate::location::{LocationId, SpawnPoint};
use crate::map_graph::WorldGraph;

/// The current phase of a scene transition.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionPhase {
    /// No transition in progress.
    None,
    /// Fading out the current scene.
    FadingOut { elapsed: f32, duration: f32 },
    /// Loading the target scene assets.
    Loading,
    /// Fading in the new scene.
    FadingIn { elapsed: f32, duration: f32 },
}

/// A pending transition request.
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionRequest {
    pub target_location: LocationId,
    pub arrival_spawn: String,
}

/// Manages scene transitions between world locations.
pub struct SceneTransitionManager {
    phase: TransitionPhase,
    pending: Option<TransitionRequest>,
    current_location: Option<LocationId>,
    fade_duration: f32,
}

impl SceneTransitionManager {
    pub fn new() -> Self {
        Self {
            phase: TransitionPhase::None,
            pending: None,
            current_location: None,
            fade_duration: 0.5,
        }
    }

    /// Get the current location id.
    pub fn current_location(&self) -> Option<&LocationId> {
        self.current_location.as_ref()
    }

    /// Get the current transition phase.
    pub fn phase(&self) -> &TransitionPhase {
        &self.phase
    }

    /// Whether a transition is currently in progress.
    pub fn is_transitioning(&self) -> bool {
        !matches!(self.phase, TransitionPhase::None)
    }

    /// Get the current fade alpha (0.0 = transparent, 1.0 = fully black).
    pub fn fade_alpha(&self) -> f32 {
        match &self.phase {
            TransitionPhase::None => 0.0,
            TransitionPhase::FadingOut { elapsed, duration } => {
                (elapsed / duration).min(1.0)
            }
            TransitionPhase::Loading => 1.0,
            TransitionPhase::FadingIn { elapsed, duration } => {
                1.0 - (elapsed / duration).min(1.0)
            }
        }
    }

    /// Request a transition to a new location. Returns an error if the exit
    /// doesn't exist or the target spawn is missing.
    pub fn request_transition(
        &mut self,
        target: LocationId,
        arrival_spawn: String,
        world: &WorldGraph,
    ) -> Result<(), TransitionError> {
        if self.is_transitioning() {
            return Err(TransitionError::AlreadyTransitioning);
        }

        // Validate target exists
        world
            .get_location(&target)
            .ok_or(TransitionError::UnknownLocation(target.0.clone()))?;

        // Validate spawn exists at target
        let target_loc = world.get_location(&target).unwrap();
        let has_spawn = target_loc
            .spawn_points
            .iter()
            .any(|sp| sp.name == arrival_spawn);
        if !has_spawn {
            return Err(TransitionError::UnknownSpawn(arrival_spawn));
        }

        self.pending = Some(TransitionRequest {
            target_location: target,
            arrival_spawn,
        });
        self.phase = TransitionPhase::FadingOut {
            elapsed: 0.0,
            duration: self.fade_duration,
        };

        Ok(())
    }

    /// Update the transition state. Returns transition events that the game
    /// should respond to.
    pub fn update(&mut self, dt: f32) -> TransitionEvent {
        match &mut self.phase {
            TransitionPhase::None => TransitionEvent::None,
            TransitionPhase::FadingOut { elapsed, duration } => {
                *elapsed += dt;
                if *elapsed >= *duration {
                    self.phase = TransitionPhase::Loading;
                    TransitionEvent::ReadyToLoad(self.pending.clone().unwrap())
                } else {
                    TransitionEvent::None
                }
            }
            TransitionPhase::Loading => {
                // Waiting for the game to call `notify_scene_loaded()`
                TransitionEvent::None
            }
            TransitionPhase::FadingIn { elapsed, duration } => {
                *elapsed += dt;
                if *elapsed >= *duration {
                    self.phase = TransitionPhase::None;
                    TransitionEvent::TransitionComplete
                } else {
                    TransitionEvent::None
                }
            }
        }
    }

    /// Called by the game after the new scene has been loaded.
    /// Resolves the spawn point and begins fade-in.
    pub fn notify_scene_loaded<'a>(
        &mut self,
        world: &'a WorldGraph,
    ) -> Option<&'a SpawnPoint> {
        if !matches!(self.phase, TransitionPhase::Loading) {
            return None;
        }

        let request = self.pending.take()?;
        let target_loc = world.get_location(&request.target_location)?;
        let spawn = target_loc
            .spawn_points
            .iter()
            .find(|sp| sp.name == request.arrival_spawn);

        self.current_location = Some(request.target_location);
        self.phase = TransitionPhase::FadingIn {
            elapsed: 0.0,
            duration: self.fade_duration,
        };

        spawn
    }

    /// Set the current location directly (e.g., on initial game load).
    pub fn set_current_location(&mut self, id: LocationId) {
        self.current_location = Some(id);
    }
}

impl Default for SceneTransitionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Events emitted by the transition manager during update.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionEvent {
    None,
    /// The fade-out is complete and the game should load the new scene.
    ReadyToLoad(TransitionRequest),
    /// The transition is fully complete.
    TransitionComplete,
}

/// Errors that can occur during scene transitions.
#[derive(Debug, thiserror::Error)]
pub enum TransitionError {
    #[error("A transition is already in progress")]
    AlreadyTransitioning,
    #[error("Unknown location: {0}")]
    UnknownLocation(String),
    #[error("Unknown spawn point: {0}")]
    UnknownSpawn(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::*;

    fn test_world() -> WorldGraph {
        WorldGraph::from_locations(vec![
            LocationDef {
                id: LocationId::from("hub"),
                display_name: "Hub".to_string(),
                scene_path: "hub.gltf".to_string(),
                location_type: LocationType::Hub,
                spawn_points: vec![SpawnPoint {
                    name: "from_dungeon".to_string(),
                    position: [5.0, 0.0, 3.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::PlayerArrival,
                }],
                exits: vec![ExitDef {
                    target_location: LocationId::from("dungeon"),
                    exit_node_name: "exit_dungeon".to_string(),
                    arrival_spawn: "entrance".to_string(),
                }],
            },
            LocationDef {
                id: LocationId::from("dungeon"),
                display_name: "Dungeon".to_string(),
                scene_path: "dungeon.gltf".to_string(),
                location_type: LocationType::Dungeon {
                    floor: 1,
                    difficulty: 1,
                },
                spawn_points: vec![SpawnPoint {
                    name: "entrance".to_string(),
                    position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    spawn_type: SpawnType::PlayerArrival,
                }],
                exits: vec![ExitDef {
                    target_location: LocationId::from("hub"),
                    exit_node_name: "exit_hub".to_string(),
                    arrival_spawn: "from_dungeon".to_string(),
                }],
            },
        ])
        .unwrap()
    }

    #[test]
    fn test_initial_state() {
        let mgr = SceneTransitionManager::new();
        assert!(!mgr.is_transitioning());
        assert!(mgr.current_location().is_none());
        assert_eq!(mgr.fade_alpha(), 0.0);
    }

    #[test]
    fn test_request_transition() {
        let world = test_world();
        let mut mgr = SceneTransitionManager::new();
        mgr.set_current_location(LocationId::from("hub"));

        mgr.request_transition(
            LocationId::from("dungeon"),
            "entrance".to_string(),
            &world,
        )
        .unwrap();

        assert!(mgr.is_transitioning());
        assert!(matches!(mgr.phase(), TransitionPhase::FadingOut { .. }));
    }

    #[test]
    fn test_full_transition_flow() {
        let world = test_world();
        let mut mgr = SceneTransitionManager::new();
        mgr.set_current_location(LocationId::from("hub"));

        mgr.request_transition(
            LocationId::from("dungeon"),
            "entrance".to_string(),
            &world,
        )
        .unwrap();

        // Fade out over multiple frames
        assert_eq!(mgr.update(0.3), TransitionEvent::None);
        assert!(mgr.fade_alpha() > 0.0);

        // Complete fade out
        let event = mgr.update(0.3);
        assert!(matches!(event, TransitionEvent::ReadyToLoad(_)));
        assert_eq!(mgr.fade_alpha(), 1.0);

        // Notify scene loaded
        let spawn = mgr.notify_scene_loaded(&world).unwrap();
        assert_eq!(spawn.name, "entrance");
        assert_eq!(spawn.position, [0.0, 0.0, 0.0]);
        assert_eq!(
            mgr.current_location(),
            Some(&LocationId::from("dungeon"))
        );

        // Fade in
        assert!(matches!(mgr.phase(), TransitionPhase::FadingIn { .. }));
        assert_eq!(mgr.update(0.3), TransitionEvent::None);

        // Complete fade in
        let event = mgr.update(0.3);
        assert_eq!(event, TransitionEvent::TransitionComplete);
        assert!(!mgr.is_transitioning());
        assert_eq!(mgr.fade_alpha(), 0.0);
    }

    #[test]
    fn test_cannot_double_transition() {
        let world = test_world();
        let mut mgr = SceneTransitionManager::new();
        mgr.set_current_location(LocationId::from("hub"));

        mgr.request_transition(
            LocationId::from("dungeon"),
            "entrance".to_string(),
            &world,
        )
        .unwrap();

        let result = mgr.request_transition(
            LocationId::from("dungeon"),
            "entrance".to_string(),
            &world,
        );
        assert!(matches!(result, Err(TransitionError::AlreadyTransitioning)));
    }

    #[test]
    fn test_transition_to_unknown_location() {
        let world = test_world();
        let mut mgr = SceneTransitionManager::new();

        let result = mgr.request_transition(
            LocationId::from("nowhere"),
            "spawn".to_string(),
            &world,
        );
        assert!(matches!(result, Err(TransitionError::UnknownLocation(_))));
    }

    #[test]
    fn test_transition_to_unknown_spawn() {
        let world = test_world();
        let mut mgr = SceneTransitionManager::new();

        let result = mgr.request_transition(
            LocationId::from("dungeon"),
            "nonexistent_spawn".to_string(),
            &world,
        );
        assert!(matches!(result, Err(TransitionError::UnknownSpawn(_))));
    }

    #[test]
    fn test_round_trip_transition() {
        let world = test_world();
        let mut mgr = SceneTransitionManager::new();
        mgr.set_current_location(LocationId::from("hub"));

        // Hub -> Dungeon
        mgr.request_transition(
            LocationId::from("dungeon"),
            "entrance".to_string(),
            &world,
        )
        .unwrap();
        mgr.update(1.0); // complete fade out
        mgr.notify_scene_loaded(&world);
        mgr.update(1.0); // complete fade in

        assert_eq!(
            mgr.current_location(),
            Some(&LocationId::from("dungeon"))
        );

        // Dungeon -> Hub
        mgr.request_transition(
            LocationId::from("hub"),
            "from_dungeon".to_string(),
            &world,
        )
        .unwrap();
        mgr.update(1.0);
        let spawn = mgr.notify_scene_loaded(&world).unwrap();
        assert_eq!(spawn.position, [5.0, 0.0, 3.0]);
        mgr.update(1.0);

        assert_eq!(
            mgr.current_location(),
            Some(&LocationId::from("hub"))
        );
        assert!(!mgr.is_transitioning());
    }
}
