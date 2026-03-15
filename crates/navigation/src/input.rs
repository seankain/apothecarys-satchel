use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::navmesh::NavMesh;

/// Commands that drive player movement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MovementCommand {
    /// Navigate to a target position via pathfinding.
    NavigateTo(Vec3),
    /// Move in a direction directly (WASD-style, camera-relative).
    DirectMove(Vec3),
    /// Stop all movement immediately.
    Stop,
}

/// The current movement mode, determining which input system is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MovementMode {
    /// No active movement.
    Idle,
    /// Following a pathfound route (click-to-move).
    FollowingPath,
    /// Moving directly via keyboard input (WASD).
    DirectControl,
}

/// State for managing player movement: path following and direct movement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMovement {
    /// Current position of the player.
    pub position: Vec3,
    /// Current movement mode.
    pub mode: MovementMode,
    /// Pathfound waypoints to follow (click-to-move).
    pub path: Vec<Vec3>,
    /// Index of the current waypoint being approached.
    pub path_index: usize,
    /// Current velocity direction for direct movement.
    pub velocity: Vec3,
    /// Movement speed in world units per second.
    pub speed: f32,
    /// Distance threshold to consider a waypoint reached.
    pub waypoint_threshold: f32,
}

impl Default for PlayerMovement {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            mode: MovementMode::Idle,
            path: Vec::new(),
            path_index: 0,
            velocity: Vec3::ZERO,
            speed: 4.0,
            waypoint_threshold: 0.2,
        }
    }
}

impl PlayerMovement {
    pub fn new(position: Vec3, speed: f32) -> Self {
        Self {
            position,
            speed,
            ..Default::default()
        }
    }

    /// Process a movement command. Click overrides WASD and vice versa.
    pub fn handle_command(&mut self, command: MovementCommand, navmesh: &NavMesh) {
        match command {
            MovementCommand::NavigateTo(target) => {
                if let Some(path) = navmesh.find_path(self.position, target) {
                    self.path = path;
                    self.path_index = 1; // skip start (current pos)
                    self.mode = MovementMode::FollowingPath;
                    self.velocity = Vec3::ZERO;
                }
            }
            MovementCommand::DirectMove(direction) => {
                self.mode = MovementMode::DirectControl;
                self.velocity = direction.normalize_or_zero();
                self.path.clear();
                self.path_index = 0;
            }
            MovementCommand::Stop => {
                self.mode = MovementMode::Idle;
                self.velocity = Vec3::ZERO;
                self.path.clear();
                self.path_index = 0;
            }
        }
    }

    /// Update movement state for one frame. Returns the new position.
    pub fn update(&mut self, dt: f32, navmesh: &NavMesh) -> Vec3 {
        match self.mode {
            MovementMode::Idle => {}
            MovementMode::FollowingPath => {
                self.update_path_following(dt, navmesh);
            }
            MovementMode::DirectControl => {
                self.update_direct_move(dt, navmesh);
            }
        }
        self.position
    }

    /// Follow the pathfound route, moving toward each waypoint in sequence.
    fn update_path_following(&mut self, dt: f32, navmesh: &NavMesh) {
        if self.path_index >= self.path.len() {
            self.mode = MovementMode::Idle;
            self.path.clear();
            return;
        }

        let target = self.path[self.path_index];
        let to_target = target - self.position;
        let distance = to_target.length();

        if distance < self.waypoint_threshold {
            self.path_index += 1;
            if self.path_index >= self.path.len() {
                self.position = target;
                self.mode = MovementMode::Idle;
                self.path.clear();
            }
            return;
        }

        let direction = to_target / distance;
        let move_dist = self.speed * dt;
        let new_pos = if move_dist >= distance {
            target
        } else {
            self.position + direction * move_dist
        };

        // Clamp to navmesh: if the new position is off the mesh, stay put
        if navmesh.is_walkable(new_pos) {
            self.position = new_pos;
        }
    }

    /// Move directly in the given direction, clamping to navmesh.
    fn update_direct_move(&mut self, dt: f32, navmesh: &NavMesh) {
        if self.velocity.length_squared() < 0.001 {
            return;
        }

        let new_pos = self.position + self.velocity * self.speed * dt;

        if navmesh.is_walkable(new_pos) {
            self.position = new_pos;
        } else {
            // Try sliding along X and Z axes independently
            let slide_x = Vec3::new(new_pos.x, self.position.y, self.position.z);
            let slide_z = Vec3::new(self.position.x, self.position.y, new_pos.z);

            if navmesh.is_walkable(slide_x) {
                self.position = slide_x;
            } else if navmesh.is_walkable(slide_z) {
                self.position = slide_z;
            }
            // If neither works, player stays in place (hit a corner)
        }
    }

    /// Returns true if the player is currently moving.
    pub fn is_moving(&self) -> bool {
        self.mode != MovementMode::Idle
    }

    /// Get the direction the player is currently facing/moving.
    pub fn facing_direction(&self) -> Vec3 {
        match self.mode {
            MovementMode::DirectControl => self.velocity.normalize_or_zero(),
            MovementMode::FollowingPath => {
                if self.path_index < self.path.len() {
                    (self.path[self.path_index] - self.position).normalize_or_zero()
                } else {
                    Vec3::ZERO
                }
            }
            MovementMode::Idle => Vec3::ZERO,
        }
    }
}

/// Convert WASD input into a camera-relative direction vector.
/// `camera_yaw` is the rotation around Y-axis in radians (45° for isometric).
pub fn wasd_to_world_direction(forward: bool, back: bool, left: bool, right: bool, camera_yaw: f32) -> Vec3 {
    let mut input = Vec3::ZERO;
    if forward { input.z -= 1.0; }
    if back { input.z += 1.0; }
    if left { input.x -= 1.0; }
    if right { input.x += 1.0; }

    if input.length_squared() < 0.001 {
        return Vec3::ZERO;
    }
    input = input.normalize();

    // Rotate input by camera yaw
    let cos_y = camera_yaw.cos();
    let sin_y = camera_yaw.sin();
    Vec3::new(
        input.x * cos_y + input.z * sin_y,
        0.0,
        -input.x * sin_y + input.z * cos_y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navmesh::NavPolygon;
    use std::f32::consts::FRAC_PI_4;

    fn make_simple_mesh() -> NavMesh {
        // Single large quad: (0,0,0) to (20,0,20)
        NavMesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 20.0),
                Vec3::new(0.0, 0.0, 20.0),
            ],
            vec![NavPolygon {
                vertices: vec![0, 1, 2, 3],
                neighbors: vec![None, None, None, None],
            }],
        )
    }

    fn make_two_room_mesh() -> NavMesh {
        NavMesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(0.0, 0.0, 10.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 10.0),
            ],
            vec![
                NavPolygon {
                    vertices: vec![0, 1, 2, 3],
                    neighbors: vec![None, Some(1), None, None],
                },
                NavPolygon {
                    vertices: vec![1, 4, 5, 2],
                    neighbors: vec![None, None, None, Some(0)],
                },
            ],
        )
    }

    #[test]
    fn test_default_movement() {
        let pm = PlayerMovement::default();
        assert_eq!(pm.mode, MovementMode::Idle);
        assert!(!pm.is_moving());
    }

    #[test]
    fn test_navigate_to() {
        let mesh = make_simple_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(5.0, 0.0, 5.0), 4.0);

        pm.handle_command(MovementCommand::NavigateTo(Vec3::new(15.0, 0.0, 15.0)), &mesh);
        assert_eq!(pm.mode, MovementMode::FollowingPath);
        assert!(pm.is_moving());
        assert!(!pm.path.is_empty());
    }

    #[test]
    fn test_path_following_arrives() {
        let mesh = make_simple_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(5.0, 0.0, 5.0), 100.0); // fast speed

        pm.handle_command(
            MovementCommand::NavigateTo(Vec3::new(6.0, 0.0, 5.0)),
            &mesh,
        );

        // After enough updates, should arrive
        for _ in 0..100 {
            pm.update(0.1, &mesh);
        }
        assert_eq!(pm.mode, MovementMode::Idle);
        assert!((pm.position.x - 6.0).abs() < 0.3);
    }

    #[test]
    fn test_direct_move() {
        let mesh = make_simple_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(10.0, 0.0, 10.0), 4.0);

        pm.handle_command(
            MovementCommand::DirectMove(Vec3::new(1.0, 0.0, 0.0)),
            &mesh,
        );
        assert_eq!(pm.mode, MovementMode::DirectControl);

        let old_x = pm.position.x;
        pm.update(1.0, &mesh);
        assert!(pm.position.x > old_x);
    }

    #[test]
    fn test_direct_move_clamped_to_navmesh() {
        let mesh = make_simple_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(19.5, 0.0, 10.0), 4.0);

        // Move toward boundary
        pm.handle_command(
            MovementCommand::DirectMove(Vec3::new(1.0, 0.0, 0.0)),
            &mesh,
        );
        pm.update(1.0, &mesh);
        // Should not go past the mesh edge
        assert!(pm.position.x <= 20.0);
    }

    #[test]
    fn test_stop_command() {
        let mesh = make_simple_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(5.0, 0.0, 5.0), 4.0);

        pm.handle_command(
            MovementCommand::DirectMove(Vec3::new(1.0, 0.0, 0.0)),
            &mesh,
        );
        assert!(pm.is_moving());

        pm.handle_command(MovementCommand::Stop, &mesh);
        assert!(!pm.is_moving());
        assert_eq!(pm.mode, MovementMode::Idle);
    }

    #[test]
    fn test_click_overrides_wasd() {
        let mesh = make_simple_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(5.0, 0.0, 5.0), 4.0);

        pm.handle_command(
            MovementCommand::DirectMove(Vec3::new(1.0, 0.0, 0.0)),
            &mesh,
        );
        assert_eq!(pm.mode, MovementMode::DirectControl);

        pm.handle_command(
            MovementCommand::NavigateTo(Vec3::new(10.0, 0.0, 10.0)),
            &mesh,
        );
        assert_eq!(pm.mode, MovementMode::FollowingPath);
    }

    #[test]
    fn test_wasd_overrides_click() {
        let mesh = make_simple_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(5.0, 0.0, 5.0), 4.0);

        pm.handle_command(
            MovementCommand::NavigateTo(Vec3::new(10.0, 0.0, 10.0)),
            &mesh,
        );
        assert_eq!(pm.mode, MovementMode::FollowingPath);

        pm.handle_command(
            MovementCommand::DirectMove(Vec3::new(0.0, 0.0, 1.0)),
            &mesh,
        );
        assert_eq!(pm.mode, MovementMode::DirectControl);
        assert!(pm.path.is_empty());
    }

    #[test]
    fn test_navigate_across_polygons() {
        let mesh = make_two_room_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(5.0, 0.0, 5.0), 100.0);

        pm.handle_command(
            MovementCommand::NavigateTo(Vec3::new(15.0, 0.0, 5.0)),
            &mesh,
        );
        assert!(pm.is_moving());

        for _ in 0..200 {
            pm.update(0.05, &mesh);
        }
        assert!((pm.position.x - 15.0).abs() < 0.5);
    }

    #[test]
    fn test_wasd_to_world_direction_no_input() {
        let dir = wasd_to_world_direction(false, false, false, false, 0.0);
        assert!(dir.length() < 0.001);
    }

    #[test]
    fn test_wasd_to_world_direction_forward() {
        let dir = wasd_to_world_direction(true, false, false, false, 0.0);
        assert!(dir.z < 0.0); // Forward is -Z
        assert!(dir.length() > 0.9);
    }

    #[test]
    fn test_wasd_to_world_direction_rotated() {
        // 45° camera yaw (isometric)
        let dir = wasd_to_world_direction(true, false, false, false, FRAC_PI_4);
        // Should be rotated 45° from pure -Z
        assert!(dir.length() > 0.9);
        // The direction should have both X and Z components
        assert!(dir.x.abs() > 0.1);
    }

    #[test]
    fn test_wasd_diagonal_normalized() {
        let dir = wasd_to_world_direction(true, false, true, false, 0.0);
        assert!((dir.length() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_facing_direction() {
        let mesh = make_simple_mesh();
        let mut pm = PlayerMovement::new(Vec3::new(5.0, 0.0, 5.0), 4.0);

        // Idle: no facing direction
        assert!(pm.facing_direction().length() < 0.001);

        // Direct move: faces velocity direction
        pm.handle_command(
            MovementCommand::DirectMove(Vec3::new(1.0, 0.0, 0.0)),
            &mesh,
        );
        let dir = pm.facing_direction();
        assert!((dir.x - 1.0).abs() < 0.01);
    }
}
