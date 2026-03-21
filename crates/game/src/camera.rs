use fyrox::{
    core::{
        algebra::{Vector2, Vector3},
        math::Rect,
        pool::Handle,
    },
    graph::{BaseSceneGraph, SceneGraphNode},
    scene::{
        base::BaseBuilder,
        camera::{Camera, CameraBuilder, OrthographicProjection, Projection},
        graph::Graph,
        node::Node,
        transform::TransformBuilder,
        Scene,
    },
};
use std::f32::consts::FRAC_PI_4;

/// Configuration for the isometric camera.
#[derive(Debug)]
pub struct IsoCameraConfig {
    /// Rotation around Y-axis in radians (default: 45° = π/4).
    pub yaw: f32,
    /// Downward pitch in radians (default: 30° = π/6).
    pub pitch: f32,
    /// Distance from the target (affects position, not projection).
    pub distance: f32,
    /// Orthographic vertical size in world units (zoom level).
    pub vertical_size: f32,
    /// Minimum zoom (closest).
    pub min_vertical_size: f32,
    /// Maximum zoom (farthest).
    pub max_vertical_size: f32,
    /// Smoothing rate for camera follow (higher = faster).
    pub follow_speed: f32,
}

impl Default for IsoCameraConfig {
    fn default() -> Self {
        Self {
            yaw: FRAC_PI_4,                     // 45°
            pitch: std::f32::consts::FRAC_PI_6,  // 30°
            distance: 20.0,
            vertical_size: 10.0,
            min_vertical_size: 3.0,
            max_vertical_size: 25.0,
            follow_speed: 5.0,
        }
    }
}

/// Isometric camera controller managing orthographic projection,
/// smooth follow, zoom, and optional bounds clamping.
#[derive(Debug)]
pub struct IsometricCamera {
    pub camera_handle: Handle<Node>,
    pub config: IsoCameraConfig,
    /// Current target position (what the camera follows).
    pub target: Vector3<f32>,
    /// Current smoothed camera target (lerped toward `target`).
    pub current_target: Vector3<f32>,
    /// Optional world-space bounds to clamp the camera within.
    pub bounds: Option<CameraBounds>,
}

/// Axis-aligned bounds for the camera target in world XZ space.
#[derive(Debug, Clone, Copy)]
pub struct CameraBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
}

impl CameraBounds {
    pub fn new(min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> Self {
        Self { min_x, max_x, min_z, max_z }
    }

    /// Clamp a position to be within bounds.
    pub fn clamp(&self, pos: Vector3<f32>) -> Vector3<f32> {
        Vector3::new(
            pos.x.clamp(self.min_x, self.max_x),
            pos.y,
            pos.z.clamp(self.min_z, self.max_z),
        )
    }
}

impl IsometricCamera {
    /// Create the isometric camera and add it to the scene.
    pub fn new(scene: &mut Scene, config: IsoCameraConfig) -> Self {
        let camera_position = Self::calculate_position(Vector3::zeros(), &config);

        let camera_handle = CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(camera_position)
                    .build(),
            ),
        )
        .with_projection(Projection::Orthographic(OrthographicProjection {
            vertical_size: config.vertical_size,
            z_near: 0.1,
            z_far: 1000.0,
        }))
        .with_viewport(Rect::new(0.0, 0.0, 1.0, 1.0))
        .build(&mut scene.graph);

        // Compute look-at rotation and apply it
        Self::apply_look_at(&mut scene.graph, camera_handle, &camera_position, &Vector3::zeros());

        Self {
            camera_handle,
            config,
            target: Vector3::zeros(),
            current_target: Vector3::zeros(),
            bounds: None,
        }
    }

    /// Calculate camera world position from a target position and config.
    fn calculate_position(target: Vector3<f32>, config: &IsoCameraConfig) -> Vector3<f32> {
        let cos_pitch = config.pitch.cos();
        let sin_pitch = config.pitch.sin();
        let cos_yaw = config.yaw.cos();
        let sin_yaw = config.yaw.sin();

        let offset = Vector3::new(
            config.distance * cos_pitch * sin_yaw,
            config.distance * sin_pitch,
            config.distance * cos_pitch * cos_yaw,
        );

        target + offset
    }

    /// Apply a look-at rotation to the camera node's transform.
    fn apply_look_at(
        graph: &mut Graph,
        handle: Handle<Node>,
        eye: &Vector3<f32>,
        target: &Vector3<f32>,
    ) {
        use fyrox::core::algebra::{UnitQuaternion, Vector3 as V3};
        let direction = target - eye;
        if direction.norm_squared() < 1e-6 {
            return;
        }
        let direction = direction.normalize();

        // Build rotation looking from eye toward target (Fyrox uses -Z forward)
        let rotation = UnitQuaternion::face_towards(&direction, &V3::new(0.0, 1.0, 0.0));
        graph[handle].local_transform_mut().set_rotation(rotation);
    }

    /// Set the target for the camera to follow (typically player position).
    pub fn set_target(&mut self, position: Vector3<f32>) {
        self.target = position;
    }

    /// Set optional camera bounds.
    pub fn set_bounds(&mut self, bounds: Option<CameraBounds>) {
        self.bounds = bounds;
    }

    /// Adjust zoom by scroll delta. Positive = zoom in, negative = zoom out.
    pub fn zoom(&mut self, delta: f32) {
        self.config.vertical_size = (self.config.vertical_size - delta)
            .clamp(self.config.min_vertical_size, self.config.max_vertical_size);
    }

    /// Get the current vertical size (zoom level).
    pub fn vertical_size(&self) -> f32 {
        self.config.vertical_size
    }

    /// Get the camera's Y-axis rotation (yaw) for WASD direction calculation.
    pub fn yaw(&self) -> f32 {
        self.config.yaw
    }

    /// Update camera position and projection each frame.
    pub fn update(&mut self, scene: &mut Scene, dt: f32) {
        // Smooth follow: lerp current target toward actual target
        let alpha = 1.0 - (-self.config.follow_speed * dt).exp();
        self.current_target = lerp_vec3(self.current_target, self.target, alpha);

        // Apply bounds clamping
        if let Some(bounds) = &self.bounds {
            self.current_target = bounds.clamp(self.current_target);
        }

        // Calculate camera position
        let camera_pos = Self::calculate_position(self.current_target, &self.config);

        // Update position
        scene.graph[self.camera_handle]
            .local_transform_mut()
            .set_position(camera_pos);

        // Update rotation (look at target)
        Self::apply_look_at(
            &mut scene.graph,
            self.camera_handle,
            &camera_pos,
            &self.current_target,
        );

        // Update orthographic projection zoom
        if let Some(camera) = scene.graph[self.camera_handle].component_mut::<Camera>() {
            if let Projection::Orthographic(ref mut ortho) = camera.projection_mut() {
                ortho.vertical_size = self.config.vertical_size;
            }
        }
    }
}

fn lerp_vec3(a: Vector3<f32>, b: Vector3<f32>, t: f32) -> Vector3<f32> {
    a + (b - a) * t
}

/// Convert a screen-space point to a world-space ray for orthographic picking.
/// Returns (ray_origin, ray_direction) in world space.
pub fn ortho_screen_to_world_ray(
    screen_pos: Vector2<f32>,
    viewport_size: Vector2<f32>,
    graph: &Graph,
    camera_handle: Handle<Node>,
) -> Option<(Vector3<f32>, Vector3<f32>)> {
    let camera_node = graph.try_get(camera_handle)?;
    let camera = camera_node.component_ref::<Camera>()?;

    // Use the camera's view-projection matrix to unproject
    let inv_view_proj = camera.view_projection_matrix().try_inverse()?;

    // Convert screen coords to NDC (-1 to 1)
    let ndc_x = (screen_pos.x / viewport_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (screen_pos.y / viewport_size.y) * 2.0;

    // Unproject near and far points
    let near_ndc = fyrox::core::algebra::Vector4::new(ndc_x, ndc_y, -1.0, 1.0);
    let far_ndc = fyrox::core::algebra::Vector4::new(ndc_x, ndc_y, 1.0, 1.0);

    let near_world = inv_view_proj * near_ndc;
    let far_world = inv_view_proj * far_ndc;

    if near_world.w.abs() < 1e-6 || far_world.w.abs() < 1e-6 {
        return None;
    }

    let near_pos = Vector3::new(
        near_world.x / near_world.w,
        near_world.y / near_world.w,
        near_world.z / near_world.w,
    );
    let far_pos = Vector3::new(
        far_world.x / far_world.w,
        far_world.y / far_world.w,
        far_world.z / far_world.w,
    );

    let direction = (far_pos - near_pos).normalize();

    Some((near_pos, direction))
}

/// Intersect a ray with the ground plane (Y=0). Returns the hit point if any.
pub fn ray_ground_intersection(
    ray_origin: Vector3<f32>,
    ray_direction: Vector3<f32>,
) -> Option<Vector3<f32>> {
    // Plane: y = 0, normal = (0, 1, 0)
    if ray_direction.y.abs() < 1e-6 {
        return None; // Ray parallel to ground
    }

    let t = -ray_origin.y / ray_direction.y;
    if t < 0.0 {
        return None; // Intersection behind ray
    }

    Some(ray_origin + ray_direction * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_bounds_clamp() {
        let bounds = CameraBounds::new(0.0, 100.0, 0.0, 100.0);
        let clamped = bounds.clamp(Vector3::new(-5.0, 10.0, 150.0));
        assert_eq!(clamped.x, 0.0);
        assert_eq!(clamped.y, 10.0);
        assert_eq!(clamped.z, 100.0);

        let inside = bounds.clamp(Vector3::new(50.0, 5.0, 50.0));
        assert_eq!(inside, Vector3::new(50.0, 5.0, 50.0));
    }

    #[test]
    fn test_camera_config_defaults() {
        let config = IsoCameraConfig::default();
        assert!((config.yaw - FRAC_PI_4).abs() < 0.01);
        assert!((config.vertical_size - 10.0).abs() < 0.01);
        assert!(config.follow_speed > 0.0);
    }

    #[test]
    fn test_calculate_position() {
        let config = IsoCameraConfig::default();
        let pos = IsometricCamera::calculate_position(Vector3::zeros(), &config);
        // Should be offset from origin in +X, +Y, +Z direction
        assert!(pos.x > 0.0);
        assert!(pos.y > 0.0);
        assert!(pos.z > 0.0);
    }

    #[test]
    fn test_calculate_position_with_target() {
        let config = IsoCameraConfig::default();
        let origin_pos = IsometricCamera::calculate_position(Vector3::zeros(), &config);
        let offset_pos =
            IsometricCamera::calculate_position(Vector3::new(10.0, 0.0, 10.0), &config);
        // Offset should translate the camera by the same amount
        assert!((offset_pos.x - origin_pos.x - 10.0).abs() < 0.01);
        assert!((offset_pos.z - origin_pos.z - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_ray_ground_intersection() {
        // Ray pointing straight down from (5, 10, 5)
        let origin = Vector3::new(5.0, 10.0, 5.0);
        let dir = Vector3::new(0.0, -1.0, 0.0);
        let hit = ray_ground_intersection(origin, dir).unwrap();
        assert!((hit.x - 5.0).abs() < 0.001);
        assert!(hit.y.abs() < 0.001);
        assert!((hit.z - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_ray_ground_intersection_angled() {
        let origin = Vector3::new(0.0, 10.0, 0.0);
        let dir = Vector3::new(1.0, -1.0, 0.0).normalize();
        let hit = ray_ground_intersection(origin, dir).unwrap();
        assert!((hit.x - 10.0).abs() < 0.01);
        assert!(hit.y.abs() < 0.01);
    }

    #[test]
    fn test_ray_ground_parallel() {
        let origin = Vector3::new(0.0, 5.0, 0.0);
        let dir = Vector3::new(1.0, 0.0, 0.0);
        assert!(ray_ground_intersection(origin, dir).is_none());
    }

    #[test]
    fn test_ray_ground_behind() {
        let origin = Vector3::new(0.0, 5.0, 0.0);
        let dir = Vector3::new(0.0, 1.0, 0.0);
        assert!(ray_ground_intersection(origin, dir).is_none());
    }

    #[test]
    fn test_lerp_vec3() {
        let a = Vector3::new(0.0, 0.0, 0.0);
        let b = Vector3::new(10.0, 20.0, 30.0);
        let mid = lerp_vec3(a, b, 0.5);
        assert!((mid.x - 5.0).abs() < 0.01);
        assert!((mid.y - 10.0).abs() < 0.01);
        assert!((mid.z - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_zoom_clamping() {
        let mut config = IsoCameraConfig::default();
        // Simulate zoom on the config directly
        config.vertical_size = (config.vertical_size - 100.0)
            .clamp(config.min_vertical_size, config.max_vertical_size);
        assert_eq!(config.vertical_size, config.min_vertical_size);

        config.vertical_size = (config.vertical_size + 100.0)
            .clamp(config.min_vertical_size, config.max_vertical_size);
        assert_eq!(config.vertical_size, config.max_vertical_size);
    }
}
