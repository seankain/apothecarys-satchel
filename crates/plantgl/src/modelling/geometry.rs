//! The geometry each turtle command draws.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/modelling/pglturtledrawer.cpp`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189 — the half of `PglTurtleDrawer`
//! that decides *what shape* a command makes, separated from the half that
//! decides where it goes.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream has one drawer that both builds the geometry and pushes it into a
//! scene. The port splits the two so the scene drawer and the mesh drawer draw
//! the *same* shapes — which is what makes the acceptance criterion "all three
//! drawers report consistent metrics" checkable rather than aspirational.

use crate::error::{Error, Result};
use crate::math::{Frame, Point2, Point3, Real, Vec2, Vec3, EPSILON};
use crate::modelling::drawer::DrawCtx;
use crate::scenegraph::curve::{Curve2DRef, Curve3D};
use crate::scenegraph::geometry::Geometry;
use crate::scenegraph::mesh::{FaceSet, Index3, Polyline, QuadSet, TriangleSet};
use crate::scenegraph::primitive::{
    Box3, Cone, Cylinder, Disc, Extrusion, Frustum, Sphere, MIN_SLICES,
};
use crate::scenegraph::transform::{Transform, Transformed};

/// A section resolution as the count a primitive takes.
///
/// Upstream's `sectionResolution` is a `uint_t` and its primitives store a
/// `uchar_t`; the conversion is implicit there and explicit here, clamped to
/// the minimum a closed ring needs.
pub fn slices(resolution: u32) -> u8 {
    resolution.clamp(MIN_SLICES as u32, u8::MAX as u32) as u8
}

/// `PglTurtleDrawer::transform` — places a shape built in the turtle's local
/// frame into the world.
///
/// Upstream orients by `Oriented(up, -left)` — whose basis columns are `up`,
/// `-left` and `up × -left = heading` — and then translates. Both wrappers are
/// skipped when they would be the identity, which keeps a scene of default
/// shapes free of no-op transformations.
pub fn place(frame: &Frame, geometry: Geometry) -> Geometry {
    let mut geometry = geometry;
    if frame.up != Vec3::x() || frame.left != -Vec3::y() {
        geometry = Geometry::from(Transformed::new(
            Transform::Oriented {
                primary: frame.up,
                secondary: -frame.left,
            },
            geometry.into_ref(),
        ));
    }
    if frame.position != Point3::origin() {
        geometry = Geometry::from(Transformed::new(
            Transform::Translated(frame.position.coords),
            geometry.into_ref(),
        ));
    }
    geometry
}

/// Whether the two lateral scales agree, which is upstream's test for folding
/// a scale into a radius rather than wrapping the shape in a `Scaled`.
fn lateral_scale(scaling: &Vec3) -> Option<Real> {
    (*scaling != Vec3::new(1.0, 1.0, 1.0) && scaling.x == scaling.y).then_some(scaling.x)
}

/// Whether the scale is uniform, upstream's stricter test.
fn uniform_scale(scaling: &Vec3) -> Option<Real> {
    (*scaling != Vec3::new(1.0, 1.0, 1.0) && scaling.x == scaling.y && scaling.y == scaling.z)
        .then_some(scaling.x)
}

/// A two-point polyline along the heading — what upstream draws for a tube of
/// no radius, so a zero-width `F` still leaves a skeleton behind.
fn segment(frame: &Frame, length: Real) -> Geometry {
    Geometry::from(Polyline::new(vec![
        frame.position,
        frame.position + frame.heading * length,
    ]))
}

/// `PglTurtleDrawer::cylinder` — an `F` with a constant width.
pub fn cylinder(ctx: &DrawCtx, length: Real, radius: Real, resolution: u32) -> Option<Geometry> {
    if length.abs() <= EPSILON {
        return None;
    }
    if radius.abs() < EPSILON {
        return Some(segment(ctx.frame, length * ctx.scaling.z));
    }
    let radius = radius * lateral_scale(&ctx.scaling).unwrap_or(1.0);
    Some(place(
        ctx.frame,
        Geometry::from(Cylinder::new(
            radius,
            length * ctx.scaling.z,
            false,
            slices(resolution),
        )),
    ))
}

/// `PglTurtleDrawer::frustum` — an `F` that narrows (or widens) as it goes.
///
/// Three shapes come out of it, exactly as upstream: a `Cone` when the top
/// radius vanishes, a `Cylinder` when the taper is 1, and a `Frustum`
/// otherwise. A base radius of zero is the inverted case — a cone standing on
/// the *far* end, which is how upstream draws a segment that starts at a point.
pub fn frustum(
    ctx: &DrawCtx,
    length: Real,
    base_radius: Real,
    top_radius: Real,
    resolution: u32,
) -> Option<Geometry> {
    if length.abs() <= EPSILON {
        return None;
    }
    let frame = ctx.frame;
    let slices = slices(resolution);

    if base_radius.abs() > EPSILON {
        let taper = top_radius / base_radius;
        let base_radius = base_radius * lateral_scale(&ctx.scaling).unwrap_or(1.0);
        let height = length * ctx.scaling.z;
        let shape = if taper.abs() < EPSILON {
            Geometry::from(Cone::new(base_radius, height, false, slices))
        } else if (taper - 1.0).abs() < EPSILON {
            Geometry::from(Cylinder::new(base_radius, height, false, slices))
        } else {
            Geometry::from(Frustum::new(base_radius, height, taper, false, slices))
        };
        return Some(place(frame, shape));
    }

    if top_radius.abs() < EPSILON {
        return Some(segment(frame, length * ctx.scaling.z));
    }

    // A cone with its point at the turtle and its base at the far end, so it
    // is oriented by `(left, -up)` rather than `(up, -left)` and translated to
    // the top.
    let top_radius = top_radius * uniform_scale(&ctx.scaling).unwrap_or(1.0);
    let height = length * ctx.scaling.z;
    let mut shape = Geometry::from(Cone::new(top_radius, height, false, slices));
    if frame.left != Vec3::x() || frame.up != -Vec3::y() {
        shape = Geometry::from(Transformed::new(
            Transform::Oriented {
                primary: frame.left,
                secondary: -frame.up,
            },
            shape.into_ref(),
        ));
    }
    Some(Geometry::from(Transformed::new(
        Transform::Translated(frame.position.coords + frame.heading * height),
        shape.into_ref(),
    )))
}

/// `PglTurtleDrawer::sphere`.
pub fn sphere(ctx: &DrawCtx, radius: Real, resolution: u32) -> Option<Geometry> {
    if radius <= EPSILON {
        return None;
    }
    let slices = slices(resolution);
    let radius = match uniform_scale(&ctx.scaling) {
        // An anisotropic scale cannot be folded into the radius, so upstream
        // wraps the unit sphere in a `Scaled` instead.
        None if ctx.scaling != Vec3::new(1.0, 1.0, 1.0) => {
            let shape = Geometry::from(Sphere::new(1.0, slices, slices));
            let scaled = Geometry::from(Transformed::new(
                Transform::Scaled(ctx.scaling * radius),
                shape.into_ref(),
            ));
            return Some(place(ctx.frame, scaled));
        }
        scale => radius * scale.unwrap_or(1.0),
    };
    Some(place(
        ctx.frame,
        Geometry::from(Sphere::new(radius, slices, slices)),
    ))
}

/// `PglTurtleDrawer::circle` — a disc facing along the heading.
pub fn circle(ctx: &DrawCtx, radius: Real, resolution: u32) -> Option<Geometry> {
    if radius < EPSILON {
        return None;
    }
    let radius = radius * uniform_scale(&ctx.scaling).unwrap_or(1.0);
    let disc = Geometry::from(Disc::new(radius, slices(resolution)));
    // A `Disc` lies in the XY plane; a quarter turn about Y stands it up
    // across the heading.
    let facing = Geometry::from(Transformed::new(
        Transform::AxisRotated {
            axis: Vec3::y(),
            angle: std::f32::consts::FRAC_PI_2,
        },
        disc.into_ref(),
    ));
    Some(place(ctx.frame, facing))
}

/// `PglTurtleDrawer::box` — a rectangular segment, tapered when its two
/// radii differ.
pub fn box3(ctx: &DrawCtx, length: Real, bottom_radius: Real, top_radius: Real) -> Option<Geometry> {
    if bottom_radius.abs() < EPSILON && top_radius.abs() < EPSILON {
        return Some(segment(ctx.frame, length * ctx.scaling.z));
    }
    let lateral = lateral_scale(&ctx.scaling).unwrap_or(1.0);
    let (bottom_radius, top_radius) = (bottom_radius * lateral, top_radius * lateral);
    let half = length * ctx.scaling.z / 2.0;

    let shape = if (top_radius - bottom_radius).abs() < EPSILON {
        Geometry::from(Box3::new(Vec3::new(bottom_radius, bottom_radius, half)))
    } else {
        Geometry::from(Transformed::new(
            Transform::Tapered {
                base_radius: bottom_radius,
                top_radius,
            },
            Geometry::from(Box3::new(Vec3::new(1.0, 1.0, half))).into_ref(),
        ))
    };
    // The box is centred on its own origin; the turtle stands at its base.
    let lifted = Geometry::from(Transformed::new(
        Transform::Translated(Vec3::new(0.0, 0.0, half)),
        shape.into_ref(),
    ));
    Some(place(ctx.frame, lifted))
}

/// `PglTurtleDrawer::quad` — a flat ribbon in the `heading`/`left` plane.
///
/// A radius of zero at either end collapses that end to a point, so a quad is
/// also how a triangular blade is drawn.
pub fn quad(ctx: &DrawCtx, length: Real, bottom_radius: Real, top_radius: Real) -> Geometry {
    let frame = ctx.frame;
    let lateral = lateral_scale(&ctx.scaling).unwrap_or(1.0);
    let (bottom_radius, top_radius) = (bottom_radius * lateral, top_radius * lateral);

    let mut points = Vec::with_capacity(4);
    if bottom_radius.abs() < EPSILON {
        points.push(frame.position);
    } else {
        points.push(frame.position + frame.left * bottom_radius);
        points.push(frame.position - frame.left * bottom_radius);
    }
    let top = frame.position + frame.heading * length * ctx.scaling.z;
    if top_radius.abs() < EPSILON {
        points.push(top);
    } else {
        points.push(top - frame.left * top_radius);
        points.push(top + frame.left * top_radius);
    }

    match points.len() {
        2 => Geometry::from(Polyline::new(points)),
        3 => Geometry::from(TriangleSet::new(points, vec![[0, 1, 2]])),
        _ => Geometry::from(QuadSet::new(points, vec![[0, 1, 2, 3]])),
    }
}

/// `PglTurtleDrawer::polygon` — the accumulated points as a surface.
///
/// Convex input is fanned from the first point, as upstream does. When
/// `concave_test` is set the points are projected into their own plane and
/// ear-clipped, which is how an L-system leaf with a notched outline gets
/// built without an art asset.
pub fn polygon(points: &[Point3], concave_test: bool) -> Result<Option<Geometry>> {
    if points.len() < 3 {
        return Ok(None);
    }
    // A closed outline repeats its first point; drop the duplicate.
    let points: Vec<Point3> = if (points[points.len() - 1] - points[0]).norm() < EPSILON {
        points[..points.len() - 1].to_vec()
    } else {
        points.to_vec()
    };
    if points.len() < 3 {
        return Ok(None);
    }

    if !concave_test {
        let indices: Vec<Index3> = (0..points.len() - 2)
            .map(|i| [0, i as u32 + 1, i as u32 + 2])
            .collect();
        return Ok(Some(Geometry::from(TriangleSet::new(points, indices))));
    }

    let projected = project_to_plane(&points)?;
    let faces = ear_clip(&projected)?;
    Ok(Some(Geometry::from(FaceSet::new(points, faces))))
}

/// The outline in its own plane, as upstream's concave branch builds it: the
/// first point is the origin, `i` runs towards the second, and `j` completes
/// the frame with the (clockwise) face normal.
fn project_to_plane(points: &[Point3]) -> Result<Vec<Point2>> {
    let (v0, v1, v2) = (points[0], points[1], points[2]);
    let normal = -(v1 - v0).cross(&(v2 - v0));
    let normal = normal
        .try_normalize(EPSILON)
        .ok_or_else(|| Error::degenerate("a polygon whose first three points are collinear"))?;
    let i = (v1 - v0)
        .try_normalize(EPSILON)
        .ok_or_else(|| Error::degenerate("a polygon with a repeated point"))?;
    let j = normal.cross(&i);
    Ok(points
        .iter()
        .map(|p| {
            let d = p - v0;
            Point2::new(d.dot(&i), d.dot(&j))
        })
        .collect())
}

/// Ear clipping over a simple polygon.
///
/// **Original to this port.** Upstream calls its own `polygonization(points,
/// eConvexTriangulation)` from `algo/base/tesselator.h`, which is not in the
/// ported scope; this is the standard O(n²) ear clip, which produces the same
/// kind of triangulation — a fan-free decomposition that stays inside a
/// concave outline.
fn ear_clip(points: &[Point2]) -> Result<Vec<Vec<u32>>> {
    let count = points.len();
    let area: Real = (0..count)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % count]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    if area.abs() <= EPSILON {
        return Err(Error::degenerate("a polygon of zero area"));
    }
    // Work counter-clockwise whichever way the outline was given.
    let mut remaining: Vec<usize> = if area > 0.0 {
        (0..count).collect()
    } else {
        (0..count).rev().collect()
    };

    let cross = |a: Point2, b: Point2, c: Point2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let inside = |a: Point2, b: Point2, c: Point2, p: Point2| {
        cross(a, b, p) >= 0.0 && cross(b, c, p) >= 0.0 && cross(c, a, p) >= 0.0
    };

    let mut faces: Vec<Vec<u32>> = Vec::with_capacity(count - 2);
    let mut guard = 0;
    while remaining.len() > 3 {
        let n = remaining.len();
        let mut clipped = false;
        for k in 0..n {
            let (ia, ib, ic) = (
                remaining[(k + n - 1) % n],
                remaining[k],
                remaining[(k + 1) % n],
            );
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            if cross(a, b, c) <= 0.0 {
                continue; // reflex, not an ear
            }
            if remaining
                .iter()
                .any(|&i| i != ia && i != ib && i != ic && inside(a, b, c, points[i]))
            {
                continue; // another vertex is inside it
            }
            faces.push(vec![ia as u32, ib as u32, ic as u32]);
            remaining.remove(k);
            clipped = true;
            break;
        }
        guard += 1;
        if !clipped || guard > count {
            return Err(Error::degenerate(
                "a polygon that is self-intersecting or not simple cannot be triangulated",
            ));
        }
    }
    faces.push(remaining.iter().map(|&i| i as u32).collect());
    Ok(faces)
}

/// `PglTurtleDrawer::generalizedCylinder` — the accumulated axis swept with
/// the cross-section, which is what `startGC`/`stopGC` produce.
///
/// # The initial normal, expressed as a twist
///
/// Upstream sets `Extrusion::InitialNormal` to the first recorded `left`, so
/// the cross-section's local `x` starts along the turtle's `left` rather than
/// wherever the axis's own frame happened to point. This crate's [`Extrusion`]
/// does not carry that field (see its docs): the same thing is said here as a
/// constant `orientation`, the angle from the axis-derived initial `left` to
/// the turtle's. The first ring lands in the same place either way, and every
/// later ring follows from it through the rotation-minimising frames.
pub fn generalized_cylinder(
    points: &[Point3],
    lefts: &[Vec3],
    radii: &[Real],
    cross_section: Curve2DRef,
    ccw: bool,
    samples: u32,
) -> Result<Option<Geometry>> {
    if points.len() < 2 {
        return Ok(None);
    }
    // Upstream drops a two-point axis whose points coincide rather than
    // building a degenerate sweep.
    if points.len() == 2 && (points[0] - points[1]).norm() < EPSILON {
        return Ok(None);
    }

    let axis = Curve3D::from(Polyline::new(points.to_vec())).into_ref();
    let scale: Vec<Vec2> = radii.iter().map(|r| Vec2::new(*r, *r)).collect();
    let mut extrusion = Extrusion::new(axis.clone(), cross_section)
        .with_scale(scale)
        .with_ccw(ccw);

    if let Some(left) = lefts.first() {
        if let Some(twist) = initial_twist(&axis, left, samples) {
            extrusion = extrusion.with_orientation(vec![twist]);
        }
    }
    Ok(Some(Geometry::from(extrusion)))
}

/// The twist that makes the first ring's cross-section `x` axis point along
/// `left`.
///
/// A point `(1, 0)` of the cross-section lands at
/// `cos(t)·left₀ − sin(t)·up₀` in the sweep's own initial frame, so the angle
/// wanted is `atan2(−left·up₀, left·left₀)`.
fn initial_twist(axis: &Curve3D, left: &Vec3, samples: u32) -> Option<Real> {
    let _ = samples;
    let frame = axis.initial_frame().ok()?;
    let left = left.try_normalize(EPSILON)?;
    let twist = (-left.dot(&frame.up)).atan2(left.dot(&frame.left));
    twist.is_finite().then_some(twist)
}

/// `PglTurtleDrawer::smallSweep` — one `F` drawn as a sweep rather than a
/// cylinder, which is what happens once a caller has set a cross-section of
/// its own.
pub fn small_sweep(
    ctx: &DrawCtx,
    length: Real,
    bottom_radius: Real,
    top_radius: Real,
    cross_section: Curve2DRef,
    ccw: bool,
    samples: u32,
) -> Result<Option<Geometry>> {
    let frame = ctx.frame;
    let points = [
        frame.position,
        frame.position + frame.heading * length * ctx.scaling.z,
    ];
    let lefts = [frame.left, frame.left];
    let radii = [bottom_radius, top_radius];
    generalized_cylinder(&points, &lefts, &radii, cross_section, ccw, samples)
}

/// `PglTurtle::customGeometry` — a named surface, scaled and placed.
pub fn custom_geometry(ctx: &DrawCtx, geometry: &Geometry, scale: Real) -> Option<Geometry> {
    let scaling = ctx.scaling * scale;
    if scaling.norm().abs() <= EPSILON {
        return None;
    }
    let scaled = Geometry::from(Transformed::new(
        Transform::Scaled(scaling),
        geometry.clone().into_ref(),
    ));
    Some(place(ctx.frame, scaled))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::discretize::discretize;
    use crate::modelling::drawer::IdPair;
    use crate::scenegraph::curve::{Curve2D, Polyline2D};

    fn ctx(frame: &Frame) -> DrawCtx<'_> {
        DrawCtx {
            ids: IdPair::none(),
            appearance: None,
            frame,
            scaling: Vec3::new(1.0, 1.0, 1.0),
        }
    }

    #[test]
    fn a_cylinder_is_placed_at_the_turtle() {
        let frame = Frame::new(
            Point3::new(0.0, 0.0, 1.0),
            Vec3::z(),
            -Vec3::y(),
            Vec3::x(),
        );
        let geometry = cylinder(&ctx(&frame), 1.0, 0.1, 8).unwrap();
        // The default orientation needs no `Oriented`, only the translation.
        assert_eq!(geometry.type_name(), "Transformed");
        let mesh = discretize(&geometry).unwrap();
        let bbox = crate::algo::bounding_box(&geometry.clone().into_ref())
            .unwrap()
            .unwrap();
        assert!((bbox.lower_left.z - 1.0).abs() < 1e-5, "{bbox:?}");
        assert!((bbox.upper_right.z - 2.0).abs() < 1e-5, "{bbox:?}");
        assert!(!mesh.points().is_empty());
    }

    #[test]
    fn a_zero_radius_tube_is_a_polyline() {
        let frame = Frame::default();
        let geometry = cylinder(&ctx(&frame), 1.0, 0.0, 8).unwrap();
        assert_eq!(geometry.type_name(), "Polyline");
    }

    #[test]
    fn a_frustum_picks_the_shape_its_taper_implies() {
        let frame = Frame::default();
        let ctx = ctx(&frame);
        let cone = frustum(&ctx, 1.0, 0.5, 0.0, 8).unwrap();
        let tube = frustum(&ctx, 1.0, 0.5, 0.5, 8).unwrap();
        let taper = frustum(&ctx, 1.0, 0.5, 0.25, 8).unwrap();
        assert_eq!(cone.type_name(), "Cone");
        assert_eq!(tube.type_name(), "Cylinder");
        assert_eq!(taper.type_name(), "Frustum");
    }

    #[test]
    fn a_quad_collapses_an_end_of_zero_radius() {
        let frame = Frame::default();
        let ctx = ctx(&frame);
        assert_eq!(quad(&ctx, 1.0, 0.5, 0.5).type_name(), "QuadSet");
        assert_eq!(quad(&ctx, 1.0, 0.5, 0.0).type_name(), "TriangleSet");
        assert_eq!(quad(&ctx, 1.0, 0.0, 0.0).type_name(), "Polyline");
    }

    /// Acceptance (T8.9): five coplanar points give three triangles with one
    /// consistent normal.
    #[test]
    fn a_convex_polygon_fans_into_triangles() {
        let points: Vec<Point3> = (0..5)
            .map(|i| {
                let a = std::f32::consts::TAU * i as Real / 5.0;
                Point3::new(a.cos(), a.sin(), 0.0)
            })
            .collect();
        let geometry = polygon(&points, false).unwrap().unwrap();
        let Geometry::TriangleSet(mesh) = &geometry else {
            panic!("expected a TriangleSet, got {}", geometry.type_name());
        };
        assert_eq!(mesh.face_count(), 3);
        let first = mesh.face_normal(0).unwrap();
        for face in 1..mesh.face_count() {
            let normal = mesh.face_normal(face).unwrap();
            assert!((normal - first).norm() < 1e-5, "normal {face} disagrees");
        }
    }

    #[test]
    fn a_concave_polygon_is_ear_clipped() {
        // An arrowhead: the fourth point pushes into the outline.
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(2.0, 2.0, 0.0),
            Point3::new(1.0, 0.5, 0.0),
            Point3::new(0.0, 2.0, 0.0),
        ];
        let geometry = polygon(&points, true).unwrap().unwrap();
        let Geometry::FaceSet(mesh) = &geometry else {
            panic!("expected a FaceSet, got {}", geometry.type_name());
        };
        assert_eq!(mesh.face_count(), 3);

        // Ear clipping must not invent area: the triangles sum to the
        // outline's shoelace area, 2.5 for this arrowhead.
        let mesh_area = crate::algo::measure::surface_area(&discretize(&geometry).unwrap()).unwrap();
        assert!((mesh_area - 2.5).abs() < 1e-4, "{mesh_area}");
    }

    #[test]
    fn a_polygon_of_fewer_than_three_points_draws_nothing() {
        assert!(polygon(&[Point3::origin(), Point3::new(1.0, 0.0, 0.0)], false)
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_generalized_cylinder_locks_the_section_to_the_turtles_left() {
        let section = Curve2D::from(Polyline2D::circle(1.0, 8)).into_ref();
        let points = vec![Point3::origin(), Point3::new(0.0, 0.0, 1.0)];
        let lefts = vec![-Vec3::y(), -Vec3::y()];
        let radii = vec![0.5, 0.25];
        let geometry = generalized_cylinder(&points, &lefts, &radii, section, true, 8)
            .unwrap()
            .unwrap();
        let Geometry::Extrusion(extrusion) = &geometry else {
            panic!("expected an Extrusion");
        };
        assert_eq!(extrusion.scale.len(), 2);
        assert_eq!(extrusion.orientation.len(), 1);

        let mesh = discretize(&geometry).unwrap();
        let bbox = crate::algo::bounding_box(&geometry.clone().into_ref())
            .unwrap()
            .unwrap();
        assert!((bbox.upper_right.z - 1.0).abs() < 1e-5);
        assert!(!mesh.points().is_empty());
    }

    #[test]
    fn a_collapsed_axis_draws_nothing() {
        let section = Curve2D::from(Polyline2D::circle(1.0, 8)).into_ref();
        let points = vec![Point3::origin(), Point3::origin()];
        assert!(
            generalized_cylinder(&points, &[Vec3::x(), Vec3::x()], &[1.0, 1.0], section, true, 8)
                .unwrap()
                .is_none()
        );
    }
}
