//! Acceptance tests for the turtle (T8.8, T8.9).
//!
//! Part of the `plantgl` crate and therefore licensed CeCILL-C; see
//! crates/plantgl/LICENSE. Nothing here is translated from PlantGL — these are
//! the issue's acceptance criteria, written against the port's API. The one
//! exception is noted where it applies: `the_botany_turtle_suite` is a
//! mechanical translation of `crates/botany/src/turtle.rs`'s own tests, which
//! are this repository's, not upstream's.

use plantgl::algo::discretize::{discretize, DiscretizeCtx};
use plantgl::algo::measure::surface_area;
use plantgl::algo::tessellate::tessellate;
use plantgl::math::{Point3, Real, Vec3};
use plantgl::modelling::{MeasureDrawer, MeshDrawer, SceneDrawer, Turtle, TurtleDrawer};
use plantgl::scenegraph::curve::{Curve3D, ParametricCurve};
use plantgl::scenegraph::geometry::Geometry;
use plantgl::scenegraph::mesh::Polyline;

/// The section resolution every test that counts triangles uses.
const SLICES: u32 = 8;

/// The tight box of a triangle set — every vertex, rather than a transformed
/// box of a box.
fn mesh_bbox(mesh: &plantgl::scenegraph::mesh::TriangleSet) -> (Point3, Point3) {
    let bbox = plantgl::algo::bbox::BoundingBox::from_points(&mesh.model.points).unwrap();
    (bbox.lower_left, bbox.upper_right)
}

// ---------------------------------------------------------------- T8.8 ----

/// Acceptance: `F(1)` from the default state draws one segment of length 1
/// along `+Y`.
///
/// "The default state" here is [`Turtle::upright`] — the game's frame. See
/// `TurtleState::upright`: upstream's own default heads `+Z`, and
/// `upstreams_default_state_heads_along_z` below pins that.
#[test]
fn one_forward_draws_one_unit_segment_along_y() {
    let mut turtle = Turtle::upright(SceneDrawer::new());
    turtle.set_width(0.1).unwrap();
    turtle.forward(1.0).unwrap();

    let scene = turtle.into_drawer().into_scene();
    assert_eq!(scene.len(), 1, "one F should draw exactly one shape");

    let bbox = scene.bbox().unwrap().unwrap();
    assert!((bbox.lower_left.y - 0.0).abs() < 1e-5, "{bbox:?}");
    assert!((bbox.upper_right.y - 1.0).abs() < 1e-5, "{bbox:?}");
    // The segment is a tube about the Y axis: its other extents are the width.
    assert!(bbox.upper_right.x <= 0.1 + 1e-5 && bbox.upper_right.z <= 0.1 + 1e-5);
}

#[test]
fn upstreams_default_state_heads_along_z() {
    let turtle = Turtle::new(MeasureDrawer::new());
    assert_eq!(turtle.heading(), Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(turtle.left_vector(), Vec3::new(0.0, -1.0, 0.0));
    assert_eq!(turtle.up_vector(), Vec3::new(1.0, 0.0, 0.0));
}

/// Acceptance: `push; F(1); pop; F(1)` draws two segments that share a start
/// point.
#[test]
fn a_balanced_push_and_pop_branches_from_one_point() {
    let mut turtle = Turtle::upright(SceneDrawer::new());
    turtle.set_width(0.05).unwrap();

    let branch_start = turtle.position();
    turtle.push();
    turtle.left(45.0);
    turtle.forward(1.0).unwrap();
    turtle.pop().unwrap();

    let trunk_start = turtle.position();
    let trunk_heading = turtle.heading();
    turtle.forward(1.0).unwrap();

    assert_eq!(trunk_start, branch_start, "pop did not restore the position");
    assert_eq!(
        trunk_heading,
        Vec3::y(),
        "pop did not restore the orientation"
    );
    assert_eq!(turtle.into_drawer().into_scene().len(), 2);
}

/// Acceptance: `left(90)` then `F(1)` moves purely laterally.
#[test]
fn a_quarter_turn_makes_the_next_move_lateral() {
    let mut turtle = Turtle::upright(MeasureDrawer::new());
    turtle.left(90.0);
    turtle.forward(1.0).unwrap();

    let position = turtle.position();
    assert!(position.y.abs() < 1e-5, "{position:?}");
    assert!(
        (position.coords.norm() - 1.0).abs() < 1e-5,
        "the move should still be one unit long: {position:?}"
    );
}

/// Acceptance: 10 000 random rotation commands leave the frame orthonormal to
/// 1e-4.
#[test]
fn ten_thousand_rotations_leave_the_frame_orthonormal() {
    // A fixed xorshift keeps this deterministic without a dependency.
    let mut state: u32 = 0x1234_5678;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state as Real / u32::MAX as Real) * 720.0 - 360.0
    };

    let mut turtle = Turtle::new(MeasureDrawer::new());
    for i in 0..10_000 {
        let angle = next();
        match i % 5 {
            0 => turtle.left(angle),
            1 => turtle.down(angle),
            2 => turtle.roll_left(angle),
            3 => turtle.up(angle),
            _ => turtle.roll_right(angle),
        }
        assert!(
            turtle.state().frame.is_orthonormal(1e-4),
            "the frame drifted after {i} rotations: {:?}",
            turtle.state().frame
        );
    }
}

/// Acceptance: all three drawers report consistent metrics for the same
/// command sequence.
///
/// "Consistent" is exact for the tubes and the flat shapes — the measure
/// drawer computes what the mesh *is*, not what the ideal surface would be
/// (see its module docs) — so the tolerance here is floating-point, not
/// convergence.
#[test]
fn the_three_drawers_agree_on_the_same_program() {
    fn run<D: TurtleDrawer>(drawer: D) -> Turtle<D> {
        let mut turtle = Turtle::upright(drawer);
        turtle.set_section_resolution(SLICES);
        turtle.set_width(0.05).unwrap();
        turtle.forward(1.0).unwrap();
        turtle.push();
        turtle.left(35.0);
        turtle.forward_tapered(0.7, 0.02).unwrap();
        turtle.quad(0.3, Some(0.1)).unwrap();
        turtle.pop().unwrap();
        turtle.right(35.0);
        turtle.forward_tapered(0.7, 0.02).unwrap();
        turtle.start_polygon();
        turtle.polygon_point();
        turtle.left(60.0);
        turtle.forward(0.2).unwrap();
        turtle.left(60.0);
        turtle.forward(0.2).unwrap();
        turtle.stop_polygon(false).unwrap();
        turtle
    }

    let scene_turtle = run(SceneDrawer::new());
    let mesh_turtle = run(MeshDrawer::with_ctx(DiscretizeCtx {
        slices: SLICES as u8,
        ..DiscretizeCtx::default()
    }));
    let measure_turtle = run(MeasureDrawer::new());

    let scene_drawer = scene_turtle.into_drawer();
    let scene = scene_drawer.scene();
    let mesh = mesh_turtle
        .into_drawer()
        .merged()
        .unwrap()
        .expect("the mesh drawer drew something");
    let measures = measure_turtle.drawer().measures();

    // Area: the scene, discretised, and the mesh are the same triangles; the
    // measure drawer computes their area in closed form.
    let scene_area: Real = scene
        .iter()
        .map(|shape| surface_area(&discretize(&shape.geometry).unwrap()).unwrap())
        .sum();
    let mesh_area = surface_area(&plantgl::algo::discretize::Explicit::TriangleSet(mesh.clone()))
        .unwrap();

    assert!(
        (scene_area - mesh_area).abs() < 1e-4 * scene_area,
        "scene {scene_area} vs mesh {mesh_area}"
    );
    assert!(
        (measures.surface_area - mesh_area).abs() < 1e-3 * mesh_area,
        "measure {} vs mesh {mesh_area}",
        measures.surface_area
    );

    // Bounding box: the measure drawer bounds the *ideal* tube and the mesh
    // is the inscribed prism, so the analytic box encloses the mesh's and
    // exceeds it by at most one sagitta of the widest tube.
    //
    // The comparison is against the mesh's own vertices. A `Scene`'s box goes
    // through `BBoxComputer`, which transforms the child's box rather than its
    // points — upstream's behaviour, and conservative for a rotated shape — so
    // it is the wrong yardstick for a claim about tightness.
    let (lower, upper) = mesh_bbox(&mesh);
    let measured = measures.bbox.expect("the measure drawer bounded something");
    let sagitta = 0.05 * (1.0 - (std::f32::consts::PI / SLICES as Real).cos()) + 1e-5;
    for axis in 0..3 {
        assert!(
            measured.lower_left[axis] <= lower[axis] + 1e-5
                && measured.lower_left[axis] >= lower[axis] - sagitta,
            "axis {axis}: measured {:?} against mesh {lower:?}",
            measured.lower_left
        );
        assert!(
            measured.upper_right[axis] >= upper[axis] - 1e-5
                && measured.upper_right[axis] <= upper[axis] + sagitta,
            "axis {axis}: measured {:?} against mesh {upper:?}",
            measured.upper_right
        );
    }

    // The scene's own box, conservative as it is, still contains everything.
    let scene_box = scene.bbox().unwrap().unwrap();
    assert!(scene_box.contains(lower) && scene_box.contains(upper));

    // Shape count: every drawer saw the same commands.
    assert_eq!(scene.len(), measures.shape_count);
    assert_eq!(measures.segment_count, 5, "three F, one quad, one polygon leg");
}

/// The `crates/botany/src/turtle.rs` suite, translated.
///
/// Those tests are this repository's own, written against the hand-rolled
/// turtle #21 deletes. They are the behaviour the game currently depends on,
/// so they are re-run here against the port before that turtle goes away.
mod the_botany_turtle_suite {
    use super::*;

    /// `test_forward_moves_position`.
    #[test]
    fn forward_moves_position() {
        let mut turtle = Turtle::upright(MeasureDrawer::new());
        let start = turtle.position();
        turtle.forward(1.0).unwrap();
        assert!(start.x.abs() < Real::EPSILON && start.y.abs() < Real::EPSILON);
        assert!((turtle.position().y - 1.0).abs() < 0.01);
        assert_eq!(turtle.drawer().measures().segment_count, 1);
    }

    /// `test_push_pop_restores_state`.
    #[test]
    fn push_pop_restores_state() {
        let mut turtle = Turtle::upright(MeasureDrawer::new());
        turtle.forward(1.0).unwrap();
        turtle.push();
        turtle.forward(1.0).unwrap();
        turtle.pop().unwrap();
        let resumed = turtle.position();
        turtle.forward(0.5).unwrap();

        assert_eq!(turtle.drawer().measures().segment_count, 3);
        // The third segment starts where the first ended, not at Y = 2.
        assert!((resumed.y - 1.0).abs() < 0.01, "{resumed:?}");
    }

    /// `test_leaf_placement` — a leaf is a surface template instanced where
    /// the turtle stands.
    #[test]
    fn leaf_placement() {
        let mut turtle = Turtle::upright(SceneDrawer::new());
        turtle.forward(1.0).unwrap();
        turtle.surface("l", 0.5).unwrap();

        let scene = turtle.into_drawer().into_scene();
        assert_eq!(scene.len(), 2);
        let leaf = scene.shapes.last().unwrap().bbox().unwrap().unwrap();
        assert!((leaf.center().y - 1.0).abs() < 0.5, "{leaf:?}");
    }

    /// `test_flower_skipped_when_disabled` becomes: a template that is not in
    /// the library is an error, not a silently missing organ.
    #[test]
    fn an_unknown_surface_is_reported() {
        let mut turtle = Turtle::upright(SceneDrawer::new());
        assert!(matches!(
            turtle.surface("petal", 1.0),
            Err(plantgl::Error::UnknownSurface(_))
        ));
    }

    /// `test_width_changes`.
    #[test]
    fn width_changes_apply_to_later_segments() {
        let mut turtle = Turtle::upright(SceneDrawer::new());
        turtle.set_width(0.1).unwrap();
        turtle.forward(1.0).unwrap();
        turtle.set_width(0.02).unwrap();
        turtle.forward(1.0).unwrap();

        let scene = turtle.into_drawer().into_scene();
        assert_eq!(scene.len(), 2);
        let widths: Vec<Real> = scene
            .iter()
            .map(|shape| {
                let bbox = shape.bbox().unwrap().unwrap();
                // `size()` is upstream's half-extent, which is the radius.
                bbox.size().x
            })
            .collect();
        assert!((widths[0] - 0.1).abs() < 1e-5, "{widths:?}");
        assert!((widths[1] - 0.02).abs() < 1e-5, "{widths:?}");
    }

    /// `test_turn_changes_direction`.
    #[test]
    fn turning_changes_direction() {
        let mut turtle = Turtle::upright(MeasureDrawer::new());
        turtle.forward(1.0).unwrap();
        let corner = turtle.position();
        turtle.left(90.0);
        turtle.forward(1.0).unwrap();
        let end = turtle.position();

        let lateral = (end.x - corner.x).abs() + (end.z - corner.z).abs();
        assert!(lateral > 0.5, "expected lateral movement, got {lateral}");
    }

    /// `test_rotation_preserves_orthogonality`.
    #[test]
    fn rotation_preserves_orthogonality() {
        let mut turtle = Turtle::upright(MeasureDrawer::new());
        turtle.roll_left(45.0);
        assert!(turtle.is_valid());
        assert!((turtle.heading().norm() - 1.0).abs() < 0.001);
    }

    /// `test_empty_symbols_produce_empty_mesh`.
    #[test]
    fn an_empty_program_draws_nothing() {
        let turtle = Turtle::upright(SceneDrawer::new());
        assert!(turtle.into_drawer().into_scene().is_empty());
    }
}

// ---------------------------------------------------------------- T8.9 ----

/// Acceptance: a generalized cylinder over 20 `F` commands is one connected
/// surface — no interior seam vertices, and
/// `(rings - 1) * section_resolution * 2` triangles.
#[test]
fn a_generalized_cylinder_is_one_connected_surface() {
    let mut turtle = Turtle::upright(SceneDrawer::new());
    turtle.set_section_resolution(SLICES);
    turtle.set_width(0.05).unwrap();
    turtle.start_gc();
    for _ in 0..20 {
        turtle.left(3.0);
        turtle.forward(0.1).unwrap();
    }
    turtle.stop_gc().unwrap();

    let scene = turtle.into_drawer().into_scene();
    assert_eq!(scene.len(), 1, "a GC is one shape, however many F it covers");

    let geometry = &scene.shapes[0].geometry;
    assert!(matches!(geometry.as_ref(), Geometry::Extrusion(_)));

    let model = discretize(geometry).unwrap();
    let rings = 21; // the point `startGC` records, plus one per F
    assert_eq!(
        model.points().len(),
        rings * SLICES as usize,
        "a closed ring must not repeat its seam vertex"
    );

    let triangles = tessellate(&model).unwrap();
    assert_eq!(
        triangles.face_count(),
        (rings - 1) * SLICES as usize * 2,
        "one quad per ring gap per facet, two triangles each"
    );
}

/// Acceptance: a polygon of five coplanar points yields three triangles with a
/// consistent normal.
#[test]
fn a_five_point_polygon_is_three_consistent_triangles() {
    let mut turtle = Turtle::upright(SceneDrawer::new());
    // `f` rather than `F`: inside a polygon an `F` still draws its own tube,
    // as upstream's does — only a generalized cylinder suppresses that.
    turtle.start_polygon();
    turtle.polygon_point();
    for _ in 0..4 {
        turtle.left(72.0);
        turtle.f(0.3).unwrap();
        turtle.polygon_point();
    }
    turtle.stop_polygon(false).unwrap();

    let scene = turtle.into_drawer().into_scene();
    assert_eq!(scene.len(), 1);
    let Geometry::TriangleSet(mesh) = scene.shapes[0].geometry.as_ref() else {
        panic!("a polygon should be a TriangleSet");
    };
    assert_eq!(mesh.face_count(), 3);

    let first = mesh.face_normal(0).unwrap();
    for face in 1..mesh.face_count() {
        let normal = mesh.face_normal(face).unwrap();
        assert!(
            (normal - first).norm() < 1e-5,
            "face {face} faces the other way: {normal:?} vs {first:?}"
        );
    }
}

/// Acceptance: with `tropism = -Y` and `elasticity = 0.5`, a stem started
/// horizontally bends monotonically downward and ends with its heading closer
/// to `-Y`.
#[test]
fn gravitropism_bends_a_horizontal_stem_down() {
    let mut turtle = Turtle::upright(MeasureDrawer::new());
    turtle.set_head(Vec3::x(), Vec3::y()).unwrap();
    turtle.set_tropism(-Vec3::y());
    turtle.set_elasticity(0.5);

    let mut headings = vec![turtle.heading()];
    let mut heights = vec![turtle.position().y];
    for _ in 0..12 {
        turtle.forward(0.1).unwrap();
        headings.push(turtle.heading());
        heights.push(turtle.position().y);
    }

    // Monotonically downward: every step ends lower than the last, and every
    // heading is more downward than the last.
    for pair in heights.windows(2) {
        assert!(pair[1] < pair[0] + 1e-6, "the stem rose: {heights:?}");
    }
    for pair in headings.windows(2) {
        assert!(
            pair[1].y < pair[0].y + 1e-6,
            "the heading turned back up: {headings:?}"
        );
    }

    let end = *headings.last().unwrap();
    assert!(
        end.dot(&-Vec3::y()) > end.dot(&Vec3::x()),
        "the stem should end pointing more down than along: {end:?}"
    );
    assert!(turtle.is_valid(), "tropism left the frame skewed");
}

/// Tropism with zero elasticity changes nothing — the switch works.
#[test]
fn tropism_without_elasticity_is_inert() {
    let mut turtle = Turtle::upright(MeasureDrawer::new());
    turtle.set_tropism(-Vec3::y());
    turtle.forward(1.0).unwrap();
    assert!((turtle.heading() - Vec3::y()).norm() < 1e-6);
}

/// Acceptance: `setGuide` along an arc produces an axis whose sampled points
/// lie on that arc within 1e-3.
#[test]
fn a_guided_axis_follows_its_curve() {
    // A quarter circle of radius 1, from the origin heading +Y — the frame
    // the upright turtle starts in — curving towards +X.
    let radius = 1.0;
    let points: Vec<Point3> = (0..=200)
        .map(|i| {
            let t = std::f32::consts::FRAC_PI_2 * i as Real / 200.0;
            Point3::new(radius * (1.0 - t.cos()), radius * t.sin(), 0.0)
        })
        .collect();
    let arc = Curve3D::from(Polyline::new(points)).into_ref();
    let length = arc.length(200).unwrap();

    let mut turtle = Turtle::upright(MeasureDrawer::new());
    // The guide's reference frame starts at upstream's default heading, so a
    // turtle that is to follow a curve starting along +Y has to be pointed
    // the way the guide assumes: +Z.
    turtle.set_head(Vec3::z(), Vec3::x()).unwrap();
    turtle.set_guide(arc, length).unwrap();

    let mut sampled = vec![turtle.position()];
    for _ in 0..20 {
        turtle.forward(length / 20.0).unwrap();
        sampled.push(turtle.position());
    }

    // The guide maps its own frame onto the turtle's, and here the two
    // coincide, so the turtle walks the arc itself: the circle of radius 1
    // centred at (1, 0, 0), in the plane z = 0.
    let centre = Point3::new(radius, 0.0, 0.0);
    for point in &sampled {
        let offset = point - centre;
        assert!(
            (offset.norm() - radius).abs() < 1e-3,
            "a guided point left the arc by {}: {point:?}",
            (offset.norm() - radius).abs()
        );
        assert!(point.z.abs() < 1e-3, "the guide left its plane: {point:?}");
    }

    // And it walked the whole arc: a quarter turn onto +X. The heading lags
    // the end tangent by half a step, since each move turns onto the chord it
    // spans rather than onto the tangent at its far end.
    assert!(
        (turtle.heading() - Vec3::x()).norm() < 6e-2,
        "{:?}",
        turtle.heading()
    );
}

/// A cross-section set by the caller makes `F` sweep rather than draw a
/// cylinder — the square-stemmed mint case.
#[test]
fn a_custom_cross_section_makes_f_sweep() {
    use plantgl::math::Point2;
    use plantgl::scenegraph::curve::{Curve2D, Polyline2D};

    let square = Curve2D::from(Polyline2D::new(vec![
        Point2::new(-1.0, -1.0),
        Point2::new(1.0, -1.0),
        Point2::new(1.0, 1.0),
        Point2::new(-1.0, 1.0),
        Point2::new(-1.0, -1.0),
    ]))
    .into_ref();

    let mut turtle = Turtle::upright(SceneDrawer::new());
    turtle.set_width(0.1).unwrap();
    turtle.set_cross_section(square, true);
    turtle.forward(1.0).unwrap();

    let scene = turtle.into_drawer().into_scene();
    assert_eq!(scene.len(), 1);
    let Geometry::Extrusion(_) = scene.shapes[0].geometry.as_ref() else {
        panic!("a set cross-section should sweep, not draw a cylinder");
    };

    // A square tube of half-width 0.1 over a unit length: four faces of
    // 0.2 × 1.
    let area = surface_area(&discretize(&scene.shapes[0].geometry).unwrap()).unwrap();
    assert!((area - 0.8).abs() < 1e-4, "{area}");
}

/// `stopGC` while a branch is open sweeps the branch too: upstream draws an
/// open generalized cylinder on `pop`, so a branching plant is one sweep per
/// axis.
#[test]
fn a_branch_inside_a_gc_is_swept_on_pop() {
    let mut turtle = Turtle::upright(SceneDrawer::new());
    turtle.set_width(0.05).unwrap();
    turtle.start_gc();
    turtle.forward(0.5).unwrap();
    turtle.push();
    turtle.left(40.0);
    turtle.forward(0.4).unwrap();
    turtle.forward(0.4).unwrap();
    turtle.pop().unwrap();
    turtle.forward(0.5).unwrap();
    turtle.stop_gc().unwrap();

    let scene = turtle.into_drawer().into_scene();
    assert_eq!(scene.len(), 2, "the branch and the trunk are separate sweeps");
    for shape in scene.iter() {
        assert!(matches!(shape.geometry.as_ref(), Geometry::Extrusion(_)));
    }
}

/// `pop` on an empty stack is an error rather than upstream's warning.
#[test]
fn popping_an_empty_stack_is_an_error() {
    let mut turtle = Turtle::upright(MeasureDrawer::new());
    assert!(matches!(turtle.pop(), Err(plantgl::Error::EmptyStack)));
}

/// The section resolution is the LOD knob: halving it halves the triangles.
#[test]
fn section_resolution_scales_the_triangle_count() {
    fn triangles(resolution: u32) -> usize {
        let mut turtle = Turtle::upright(SceneDrawer::new());
        turtle.set_section_resolution(resolution);
        turtle.set_width(0.05).unwrap();
        turtle.forward(1.0).unwrap();
        let scene = turtle.into_drawer().into_scene();
        let model = discretize(&scene.shapes[0].geometry).unwrap();
        tessellate(&model).unwrap().face_count()
    }
    assert_eq!(triangles(16), 2 * triangles(8));
}
