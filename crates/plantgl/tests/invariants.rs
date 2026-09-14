//! Property tests over random input.
//!
//! Original to this port; no upstream equivalent. Part of the `plantgl` crate
//! and therefore licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! These cover the invariants that a golden file cannot: that no input makes
//! a frame skew, that the OBJ round trip is lossless for geometry, and that a
//! bounding box really bounds.

use plantgl::algo::bbox::{bounding_box, BoundingBox};
use plantgl::codec::{from_obj, to_obj};
use plantgl::math::{frame::rotation_minimizing_frames, Frame, Point3, Real, Vec3};
use plantgl::modelling::{MeasureDrawer, Turtle};
use plantgl::{Geometry, Scene, Shape, TriangleSet};
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;

fn coordinate() -> impl Strategy<Value = f32> {
    -50.0f32..50.0f32
}

fn point() -> impl Strategy<Value = Point3> {
    (coordinate(), coordinate(), coordinate()).prop_map(|(x, y, z)| Point3::new(x, y, z))
}

/// One turtle command, as an L-system driver would issue it.
#[derive(Debug, Clone, Copy)]
enum Command {
    Forward(Real),
    Left(Real),
    Down(Real),
    RollLeft(Real),
    Width(Real),
    Scale(Real),
}

fn command() -> impl Strategy<Value = Command> {
    prop_oneof![
        (0.01f32..3.0).prop_map(Command::Forward),
        (-360.0f32..360.0).prop_map(Command::Left),
        (-360.0f32..360.0).prop_map(Command::Down),
        (-360.0f32..360.0).prop_map(Command::RollLeft),
        (0.001f32..1.0).prop_map(Command::Width),
        (0.1f32..4.0).prop_map(Command::Scale),
    ]
}

fn apply(turtle: &mut Turtle<MeasureDrawer>, command: Command) {
    match command {
        Command::Forward(length) => turtle.forward(length).unwrap(),
        Command::Left(angle) => turtle.left(angle),
        Command::Down(angle) => turtle.down(angle),
        Command::RollLeft(angle) => turtle.roll_left(angle),
        Command::Width(width) => turtle.set_width(width).unwrap(),
        Command::Scale(scale) => turtle.scale_uniform(scale),
    }
}

fn direction() -> impl Strategy<Value = Vec3> {
    (coordinate(), coordinate(), coordinate())
        .prop_map(|(x, y, z)| Vec3::new(x, y, z))
        .prop_filter("degenerate direction", |v| v.norm() > 1e-3)
}

proptest! {
    // Keep failing inputs next to this file rather than hunting for a crate
    // root that an integration test does not have.
    #![proptest_config(ProptestConfig {
        failure_persistence: Some(Box::new(FileFailurePersistence::WithSource(
            "proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// Any heading yields an orthonormal frame.
    #[test]
    fn from_heading_is_always_orthonormal(heading in direction()) {
        let frame = Frame::from_heading(Point3::origin(), heading).unwrap();
        prop_assert!(frame.is_orthonormal(1e-4), "{frame:?}");
    }

    /// A sequence of rotations, re-orthonormalised each step, never skews.
    #[test]
    fn rotations_keep_the_frame_orthonormal(
        steps in prop::collection::vec((direction(), -3.2f32..3.2f32), 1..64)
    ) {
        let mut frame = Frame::default();
        for (axis, angle) in steps {
            let axis = nalgebra::Unit::new_normalize(axis);
            let rotation = nalgebra::Rotation3::from_axis_angle(&axis, angle);
            frame = Frame::new(
                frame.position,
                rotation * frame.heading,
                rotation * frame.left,
                rotation * frame.up,
            );
            prop_assert!(frame.orthonormalize());
            prop_assert!(frame.is_orthonormal(1e-4), "{frame:?}");
        }
    }

    /// Every frame an RMF produces is orthonormal and sits on its sample.
    #[test]
    fn rmf_frames_are_orthonormal(points in prop::collection::vec(point(), 2..40)) {
        // Consecutive duplicates have no heading; drop them the way a caller
        // sampling a curve would.
        let mut distinct: Vec<Point3> = Vec::with_capacity(points.len());
        for p in points {
            if distinct.last().is_none_or(|last| (p - *last).norm() > 1e-3) {
                distinct.push(p);
            }
        }
        prop_assume!(distinct.len() >= 2);

        let Some(frames) = rotation_minimizing_frames(&distinct, &Frame::default()) else {
            return Ok(());
        };
        prop_assert_eq!(frames.len(), distinct.len());
        for (frame, sample) in frames.iter().zip(&distinct) {
            prop_assert!(frame.is_orthonormal(1e-3), "{frame:?}");
            prop_assert_eq!(frame.position, *sample);
        }
    }

    /// No command sequence skews the turtle's frame.
    ///
    /// This is the invariant the whole modelling layer rests on: every organ
    /// is placed by the frame, so a frame that is not orthonormal is a plant
    /// whose leaves are sheared.
    #[test]
    fn a_turtle_frame_stays_orthonormal(
        commands in prop::collection::vec(command(), 1..200)
    ) {
        let mut turtle = Turtle::new(MeasureDrawer::new());
        for command in commands {
            apply(&mut turtle, command);
            prop_assert!(
                turtle.is_valid(),
                "{command:?} skewed the frame: {:?}",
                turtle.state().frame
            );
        }
    }

    /// A balanced `push`/`pop` restores the state exactly, whatever happens
    /// in between.
    #[test]
    fn a_balanced_push_and_pop_restores_the_state(
        commands in prop::collection::vec(command(), 1..60)
    ) {
        let mut turtle = Turtle::new(MeasureDrawer::new());
        // Somewhere other than the initial state, so a restore that quietly
        // resets would be caught too.
        turtle.left(23.0);
        turtle.forward(0.7).unwrap();
        turtle.set_width(0.3).unwrap();

        let before = turtle.state().clone();
        turtle.push();
        for command in commands {
            apply(&mut turtle, command);
        }
        turtle.pop().unwrap();

        prop_assert_eq!(turtle.state().frame, before.frame);
        prop_assert_eq!(turtle.state().width, before.width);
        prop_assert_eq!(turtle.state().scale, before.scale);
        prop_assert_eq!(turtle.state().draw.color, before.draw.color);
        prop_assert_eq!(turtle.state().section_resolution, before.section_resolution);
    }

    /// Nesting pushes and pops in balanced pairs restores the state too, and
    /// the stack ends where it started.
    #[test]
    fn nested_pushes_unwind_to_the_start(depth in 1usize..12) {
        let mut turtle = Turtle::new(MeasureDrawer::new());
        let before = turtle.state().clone();
        for i in 0..depth {
            turtle.push();
            turtle.left(17.0 * i as Real);
            turtle.forward(0.1).unwrap();
        }
        prop_assert_eq!(turtle.stack_depth(), depth);
        for _ in 0..depth {
            turtle.pop().unwrap();
        }
        prop_assert!(turtle.stack_is_empty());
        prop_assert_eq!(turtle.state().frame, before.frame);
        prop_assert!(turtle.pop().is_err(), "the stack should now be empty");
    }

    /// A bounding box contains every point it was built from.
    #[test]
    fn bounding_box_contains_its_points(points in prop::collection::vec(point(), 1..64)) {
        let bbox = BoundingBox::from_points(&points).unwrap();
        for p in &points {
            prop_assert!(bbox.contains(*p), "{p:?} outside {bbox:?}");
        }
        prop_assert!(bbox.volume() >= 0.0);
    }

    /// Writing a mesh to OBJ and reading it back preserves the face count and
    /// the bounding box.
    #[test]
    fn obj_round_trip_preserves_faces_and_bounds(
        points in prop::collection::vec(point(), 3..30),
        face_seeds in prop::collection::vec((0usize..30, 0usize..30, 0usize..30), 1..30),
    ) {
        let count = points.len();
        let faces: Vec<[u32; 3]> = face_seeds
            .iter()
            .map(|(a, b, c)| [(a % count) as u32, (b % count) as u32, (c % count) as u32])
            .filter(|f| f[0] != f[1] && f[1] != f[2] && f[0] != f[2])
            .collect();
        prop_assume!(!faces.is_empty());

        let expected_faces = faces.len();
        let mesh = TriangleSet::new(points, faces);
        let scene = Scene::from_shapes(vec![
            Shape::new(Geometry::from(mesh).into_ref()).with_id(1),
        ]);

        let files = to_obj(&scene).unwrap();
        let restored = from_obj(&files.obj).unwrap();
        prop_assert_eq!(restored.len(), 1);

        let Geometry::TriangleSet(restored_mesh) = restored.shapes[0].geometry.as_ref() else {
            return Err(TestCaseError::fail("expected a TriangleSet"));
        };
        prop_assert_eq!(restored_mesh.indices.len(), expected_faces);

        let before = bounding_box(&scene.shapes[0].geometry).unwrap().unwrap();
        let after = bounding_box(&restored.shapes[0].geometry).unwrap().unwrap();
        // Only vertices a face references survive the round trip, so the box
        // can shrink, never grow — up to the rounding the writer's fixed
        // precision imposes.
        const SLACK: f32 = 1e-4;
        for axis in 0..3 {
            prop_assert!(after.lower_left[axis] >= before.lower_left[axis] - SLACK);
            prop_assert!(after.upper_right[axis] <= before.upper_right[axis] + SLACK);
        }
    }

    /// Writing twice gives the same bytes, whatever the geometry.
    #[test]
    fn export_is_deterministic(points in prop::collection::vec(point(), 3..20)) {
        let count = points.len() as u32;
        let faces: Vec<[u32; 3]> = (0..count - 2).map(|i| [i, i + 1, i + 2]).collect();
        let scene = Scene::from_shapes(vec![
            Shape::new(Geometry::from(TriangleSet::new(points, faces)).into_ref()),
        ]);
        prop_assert_eq!(to_obj(&scene).unwrap(), to_obj(&scene).unwrap());
    }
}
