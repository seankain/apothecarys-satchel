//! The acceptance criteria of T8.6 and T8.7, as tests.
//!
//! Part of the `plantgl` crate and therefore licensed CeCILL-C; see
//! crates/plantgl/LICENSE. Nothing here is translated from PlantGL — these are
//! checks *on* the translation.
//!
//! Each of these says something a rendering cannot:
//!
//! 1. **A rational circle is a circle.** The nine-point NURBS circle holds its
//!    radius to 1e-5 at 256 samples, which no weight or knot mistake survives.
//! 2. **Degree elevation is value-preserving.** Two different control nets
//!    describing the same curve must evaluate identically; a wrong de Casteljau
//!    almost never agrees with itself across both.
//! 3. **Analytic normals equal numeric ones.** A patch's partial derivatives
//!    are the thing a renderer shades with, and the finite difference of its
//!    own evaluator is the honest check on them.
//! 4. **`f32` is not enough for knots, and the port does not use it.** A
//!    200-control-point curve is where the margin runs out.
//! 5. **A sweep is the solid it should be.** An extrusion along a straight axis
//!    is the equivalent cylinder, and a sweep tapered to nothing is a cone.
//! 6. **The frames do not flip.** Sweeping a helix is the case a naive Frenet
//!    frame gets wrong, and this is the regression guard that says so.

use plantgl::algo::discretize::{discretize_with, DiscretizeCtx, Explicit};
use plantgl::algo::measure::{surface_area, volume};
use plantgl::math::{Point2, Point3, Real, Vec2, Vec3};
use plantgl::scenegraph::curve::{
    BezierCurve, BezierPatch, CtrlPointMatrix, Curve2D, Curve3D, NurbsCurve, NurbsCurve2D,
    NurbsPatch, ParametricCurve, Polyline2D,
};
use plantgl::scenegraph::function::QuantisedFunction;
use plantgl::scenegraph::mesh::Polyline;
use plantgl::scenegraph::primitive::{Cylinder, Extrusion};
use plantgl::Geometry;

const PI: Real = std::f32::consts::PI;

/// "Within 1%", as the issue words it.
const TOLERANCE: Real = 0.01;

/// The density the acceptance criteria are stated at.
const SLICES: u8 = 64;

fn ctx(curve_samples: u32) -> DiscretizeCtx {
    DiscretizeCtx {
        slices: SLICES,
        stacks: SLICES,
        curve_samples,
    }
}

fn relative_error(actual: Real, expected: Real) -> Real {
    ((actual - expected) / expected).abs()
}

// --- T8.6: curves and patches -----------------------------------------------

/// Acceptance: a NURBS circle of nine control points and the standard rational
/// weights evaluates to radius 1 ± 1e-5 at 256 samples.
#[test]
fn a_nurbs_circle_holds_its_radius_at_256_samples() {
    let circle = NurbsCurve2D::circle(1.0);
    assert_eq!(circle.ctrl_points.len(), 9);
    assert_eq!(circle.degree, 2);

    let mut worst = 0.0f32;
    for i in 0..=256 {
        let u = i as Real / 256.0;
        let radius = circle.eval(u).unwrap().coords.norm();
        worst = worst.max((radius - 1.0).abs());
    }
    assert!(worst <= 1e-5, "worst radius error {worst} exceeds 1e-5");
}

/// Acceptance: Bézier degree elevation is value-preserving.
#[test]
fn degree_elevation_preserves_the_curve() {
    let curve = BezierCurve::new(vec![
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(1.0, 4.0, -2.0),
        Point3::new(3.0, -1.0, 2.0),
        Point3::new(5.0, 2.0, 0.0),
    ]);

    // Three elevations in a row, so the check is not a one-step coincidence.
    let mut elevated = curve.clone();
    for step in 1..=3 {
        elevated = elevated.elevated();
        assert_eq!(elevated.degree(), curve.degree() + step);
        for i in 0..=128 {
            let u = i as Real / 128.0;
            let error = (curve.eval(u).unwrap() - elevated.eval(u).unwrap()).norm();
            assert!(
                error < 1e-4,
                "elevation {step} moved the curve by {error} at u = {u}"
            );
        }
    }
}

/// Acceptance: patch normals match finite-difference normals to 1e-4.
#[test]
fn patch_normals_match_finite_differences() {
    let net = || {
        CtrlPointMatrix::from_point_rows(vec![
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.8),
                Point3::new(0.0, 2.0, -0.2),
                Point3::new(0.0, 3.0, 0.0),
            ],
            vec![
                Point3::new(1.0, 0.0, 0.5),
                Point3::new(1.0, 1.0, 2.0),
                Point3::new(1.0, 2.0, 1.0),
                Point3::new(1.0, 3.0, -0.4),
            ],
            vec![
                Point3::new(2.0, 0.0, -0.3),
                Point3::new(2.0, 1.0, 0.9),
                Point3::new(2.0, 2.0, 1.4),
                Point3::new(2.0, 3.0, 0.2),
            ],
            vec![
                Point3::new(3.0, 0.0, 0.0),
                Point3::new(3.0, 1.0, -0.6),
                Point3::new(3.0, 2.0, 0.3),
                Point3::new(3.0, 3.0, 0.0),
            ],
        ])
        .unwrap()
    };

    let bezier = BezierPatch::new(net());
    let nurbs = NurbsPatch::new(net(), 3, 3);

    check_normals("BezierPatch", &|u, v| bezier.eval(u, v).unwrap(), &|u, v| {
        bezier.normal(u, v).unwrap()
    });
    check_normals("NurbsPatch", &|u, v| nurbs.eval(u, v).unwrap(), &|u, v| {
        nurbs.normal(u, v).unwrap()
    });
}

/// The finite-difference comparison itself, over the interior of the unit
/// square: the edges are left out because a central difference needs samples on
/// both sides of the parameter.
fn check_normals(
    name: &str,
    eval: &dyn Fn(Real, Real) -> Point3,
    normal: &dyn Fn(Real, Real) -> Vec3,
) {
    let h = 1e-3;
    for i in 1..16 {
        for j in 1..16 {
            let (u, v) = (i as Real / 16.0, j as Real / 16.0);
            let du = (eval(u + h, v) - eval(u - h, v)) / (2.0 * h);
            let dv = (eval(u, v + h) - eval(u, v - h)) / (2.0 * h);
            let numeric = du.cross(&dv).normalize();
            let error = (normal(u, v) - numeric).norm();
            assert!(error < 1e-4, "{name} normal at ({u}, {v}) is off by {error}");
        }
    }
}

/// Acceptance: the arc length of a straight NURBS line equals its chord.
#[test]
fn a_straight_nurbs_line_has_the_length_of_its_chord() {
    for degree in 1..=4 {
        let ctrl: Vec<Point3> = (0..=degree + 1)
            .map(|i| {
                let t = i as Real / (degree + 1) as Real;
                Point3::new(2.0 * t, -3.0 * t, 6.0 * t)
            })
            .collect();
        let end = *ctrl.last().unwrap();
        let curve = Curve3D::from(NurbsCurve::new(ctrl, degree as usize));
        let chord = (end - Point3::origin()).norm();
        assert!(
            relative_error(curve.length(128).unwrap(), chord) < 1e-4,
            "degree {degree}: length {} against chord {chord}",
            curve.length(128).unwrap()
        );
    }
}

/// Acceptance: an explicit precision test on a 200-control-point curve.
///
/// This is the case the module header of `scenegraph::curve::spline` is about.
/// A degree-3 NURBS over 200 control points has interior knots ~5e-3 apart, and
/// `basis_functions` divides by differences of them; in `f32` that division
/// starts from seven significant digits and keeps rather fewer.
///
/// The check is against an *independent* evaluator: the textbook Cox–de Boor
/// recursion, written out below in `f64` and deliberately not sharing a line of
/// code with the port's iterative de Boor. Agreement to 1e-4 over a curve
/// 100 units long is agreement to a relative 1e-6, which the port only reaches
/// because it widens the knots; the same algorithm in `f32` throughout does
/// not.
#[test]
fn precision_holds_on_a_two_hundred_control_point_curve() {
    const COUNT: usize = 200;
    let control: Vec<Point3> = (0..COUNT)
        .map(|i| {
            let t = i as Real / (COUNT - 1) as Real;
            // Not a straight line: a wave, so an error in any one basis
            // function moves the point instead of being absorbed by its
            // neighbours summing to the same collinear answer.
            Point3::new(100.0 * t, (t * 17.0).sin(), (t * 11.0).cos())
        })
        .collect();
    let curve = NurbsCurve::new(control.clone(), 3);
    assert_eq!(curve.knots.len(), COUNT + 4);

    assert!(
        (curve.eval(0.0).unwrap() - control[0]).norm() < 1e-5,
        "the curve does not start on its first control point"
    );
    assert!(
        (curve.eval(1.0).unwrap() - control[COUNT - 1]).norm() < 1e-5,
        "the curve does not end on its last control point"
    );

    let knots: Vec<f64> = curve.knots.iter().map(|k| *k as f64).collect();
    let mut worst = 0.0f64;
    for i in 0..2000 {
        let u = i as f64 / 2000.0;
        let reference = cox_de_boor(&control, &knots, 3, u);
        let point = curve.eval(u as Real).unwrap();
        let error = ((point.x as f64 - reference.0).powi(2)
            + (point.y as f64 - reference.1).powi(2)
            + (point.z as f64 - reference.2).powi(2))
        .sqrt();
        worst = worst.max(error);
    }
    assert!(worst < 1e-4, "worst deviation from Cox–de Boor {worst}");
}

/// The recursive definition of a B-spline, straight from the definition and in
/// `f64`: `N_{i,0}` is an indicator function and `N_{i,p}` is the two-term
/// recurrence over it. Quadratic in the control count and far too slow to ship,
/// which is the point — it shares nothing with the evaluator it checks.
fn cox_de_boor(control: &[Point3], knots: &[f64], degree: usize, u: f64) -> (f64, f64, f64) {
    fn basis(i: usize, p: usize, u: f64, knots: &[f64]) -> f64 {
        if p == 0 {
            return if knots[i] <= u && u < knots[i + 1] {
                1.0
            } else {
                0.0
            };
        }
        let left_span = knots[i + p] - knots[i];
        let right_span = knots[i + p + 1] - knots[i + 1];
        let left = if left_span > 0.0 {
            (u - knots[i]) / left_span * basis(i, p - 1, u, knots)
        } else {
            0.0
        };
        let right = if right_span > 0.0 {
            (knots[i + p + 1] - u) / right_span * basis(i + 1, p - 1, u, knots)
        } else {
            0.0
        };
        left + right
    }

    let mut point = (0.0, 0.0, 0.0);
    for (i, c) in control.iter().enumerate() {
        let n = basis(i, degree, u, knots);
        if n == 0.0 {
            continue;
        }
        point.0 += c.x as f64 * n;
        point.1 += c.y as f64 * n;
        point.2 += c.z as f64 * n;
    }
    point
}

// --- T8.7: the generalized cylinder -----------------------------------------

fn circle_section(radius: Real, slices: u8) -> Curve2D {
    Curve2D::from(Polyline2D::circle(radius, slices))
}

fn straight_axis(height: Real, segments: usize) -> Curve3D {
    Curve3D::from(Polyline::new(
        (0..=segments)
            .map(|i| Point3::new(0.0, 0.0, height * i as Real / segments as Real))
            .collect(),
    ))
}

/// Acceptance: an `Extrusion` of a circle along a straight axis is within 1%
/// of the equivalent `Cylinder`'s surface area.
#[test]
fn an_extrusion_along_a_straight_axis_is_its_cylinder() {
    let (radius, height) = (0.5, 2.0);
    let extrusion = Extrusion::new(
        straight_axis(height, 8).into_ref(),
        circle_section(radius, SLICES).into_ref(),
    )
    .with_solid(true);

    let swept = discretize_with(&Geometry::from(extrusion), ctx(64)).expect("sweep");
    let cylinder = discretize_with(
        &Geometry::from(Cylinder::new(radius, height, true, SLICES)),
        ctx(64),
    )
    .expect("cylinder");

    let swept_area = surface_area(&swept).expect("swept area");
    let cylinder_area = surface_area(&cylinder).expect("cylinder area");
    assert!(
        relative_error(swept_area, cylinder_area) < TOLERANCE,
        "swept area {swept_area} against the cylinder's {cylinder_area}"
    );

    // And against the ideal cylinder, which both approach from below.
    let ideal = std::f32::consts::TAU * radius * height + 2.0 * PI * radius * radius;
    assert!(
        relative_error(swept_area, ideal) < TOLERANCE,
        "swept area {swept_area} against the ideal {ideal}"
    );

    let swept_volume = volume(&swept).expect("swept volume");
    assert!(
        relative_error(swept_volume, PI * radius * radius * height) < TOLERANCE,
        "swept volume {swept_volume}"
    );
}

/// Acceptance: a radius `QuantisedFunction` from 1.0 to 0.0 produces a cone
/// within 1% of the analytic volume.
#[test]
fn a_radius_profile_tapering_to_zero_produces_a_cone() {
    let (radius, height) = (1.0, 3.0);
    let extrusion = Extrusion::with_radius_profile(
        straight_axis(height, 16).into_ref(),
        circle_section(radius, SLICES).into_ref(),
        &QuantisedFunction::ramp(1.0, 0.0),
        17,
    )
    .with_solid(true);

    let cone = discretize_with(&Geometry::from(extrusion), ctx(64)).expect("cone");
    let measured = volume(&cone).expect("cone volume");
    let analytic = PI * radius * radius * height / 3.0;
    assert!(
        relative_error(measured, analytic) < TOLERANCE,
        "swept cone volume {measured} against the analytic {analytic}"
    );
}

/// Acceptance: sweeping a circle along a helix produces **zero frame flips** —
/// the twist between adjacent rings stays under 1°.
///
/// This is the single most important regression guard in the crate. A Frenet
/// frame is the obvious way to sweep a curve and the wrong one: it rotates
/// about the tangent at the rate of the curve's torsion, so on this helix it
/// would wind the cross-section through `τ · L` radians end to end — computed
/// below, and hundreds of degrees. Worse, it is undefined wherever the
/// curvature vanishes, so an axis with an inflection flips the mesh inside out
/// at that point. The port carries rotation-minimising frames by double
/// reflection instead, and this test is what says so.
#[test]
fn sweeping_a_circle_along_a_helix_produces_no_frame_flips() {
    // A helix (a cos t, a sin t, b t) over four turns.
    let (a, b) = (1.0f32, 0.35f32);
    let turns = 4.0;
    let segments = 400;
    let t_of = |i: usize| turns * std::f32::consts::TAU * i as Real / segments as Real;

    let axis = Curve3D::from(Polyline::new(
        (0..=segments)
            .map(|i| {
                let t = t_of(i);
                Point3::new(a * t.cos(), a * t.sin(), b * t)
            })
            .collect(),
    ));

    let ring_points = 12u8;
    let extrusion = Extrusion::new(
        axis.clone().into_ref(),
        circle_section(0.15, ring_points).into_ref(),
    );
    let mesh = discretize_with(&Geometry::from(extrusion), ctx(64)).expect("sweep");
    let Explicit::QuadSet(quads) = &mesh else {
        panic!("an open extrusion meshes to a quad set");
    };
    let points = &quads.model.points;
    let ring_size = ring_points as usize;
    let rings = points.len() / ring_size;
    assert_eq!(rings, segments + 1, "one ring per axis sample");

    // The axis as the sweep saw it: a polyline is parameterised by point index,
    // so ring `i` sits at parameter `i`. Its own tangents are the ones the
    // frames were built from, so any angle left over after transporting by them
    // is twist the frame chain introduced rather than a sampling artefact.
    let centre = |ring: usize| axis.eval(ring as Real).unwrap();
    let tangent = |ring: usize| axis.tangent(ring as Real).unwrap().normalize();

    let mut worst_twist = 0.0f32;
    for ring in 0..rings - 1 {
        // The reference vector of each ring: its first vertex, relative to the
        // axis. It is perpendicular to the tangent by construction.
        let here = points[ring * ring_size] - centre(ring);
        let there = points[(ring + 1) * ring_size] - centre(ring + 1);

        // Carry `here` across by the minimal rotation between the two tangents,
        // which is the rotation that adds no twist. Whatever angle is left is
        // twist the sweep introduced.
        let carried = rotate_between(tangent(ring), tangent(ring + 1), here);
        let cosine = carried.normalize().dot(&there.normalize()).clamp(-1.0, 1.0);
        worst_twist = worst_twist.max(cosine.acos().to_degrees());
    }

    assert!(
        worst_twist < 1.0,
        "adjacent rings twist by {worst_twist}°, which is a frame flip"
    );

    // The test has teeth: a Frenet frame on this helix would have wound the
    // cross-section through this much in total, so "no twist" is a real claim
    // and not something any frame chain would satisfy.
    let torsion = b / (a * a + b * b);
    let arc_length = (a * a + b * b).sqrt() * turns * std::f32::consts::TAU;
    let frenet_twist = (torsion * arc_length).to_degrees();
    assert!(
        frenet_twist > 180.0,
        "the helix should be torsional enough to matter, but a Frenet frame \
         would only wind through {frenet_twist}°"
    );
}

/// Rodrigues' formula for the minimal rotation taking `from` to `to`, applied
/// to `v`. This is the rotation that introduces no twist about either vector,
/// so it is the right yardstick for how much twist a frame chain added.
fn rotate_between(from: Vec3, to: Vec3, v: Vec3) -> Vec3 {
    let axis = from.cross(&to);
    let sine = axis.norm();
    if sine < 1e-9 {
        return v;
    }
    let axis = axis / sine;
    let angle = sine.atan2(from.dot(&to));
    let (s, c) = angle.sin_cos();
    v * c + axis.cross(&v) * s + axis * (axis.dot(&v) * (1.0 - c))
}

/// A swept surface has to be watertight where it claims to be solid: every edge
/// shared by exactly two faces. A cap that misses the seam vertex, or a ring
/// stitched to the wrong neighbour, fails here and nowhere else.
#[test]
fn a_solid_extrusion_is_edge_manifold() {
    let extrusion = Extrusion::new(
        straight_axis(2.0, 5).into_ref(),
        circle_section(0.4, 10).into_ref(),
    )
    .with_solid(true);
    let mesh = discretize_with(&Geometry::from(extrusion), ctx(64)).expect("sweep");
    let Explicit::FaceSet(faces) = &mesh else {
        panic!("a solid extrusion meshes to a face set");
    };

    let mut edges: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();
    for face in &faces.indices {
        for (i, corner) in face.iter().enumerate() {
            let next = face[(i + 1) % face.len()];
            let key = (*corner.min(&next), *corner.max(&next));
            *edges.entry(key).or_default() += 1;
        }
    }
    let unshared: Vec<_> = edges.iter().filter(|(_, uses)| **uses != 2).collect();
    assert!(
        unshared.is_empty(),
        "a solid sweep must be closed, but these edges are not shared by two \
         faces: {unshared:?}"
    );
}

/// Both end caps face outward — the check that caught upstream's inverted base.
///
/// Upstream fans both caps in the same vertex order, so the near one points
/// into the solid. Its own `VolComputer` cannot see that (it sums *absolute*
/// tetrahedra about the centroid) and neither can a face count or an area, so
/// the failure is invisible to every other comparison. A signed volume sees it
/// immediately — but only if the solid is placed away from the origin, because
/// a cap lying *in* the plane through the origin contributes nothing either
/// way. Hence the axis from `z = 1` to `z = 3`: it puts both caps where a flip
/// cannot hide.
#[test]
fn a_solid_sweep_winds_both_caps_outward() {
    let slices = 64u8;
    let axis = Curve3D::from(Polyline::new(vec![
        Point3::new(0.0, 0.0, 1.0),
        Point3::new(0.0, 0.0, 2.0),
        Point3::new(0.0, 0.0, 3.0),
    ]));
    let extrusion = Extrusion::new(axis.into_ref(), circle_section(1.0, slices).into_ref())
        .with_solid(true);
    let mesh = discretize_with(&Geometry::from(extrusion), ctx(64)).expect("sweep");

    // The prism over a regular n-gon of circumradius 1, two units tall. Not
    // π·2: the cross-section is the inscribed polygon, and at 64 sides the
    // difference is a tenth of a percent — well above the 1e-3 asserted here.
    let ngon_area = slices as Real / 2.0 * (std::f32::consts::TAU / slices as Real).sin();
    let measured = volume(&mesh).expect("volume");
    assert!(
        (measured - ngon_area * 2.0).abs() < 1e-3,
        "signed volume {measured} against the prism's {}: an inverted cap would \
         read {} instead",
        ngon_area * 2.0,
        ngon_area * 2.0 + 2.0 * ngon_area / 3.0
    );
}

/// An open sweep is a tube, not a solid: it has a boundary at each end, and the
/// port refuses to report a volume for it rather than returning zero.
#[test]
fn an_open_extrusion_has_no_volume() {
    let extrusion = Extrusion::new(
        straight_axis(1.0, 4).into_ref(),
        circle_section(0.3, 8).into_ref(),
    );
    let mesh = discretize_with(&Geometry::from(extrusion), ctx(64)).expect("sweep");
    assert!(!mesh.is_solid());
    assert!(volume(&mesh).is_err());
    assert!(surface_area(&mesh).unwrap() > 0.0);
}

/// A twist of a whole turn is the identity, and a twist of a quarter turn is
/// not — the cheapest end-to-end check that `orientation` is applied in the
/// cross-section plane, in radians, and by the same amount everywhere when the
/// list holds one entry.
#[test]
fn a_full_turn_of_orientation_leaves_the_sweep_where_it_was() {
    let section = || {
        Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 0.5),
            Point2::new(-1.0, 0.0),
        ]))
    };
    let swept = |orientation: Vec<Real>| {
        let extrusion = Extrusion::new(straight_axis(2.0, 4).into_ref(), section().into_ref())
            .with_orientation(orientation);
        discretize_with(&Geometry::from(extrusion), ctx(64)).unwrap()
    };

    let plain = swept(Vec::new());
    let turned = swept(vec![std::f32::consts::TAU]);
    assert_eq!(plain.points().len(), turned.points().len());
    for (i, (p, q)) in plain.points().iter().zip(turned.points()).enumerate() {
        assert!(
            (p - q).norm() < 1e-5,
            "vertex {i} moved by {} under a full turn",
            (p - q).norm()
        );
    }

    let quarter = swept(vec![std::f32::consts::FRAC_PI_2]);
    let moved = plain
        .points()
        .iter()
        .zip(quarter.points())
        .map(|(p, q)| (p - q).norm())
        .fold(0.0f32, f32::max);
    assert!(moved > 0.5, "a quarter turn should move the sweep, not {moved}");
}

/// Scaling the cross-section scales the swept area with it, which is what a
/// per-knot `scale` list has to mean for a tapering stem to be believable.
#[test]
fn a_uniform_scale_scales_the_swept_area() {
    let build = |scale: Real| {
        let mut extrusion = Extrusion::new(
            straight_axis(2.0, 4).into_ref(),
            circle_section(1.0, 32).into_ref(),
        );
        extrusion.scale = vec![Vec2::new(scale, scale)];
        surface_area(&discretize_with(&Geometry::from(extrusion), ctx(64)).unwrap()).unwrap()
    };
    assert!(
        relative_error(build(2.0), 2.0 * build(1.0)) < 1e-4,
        "doubling the cross-section should double the lateral area"
    );
}
