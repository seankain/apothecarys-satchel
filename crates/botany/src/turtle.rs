use crate::lsystem::LSymbol;
use crate::mesh_gen::{MeshInstance, PlantMeshData, StemSegment};
use crate::phenotype::PlantPhenotype;
use serde::{Deserialize, Serialize};

/// 3D vector for turtle positioning and orientation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    pub fn up() -> Self {
        Self::new(0.0, 1.0, 0.0)
    }

    pub fn forward() -> Self {
        Self::new(0.0, 0.0, 1.0)
    }

    pub fn left() -> Self {
        Self::new(-1.0, 0.0, 0.0)
    }

    pub fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-8 {
            return self;
        }
        self.scale(1.0 / len)
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

/// Rotate a vector around an axis by angle (in degrees).
fn rotate_around_axis(v: Vec3, axis: Vec3, angle_deg: f32) -> Vec3 {
    let angle = angle_deg.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let k = axis.normalize();

    // Rodrigues' rotation formula: v*cos(a) + (k×v)*sin(a) + k*(k·v)*(1-cos(a))
    let dot = k.x * v.x + k.y * v.y + k.z * v.z;
    let cross = k.cross(v);

    Vec3::new(
        v.x * cos_a + cross.x * sin_a + k.x * dot * (1.0 - cos_a),
        v.y * cos_a + cross.y * sin_a + k.y * dot * (1.0 - cos_a),
        v.z * cos_a + cross.z * sin_a + k.z * dot * (1.0 - cos_a),
    )
}

/// The turtle's current state in 3D space.
#[derive(Debug, Clone)]
pub struct TurtleState {
    pub position: Vec3,
    pub heading: Vec3,
    pub left: Vec3,
    pub up: Vec3,
    pub width: f32,
}

impl Default for TurtleState {
    fn default() -> Self {
        Self {
            position: Vec3::zero(),
            heading: Vec3::up(),       // Plants grow upward
            left: Vec3::left(),
            up: Vec3::forward(),
            width: 0.05,
        }
    }
}

/// Interprets L-system symbol strings into 3D mesh placement data.
pub struct TurtleInterpreter {
    state: TurtleState,
    stack: Vec<TurtleState>,
}

impl TurtleInterpreter {
    pub fn new() -> Self {
        Self {
            state: TurtleState::default(),
            stack: Vec::new(),
        }
    }

    /// Interpret an L-system string into plant mesh data.
    pub fn interpret(
        &mut self,
        symbols: &[LSymbol],
        phenotype: &PlantPhenotype,
    ) -> PlantMeshData {
        let mut mesh_data = PlantMeshData::new();

        for symbol in symbols {
            match symbol {
                LSymbol::Forward(len) => {
                    let start = self.state.position;
                    self.state.position = start + self.state.heading.scale(*len);
                    mesh_data.stem_segments.push(StemSegment {
                        start,
                        end: self.state.position,
                        start_width: self.state.width,
                        end_width: self.state.width * 0.9,
                    });
                }
                LSymbol::TurnLeft(angle) => {
                    self.rotate_heading(*angle, self.state.up);
                }
                LSymbol::TurnRight(angle) => {
                    self.rotate_heading(-*angle, self.state.up);
                }
                LSymbol::PitchUp(angle) => {
                    self.rotate_heading(*angle, self.state.left);
                }
                LSymbol::PitchDown(angle) => {
                    self.rotate_heading(-*angle, self.state.left);
                }
                LSymbol::RollLeft(angle) => {
                    self.rotate_up(*angle);
                }
                LSymbol::RollRight(angle) => {
                    self.rotate_up(-*angle);
                }
                LSymbol::Push => {
                    self.stack.push(self.state.clone());
                }
                LSymbol::Pop => {
                    if let Some(saved) = self.stack.pop() {
                        self.state = saved;
                    }
                }
                LSymbol::Leaf => {
                    mesh_data.leaf_instances.push(MeshInstance {
                        position: self.state.position,
                        direction: self.state.heading,
                        up: self.state.up,
                        template_index: phenotype.leaf_mesh_index,
                        scale: phenotype.leaf_scale,
                        color: phenotype.leaf_color,
                    });
                }
                LSymbol::Flower if phenotype.produces_flowers => {
                    mesh_data.flower_instances.push(MeshInstance {
                        position: self.state.position,
                        direction: self.state.heading,
                        up: self.state.up,
                        template_index: 0,
                        scale: phenotype.petal_scale,
                        color: phenotype.petal_color,
                    });
                }
                LSymbol::Fruit if phenotype.produces_fruit => {
                    mesh_data.fruit_instances.push(MeshInstance {
                        position: self.state.position,
                        direction: self.state.heading,
                        up: self.state.up,
                        template_index: phenotype.fruit_mesh_index,
                        scale: phenotype.fruit_scale,
                        color: phenotype.fruit_color,
                    });
                }
                LSymbol::Width(w) => {
                    self.state.width = *w;
                }
                _ => {}
            }
        }

        mesh_data
    }

    fn rotate_heading(&mut self, angle: f32, axis: Vec3) {
        self.state.heading = rotate_around_axis(self.state.heading, axis, angle).normalize();
        self.state.left = rotate_around_axis(self.state.left, axis, angle).normalize();
        self.state.up = rotate_around_axis(self.state.up, axis, angle).normalize();
    }

    fn rotate_up(&mut self, angle: f32) {
        self.state.left = rotate_around_axis(self.state.left, self.state.heading, angle).normalize();
        self.state.up = rotate_around_axis(self.state.up, self.state.heading, angle).normalize();
    }
}

impl Default for TurtleInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phenotype::PlantColor;

    fn test_phenotype() -> PlantPhenotype {
        PlantPhenotype {
            axiom_complexity: 3,
            branch_angle: 30.0,
            branch_length: 1.0,
            branch_thickness: 0.05,
            branching_factor: 2,
            leaf_mesh_index: 0,
            leaf_scale: 0.5,
            leaf_color: PlantColor::from_hsv(120.0, 0.8, 0.7),
            leaves_per_segment: 2,
            produces_flowers: true,
            petal_count: 5,
            petal_color: PlantColor::from_hsv(300.0, 0.8, 0.9),
            petal_scale: 0.3,
            produces_fruit: true,
            fruit_mesh_index: 0,
            fruit_color: PlantColor::from_hsv(30.0, 0.7, 0.8),
            fruit_scale: 0.2,
        }
    }

    #[test]
    fn test_forward_moves_position() {
        let mut turtle = TurtleInterpreter::new();
        let phenotype = test_phenotype();
        let symbols = vec![LSymbol::Forward(1.0)];

        let mesh = turtle.interpret(&symbols, &phenotype);

        assert_eq!(mesh.stem_segments.len(), 1);
        let seg = &mesh.stem_segments[0];
        assert!((seg.start.x).abs() < f32::EPSILON);
        assert!((seg.start.y).abs() < f32::EPSILON);
        // Heading is up (Y+), so end should be at Y=1
        assert!((seg.end.y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_push_pop_restores_state() {
        let mut turtle = TurtleInterpreter::new();
        let phenotype = test_phenotype();
        let symbols = vec![
            LSymbol::Forward(1.0),
            LSymbol::Push,
            LSymbol::Forward(1.0),
            LSymbol::Pop,
            LSymbol::Forward(0.5),
        ];

        let mesh = turtle.interpret(&symbols, &phenotype);

        assert_eq!(mesh.stem_segments.len(), 3);
        // Third segment should start from where first ended (Y=1), not from Y=2
        let seg3 = &mesh.stem_segments[2];
        assert!((seg3.start.y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_leaf_placement() {
        let mut turtle = TurtleInterpreter::new();
        let phenotype = test_phenotype();
        let symbols = vec![LSymbol::Forward(1.0), LSymbol::Leaf];

        let mesh = turtle.interpret(&symbols, &phenotype);

        assert_eq!(mesh.leaf_instances.len(), 1);
        assert!((mesh.leaf_instances[0].position.y - 1.0).abs() < 0.01);
        assert_eq!(mesh.leaf_instances[0].template_index, phenotype.leaf_mesh_index);
    }

    #[test]
    fn test_flower_placement_when_enabled() {
        let mut turtle = TurtleInterpreter::new();
        let phenotype = test_phenotype();
        assert!(phenotype.produces_flowers);

        let symbols = vec![LSymbol::Forward(1.0), LSymbol::Flower];
        let mesh = turtle.interpret(&symbols, &phenotype);
        assert_eq!(mesh.flower_instances.len(), 1);
    }

    #[test]
    fn test_flower_skipped_when_disabled() {
        let mut turtle = TurtleInterpreter::new();
        let mut phenotype = test_phenotype();
        phenotype.produces_flowers = false;

        let symbols = vec![LSymbol::Forward(1.0), LSymbol::Flower];
        let mesh = turtle.interpret(&symbols, &phenotype);
        assert_eq!(mesh.flower_instances.len(), 0);
    }

    #[test]
    fn test_fruit_placement() {
        let mut turtle = TurtleInterpreter::new();
        let phenotype = test_phenotype();
        assert!(phenotype.produces_fruit);

        let symbols = vec![LSymbol::Forward(1.0), LSymbol::Fruit];
        let mesh = turtle.interpret(&symbols, &phenotype);
        assert_eq!(mesh.fruit_instances.len(), 1);
    }

    #[test]
    fn test_width_changes() {
        let mut turtle = TurtleInterpreter::new();
        let phenotype = test_phenotype();
        let symbols = vec![
            LSymbol::Width(0.1),
            LSymbol::Forward(1.0),
            LSymbol::Width(0.02),
            LSymbol::Forward(1.0),
        ];

        let mesh = turtle.interpret(&symbols, &phenotype);
        assert_eq!(mesh.stem_segments.len(), 2);
        assert!((mesh.stem_segments[0].start_width - 0.1).abs() < f32::EPSILON);
        assert!((mesh.stem_segments[1].start_width - 0.02).abs() < f32::EPSILON);
    }

    #[test]
    fn test_turn_changes_direction() {
        let mut turtle = TurtleInterpreter::new();
        let phenotype = test_phenotype();
        let symbols = vec![
            LSymbol::Forward(1.0),
            LSymbol::TurnLeft(90.0),
            LSymbol::Forward(1.0),
        ];

        let mesh = turtle.interpret(&symbols, &phenotype);
        assert_eq!(mesh.stem_segments.len(), 2);

        // After turning 90 degrees, the second segment should move in a different axis
        let seg2 = &mesh.stem_segments[1];
        let dx = (seg2.end.x - seg2.start.x).abs();
        let dz = (seg2.end.z - seg2.start.z).abs();
        let lateral = dx + dz;
        // Should have significant lateral movement after turning
        assert!(lateral > 0.5, "Expected lateral movement after turn, got {lateral}");
    }

    #[test]
    fn test_rotation_preserves_orthogonality() {
        let v = Vec3::up();
        let axis = Vec3::forward();
        let rotated = rotate_around_axis(v, axis, 45.0);

        // Length should be preserved
        assert!((rotated.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_empty_symbols_produce_empty_mesh() {
        let mut turtle = TurtleInterpreter::new();
        let phenotype = test_phenotype();
        let mesh = turtle.interpret(&[], &phenotype);

        assert!(mesh.stem_segments.is_empty());
        assert!(mesh.leaf_instances.is_empty());
        assert!(mesh.flower_instances.is_empty());
        assert!(mesh.fruit_instances.is_empty());
    }
}
