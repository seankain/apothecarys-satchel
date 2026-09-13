//! The acceptance criteria of T8.4 and T8.5, as tests.
//!
//! Part of the `plantgl` crate and therefore licensed CeCILL-C; see
//! crates/plantgl/LICENSE. Nothing here is translated from PlantGL — these are
//! checks *on* the translation.
//!
//! "The mesh looks plausible" is not a test. These four say something a
//! rendering cannot:
//!
//! 1. **Convergence.** Discretised area and volume approach the closed form as
//!    the slice count rises. One assertion catches a missing cap, a
//!    double-counted vertex and a reversed winding at once.
//! 2. **Manifoldness.** Every edge of a `solid` mesh is shared by exactly two
//!    faces. A gap in a cap or a duplicated seam vertex fails this and nothing
//!    else.
//! 3. **Winding.** The divergence theorem over a closed mesh gives the volume
//!    only if every face points outward; one inverted face makes it wrong by
//!    twice that face's contribution.
//! 4. **Monotonicity.** The approximation is inscribed, so refining it may not
//!    overshoot.

use std::collections::HashMap;

use plantgl::algo::discretize::{discretize_with, DiscretizeCtx, Explicit};
use plantgl::algo::measure::{surface_area, volume};
use plantgl::algo::tessellate::tessellate;
use plantgl::math::{Point2, Point3, Real};
use plantgl::scenegraph::curve::{Curve2D, Polyline2D};
use plantgl::scenegraph::mesh::FaceIndex;
use plantgl::scenegraph::primitive::{
    Box3, Cone, Cylinder, Disc, ElevationGrid, Frustum, HeightField, Paraboloid, Revolution,
    Sphere, Swung,
};
use plantgl::Geometry;

const PI: Real = std::f32::consts::PI;
const TAU: Real = std::f32::consts::TAU;

/// The slice count the acceptance criteria are stated at.
const ACCEPTANCE_SLICES: u8 = 64;

/// "Within 1%", as the issue words it.
const TOLERANCE: Real = 0.01;

fn ctx(slices: u8, stacks: u8) -> DiscretizeCtx {
    DiscretizeCtx {
        slices,
        stacks,
        curve_samples: 64,
    }
}

fn at(geometry: Geometry, slices: u8, stacks: u8) -> Explicit {
    discretize_with(&geometry, ctx(slices, stacks)).expect("discretisation")
}

fn relative_error(actual: Real, expected: Real) -> Real {
    ((actual - expected) / expected).abs()
}

// --- 1. Convergence to the closed forms ------------------------------------

/// Acceptance (T8.4): a unit sphere's area converges to 4π and its volume to
/// 4π/3 within 1% at 64 slices.
#[test]
fn a_unit_sphere_converges_to_its_closed_forms() {
    let mesh = at(
        Geometry::from(Sphere::sized(1.0)),
        ACCEPTANCE_SLICES,
        ACCEPTANCE_SLICES,
    );
    assert!(
        relative_error(surface_area(&mesh).unwrap(), 4.0 * PI) < TOLERANCE,
        "area {} vs 4π = {}",
        surface_area(&mesh).unwrap(),
        4.0 * PI
    );
    assert!(
        relative_error(volume(&mesh).unwrap(), 4.0 * PI / 3.0) < TOLERANCE,
        "volume {} vs 4π/3 = {}",
        volume(&mesh).unwrap(),
        4.0 * PI / 3.0
    );
}

/// Acceptance (T8.4): a unit cylinder's area converges to 2πrh + caps.
#[test]
fn a_unit_cylinder_converges_to_its_closed_forms() {
    let (r, h) = (1.0, 1.0);
    let mesh = at(
        Geometry::from(Cylinder::sized(r, h)),
        ACCEPTANCE_SLICES,
        ACCEPTANCE_SLICES,
    );
    let expected_area = TAU * r * h + 2.0 * PI * r * r;
    assert!(
        relative_error(surface_area(&mesh).unwrap(), expected_area) < TOLERANCE,
        "area {} vs {expected_area}",
        surface_area(&mesh).unwrap()
    );
    assert!(
        relative_error(volume(&mesh).unwrap(), PI * r * r * h) < TOLERANCE,
        "volume {} vs {}",
        volume(&mesh).unwrap(),
        PI * r * r * h
    );
}

#[test]
fn a_cone_converges_to_its_closed_forms() {
    let (r, h) = (1.0, 2.0);
    let mesh = at(
        Geometry::from(Cone::sized(r, h)),
        ACCEPTANCE_SLICES,
        ACCEPTANCE_SLICES,
    );
    // Lateral πrl with slant l = √(r² + h²), plus the base cap.
    let slant = (r * r + h * h).sqrt();
    let expected_area = PI * r * slant + PI * r * r;
    assert!(
        relative_error(surface_area(&mesh).unwrap(), expected_area) < TOLERANCE,
        "area {} vs {expected_area}",
        surface_area(&mesh).unwrap()
    );
    assert!(
        relative_error(volume(&mesh).unwrap(), PI * r * r * h / 3.0) < TOLERANCE,
        "volume {}",
        volume(&mesh).unwrap()
    );
}

#[test]
fn a_frustum_converges_to_its_closed_forms() {
    let (r, h, taper) = (2.0, 3.0, 0.5);
    let top = r * taper;
    let mesh = at(
        Geometry::from(Frustum::sized(r, h, taper)),
        ACCEPTANCE_SLICES,
        ACCEPTANCE_SLICES,
    );
    let slant = ((r - top).powi(2) + h * h).sqrt();
    let expected_area = PI * (r + top) * slant + PI * r * r + PI * top * top;
    let expected_volume = PI * h / 3.0 * (r * r + r * top + top * top);
    assert!(
        relative_error(surface_area(&mesh).unwrap(), expected_area) < TOLERANCE,
        "area {} vs {expected_area}",
        surface_area(&mesh).unwrap()
    );
    assert!(
        relative_error(volume(&mesh).unwrap(), expected_volume) < TOLERANCE,
        "volume {} vs {expected_volume}",
        volume(&mesh).unwrap()
    );
}

#[test]
fn a_disc_converges_to_pi_r_squared() {
    let mesh = at(Geometry::from(Disc::sized(2.0)), ACCEPTANCE_SLICES, 8);
    assert!(
        relative_error(surface_area(&mesh).unwrap(), PI * 4.0) < TOLERANCE,
        "area {}",
        surface_area(&mesh).unwrap()
    );
}

#[test]
fn a_box_is_exact_at_any_density() {
    // A box has no curvature to approximate, so it is not "within 1%" — it is
    // right, and the density does not enter into it.
    for slices in [3, 8, 64] {
        let mesh = at(Geometry::from(Box3::cube(3.0)), slices, slices);
        assert!(relative_error(surface_area(&mesh).unwrap(), 54.0) < 1e-4);
        assert!(relative_error(volume(&mesh).unwrap(), 27.0) < 1e-4);
    }
}

/// A paraboloid of shape 2: V = πr²h/2, the classic "half the cylinder".
#[test]
fn a_paraboloid_converges_to_half_its_cylinder() {
    let (r, h) = (1.0, 2.0);
    let mesh = at(
        Geometry::from(Paraboloid::sized(r, h, 2.0)),
        ACCEPTANCE_SLICES,
        ACCEPTANCE_SLICES,
    );
    let expected = PI * r * r * h / 2.0;
    assert!(
        relative_error(volume(&mesh).unwrap(), expected) < TOLERANCE,
        "volume {} vs {expected}",
        volume(&mesh).unwrap()
    );
}

/// A revolution of a straight profile is a cone frustum, so it has a closed
/// form even though `Revolution` in general does not.
#[test]
fn a_revolution_of_a_straight_profile_converges_to_its_frustum() {
    let profile = Curve2D::from(Polyline2D::new(vec![
        Point2::new(2.0, 0.0),
        Point2::new(1.0, 3.0),
    ]))
    .into_ref();
    let mesh = at(
        Geometry::from(Revolution::from_profile(profile)),
        ACCEPTANCE_SLICES,
        8,
    );
    // Lateral surface only: the profile does not meet the axis, so there are
    // no caps.
    let slant = (1.0f32 + 9.0).sqrt();
    let expected = PI * (2.0 + 1.0) * slant;
    assert!(
        relative_error(surface_area(&mesh).unwrap(), expected) < TOLERANCE,
        "area {} vs {expected}",
        surface_area(&mesh).unwrap()
    );
}

#[test]
fn an_elevation_grid_of_a_plane_has_the_planes_area() {
    // A 5×5 flat grid at unit spacing is a 4×4 square.
    let grid = ElevationGrid::from_heights(HeightField::flat(5, 5).unwrap());
    let mesh = at(Geometry::from(grid), 8, 8);
    assert!(relative_error(surface_area(&mesh).unwrap(), 16.0) < 1e-4);
}

// --- 2. Edge manifoldness ---------------------------------------------------

/// Every edge of a closed mesh, with how many faces use it and in which
/// direction.
fn edge_use(mesh: &Explicit) -> HashMap<(u32, u32), (usize, i32)> {
    let mut edges: HashMap<(u32, u32), (usize, i32)> = HashMap::new();
    let mut record = |a: u32, b: u32| {
        // Undirected key, with the direction recorded separately so a pair of
        // faces traversing the edge the same way shows up as ±2 rather than 0.
        let (key, direction) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
        let slot = edges.entry(key).or_insert((0, 0));
        slot.0 += 1;
        slot.1 += direction;
    };

    fn walk<I: FaceIndex>(
        mesh: &plantgl::scenegraph::mesh::IndexedMesh<I>,
        record: &mut impl FnMut(u32, u32),
    ) {
        for i in 0..mesh.face_count() {
            let size = mesh.face_size(i).unwrap_or(0);
            for j in 0..size {
                let a = mesh.face_point_index_at(i, j).unwrap();
                let b = mesh.face_point_index_at(i, (j + 1) % size).unwrap();
                record(a, b);
            }
        }
    }

    match mesh {
        Explicit::TriangleSet(m) => walk(m, &mut record),
        Explicit::QuadSet(m) => walk(m, &mut record),
        Explicit::FaceSet(m) => walk(m, &mut record),
        Explicit::PointSet(_) | Explicit::Polyline(_) => {}
    }
    edges
}

/// Acceptance (T8.4): every `solid` mesh is edge-manifold, and the winding is
/// consistent.
fn assert_edge_manifold(name: &str, mesh: &Explicit) {
    assert!(mesh.is_solid(), "{name} should be solid");
    let edges = edge_use(mesh);
    assert!(!edges.is_empty(), "{name} has no edges");

    for ((a, b), (count, direction)) in &edges {
        assert_eq!(
            *count, 2,
            "{name}: edge ({a}, {b}) is used by {count} faces, not 2"
        );
        // Two faces sharing an edge traverse it in opposite directions when
        // they are wound consistently, so the directions cancel.
        assert_eq!(
            *direction, 0,
            "{name}: edge ({a}, {b}) is traversed the same way twice — \
             one of its faces is wound backwards"
        );
    }
}

#[test]
fn every_solid_primitive_is_edge_manifold() {
    let cases: Vec<(&str, Geometry)> = vec![
        ("box", Geometry::from(Box3::default())),
        ("sphere", Geometry::from(Sphere::sized(1.0))),
        ("cone", Geometry::from(Cone::sized(1.0, 2.0))),
        ("cylinder", Geometry::from(Cylinder::sized(1.0, 2.0))),
        ("frustum", Geometry::from(Frustum::sized(1.0, 2.0, 0.4))),
        (
            "paraboloid",
            Geometry::from(Paraboloid::sized(1.0, 2.0, 2.0)),
        ),
    ];
    for (name, geometry) in cases {
        for slices in [3u8, 8, 17, 64] {
            let mesh = at(geometry.clone(), slices, slices.max(4));
            assert_edge_manifold(&format!("{name} at {slices} slices"), &mesh);
        }
    }
}

#[test]
fn a_solid_revolution_is_edge_manifold() {
    // A profile that starts and ends on the axis closes the solid.
    let profile = Curve2D::from(Polyline2D::new(vec![
        Point2::new(0.0, 0.0),
        Point2::new(1.0, 1.0),
        Point2::new(1.0, 2.0),
        Point2::new(0.0, 3.0),
    ]))
    .into_ref();
    for slices in [3u8, 8, 32] {
        let mesh = at(
            Geometry::from(Revolution::from_profile(profile.clone())),
            slices,
            8,
        );
        assert_edge_manifold(&format!("revolution at {slices} slices"), &mesh);
    }
}

#[test]
fn a_closed_swung_is_edge_manifold() {
    let profile = |radius: Real| {
        Curve2D::from(Polyline2D::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(radius, 1.0),
            Point2::new(0.0, 2.0),
        ]))
        .into_ref()
    };
    let swung = Swung::linear(
        vec![profile(1.0), profile(1.6), profile(1.0)],
        vec![0.0, PI, TAU],
    );
    let mesh = at(Geometry::from(swung), 16, 8);
    assert_edge_manifold("swung", &mesh);
}

#[test]
fn tessellating_a_solid_keeps_it_edge_manifold() {
    // The cylinder is the interesting one: it is a FaceSet of quads and
    // triangles, so the fan has to cut it without leaving a T-junction.
    for geometry in [
        Geometry::from(Cylinder::sized(1.0, 2.0)),
        Geometry::from(Box3::default()),
        Geometry::from(Frustum::sized(1.0, 2.0, 0.3)),
    ] {
        let mesh = at(geometry, 12, 8);
        let triangles = tessellate(&mesh).unwrap();
        assert_edge_manifold("tessellated", &Explicit::TriangleSet(triangles));
    }
}

#[test]
fn an_open_primitive_has_a_boundary_and_so_is_not_manifold() {
    // The negative control: without this, the manifold test above could be
    // passing because the check is vacuous.
    let open = at(
        Geometry::from(Cylinder::default().with_solid(false)),
        8,
        8,
    );
    assert!(!open.is_solid());
    let boundary = edge_use(&open)
        .values()
        .filter(|(count, _)| *count == 1)
        .count();
    assert!(
        boundary > 0,
        "an open cylinder must have boundary edges, found none"
    );
}

// --- 3. Consistent winding, by divergence -----------------------------------

/// The divergence theorem applied face by face: `∮ x·n dA / 3` is the enclosed
/// volume when every normal points outward, and is wrong otherwise.
///
/// The winding check that matters: this agrees with the closed form only if
/// the whole mesh agrees about which way is out.
fn divergence_volume(mesh: &Explicit) -> Real {
    fn walk<I: FaceIndex>(mesh: &plantgl::scenegraph::mesh::IndexedMesh<I>) -> Real {
        let mut total = 0.0;
        for i in 0..mesh.face_count() {
            let size = mesh.face_size(i).unwrap_or(0);
            if size < 3 {
                continue;
            }
            let p = |j: usize| mesh.face_point_at(i, j).unwrap();
            let origin = p(0);
            for j in 1..size - 1 {
                let (a, b, c) = (origin, p(j), p(j + 1));
                // The unnormalised normal times the area, dotted with a point
                // on the face — the flux of the position field through it.
                let normal = (b - a).cross(&(c - a));
                let centroid = (a.coords + b.coords + c.coords) / 3.0;
                total += centroid.dot(&normal) / 2.0;
            }
        }
        total / 3.0
    }

    let signed = match mesh {
        Explicit::TriangleSet(m) => walk(m),
        Explicit::QuadSet(m) => walk(m),
        Explicit::FaceSet(m) => walk(m),
        Explicit::PointSet(_) | Explicit::Polyline(_) => 0.0,
    };
    // `ccw = false` declares the corner lists clockwise, which flips the sign.
    let ccw = match mesh {
        Explicit::TriangleSet(m) => m.model.ccw,
        Explicit::QuadSet(m) => m.model.ccw,
        Explicit::FaceSet(m) => m.model.ccw,
        _ => true,
    };
    if ccw {
        signed
    } else {
        -signed
    }
}

/// Acceptance (T8.4): consistent winding, verified by the divergence test.
#[test]
fn every_solid_primitive_winds_consistently_outward() {
    let cases: Vec<(&str, Geometry, Real)> = vec![
        ("box", Geometry::from(Box3::cube(2.0)), 8.0),
        ("sphere", Geometry::from(Sphere::sized(1.0)), 4.0 * PI / 3.0),
        ("cone", Geometry::from(Cone::sized(1.0, 3.0)), PI / 3.0 * 3.0),
        (
            "cylinder",
            Geometry::from(Cylinder::sized(1.0, 2.0)),
            PI * 2.0,
        ),
        (
            "frustum",
            Geometry::from(Frustum::sized(2.0, 3.0, 0.5)),
            PI * 3.0 / 3.0 * (4.0 + 2.0 + 1.0),
        ),
        (
            "paraboloid",
            Geometry::from(Paraboloid::sized(1.0, 2.0, 2.0)),
            PI * 2.0 / 2.0,
        ),
    ];

    for (name, geometry, expected) in cases {
        let mesh = at(geometry, ACCEPTANCE_SLICES, ACCEPTANCE_SLICES);
        let divergence = divergence_volume(&mesh);
        assert!(
            divergence > 0.0,
            "{name}: divergence volume {divergence} is not positive — the \
             mesh is wound inward"
        );
        assert!(
            relative_error(divergence, expected) < TOLERANCE,
            "{name}: divergence volume {divergence} vs closed form {expected}"
        );
        // And the divergence sum agrees with what `VolComputer` reports, which
        // is the same theorem taken about the origin instead of face by face.
        assert!(
            relative_error(volume(&mesh).unwrap(), divergence) < 1e-3,
            "{name}: VolComputer {} disagrees with the divergence sum {divergence}",
            volume(&mesh).unwrap()
        );
    }
}

/// The negative control for the winding test: flip one face and the divergence
/// sum must notice.
#[test]
fn one_inverted_face_breaks_the_divergence_test() {
    let mesh = at(Geometry::from(Box3::cube(2.0)), 8, 8);
    let correct = divergence_volume(&mesh);

    let Explicit::QuadSet(mut broken) = mesh else {
        panic!("a box discretises to quads");
    };
    broken.indices[0].reverse();
    let flipped = divergence_volume(&Explicit::QuadSet(broken));

    // Reversing a face negates its contribution, so the total falls by twice
    // that face's share — here a 2×2 face of a 2×2×2 cube centred on the
    // origin, contributing 8/6 of the volume.
    assert!(
        (correct - flipped - 2.0 * 8.0 / 6.0).abs() < 1e-3,
        "correct {correct}, flipped {flipped}"
    );
}

// --- 4. Monotone convergence ------------------------------------------------

/// Every discretisation here is inscribed in the ideal surface, so refining it
/// must approach the closed form from below and never overshoot.
#[test]
fn refinement_approaches_the_closed_form_from_below() {
    let cases: Vec<(&str, Geometry, Real)> = vec![
        ("sphere volume", Geometry::from(Sphere::sized(1.0)), 4.0 * PI / 3.0),
        (
            "cylinder volume",
            Geometry::from(Cylinder::sized(1.0, 2.0)),
            PI * 2.0,
        ),
        ("cone volume", Geometry::from(Cone::sized(1.0, 3.0)), PI),
    ];

    for (name, geometry, closed_form) in cases {
        let mut previous = 0.0;
        for slices in [4u8, 8, 16, 32, 64, 128] {
            let mesh = at(geometry.clone(), slices, slices);
            let measured = volume(&mesh).unwrap();
            assert!(
                measured <= closed_form * (1.0 + 1e-4),
                "{name} at {slices} slices: {measured} overshoots {closed_form}"
            );
            assert!(
                measured >= previous - 1e-5,
                "{name}: refining from the previous step lost volume \
                 ({previous} → {measured} at {slices} slices)"
            );
            previous = measured;
        }
        assert!(
            relative_error(previous, closed_form) < TOLERANCE,
            "{name}: {previous} has not reached {closed_form}"
        );
    }
}

#[test]
fn refinement_approaches_the_closed_form_for_area_too() {
    let closed_form = 4.0 * PI;
    let mut previous = 0.0;
    for slices in [4u8, 8, 16, 32, 64, 128] {
        let mesh = at(Geometry::from(Sphere::sized(1.0)), slices, slices);
        let measured = surface_area(&mesh).unwrap();
        assert!(
            measured <= closed_form * (1.0 + 1e-4),
            "sphere area at {slices} slices: {measured} overshoots {closed_form}"
        );
        assert!(measured >= previous - 1e-5, "{previous} → {measured}");
        previous = measured;
    }
    assert!(relative_error(previous, closed_form) < TOLERANCE);
}

// --- Degenerate input -------------------------------------------------------

#[test]
fn degenerate_primitives_are_rejected_rather_than_sampled() {
    let bad: Vec<Geometry> = vec![
        Geometry::from(Sphere::sized(0.0)),
        Geometry::from(Cone::sized(1.0, 0.0)),
        Geometry::from(Cylinder::sized(-1.0, 1.0)),
        Geometry::from(Frustum::sized(1.0, 1.0, -0.5)),
        Geometry::from(Disc::sized(0.0)),
        Geometry::from(Paraboloid::sized(1.0, 1.0, 0.0)),
        Geometry::from(Box3::new(plantgl::math::Vec3::new(1.0, 0.0, 1.0))),
    ];
    for geometry in bad {
        assert!(
            discretize_with(&geometry, ctx(8, 8)).is_err(),
            "{} should not discretise",
            geometry.type_name()
        );
    }
}

#[test]
fn the_lowest_usable_density_still_produces_a_closed_solid() {
    // Three slices is the floor; the mesh must still be watertight there,
    // because that is what the most distant LOD tier will use.
    for geometry in [
        Geometry::from(Sphere::sized(1.0)),
        Geometry::from(Cone::sized(1.0, 1.0)),
        Geometry::from(Cylinder::sized(1.0, 1.0)),
    ] {
        let name = geometry.type_name();
        let mesh = discretize_with(&geometry, ctx(1, 1)).unwrap();
        assert_edge_manifold(&format!("{name} at the density floor"), &mesh);
        assert!(volume(&mesh).unwrap() > 0.0, "{name} encloses nothing");
    }
}

#[test]
fn every_sampled_point_is_finite() {
    let cases: Vec<Geometry> = vec![
        Geometry::from(Box3::default()),
        Geometry::from(Sphere::sized(1.0)),
        Geometry::from(Cone::sized(1.0, 2.0)),
        Geometry::from(Cylinder::sized(1.0, 2.0)),
        Geometry::from(Frustum::sized(1.0, 2.0, 0.0)),
        Geometry::from(Disc::sized(1.0)),
        Geometry::from(Paraboloid::sized(1.0, 2.0, 0.5)),
    ];
    for geometry in cases {
        let name = geometry.type_name();
        let mesh = at(geometry, 16, 16);
        for (i, point) in mesh.points().iter().enumerate() {
            assert!(
                point.coords.iter().all(|c| c.is_finite()),
                "{name} point {i} is {point:?}"
            );
        }
        assert!(mesh.points().iter().all(|p: &Point3| p.z.is_finite()));
    }
}
