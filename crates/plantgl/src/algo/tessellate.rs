//! Explicit meshes to triangles.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/base/tesselator.{h,cpp}` and
//! `src/cpp/plantgl/scenegraph/container/indexarray.cpp`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # Concave faces: where this diverges, and why
//!
//! Upstream's `Tesselator` delegates to `IndexArray::triangulate()`, which is a
//! fan from corner 0 and nothing else. That is correct for a convex face and
//! wrong for a concave one: the fan's triangles spill outside the polygon and
//! overlap each other, so the mesh renders with visible slivers and measures
//! with too much area.
//!
//! Every face this crate's own discretizer emits is convex, so the fan is
//! enough for them — and the fast path below takes it, producing byte-identical
//! output to upstream. A face reaching here from a hand-built `FaceSet`, an
//! imported mesh or a future turtle cross-section may not be, so a concavity
//! test gates an ear-clipping fallback. Detecting convexity costs one pass over
//! the corners and is skipped entirely for triangles and quads.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point3, Real, Vec3, EPSILON};
use crate::scenegraph::geometry::Geometry;
use crate::scenegraph::mesh::{FaceIndex, Index3, IndexedMesh, TriangleSet};

use super::discretize::{Discretizer, Explicit};

/// Upstream's `Tesselator` — any explicit mesh reduced to triangles.
///
/// Point sets and polylines have no faces and are rejected rather than
/// returned empty, because a caller asking for triangles has a bug if it
/// handed over a polyline.
pub fn tessellate(model: &Explicit) -> Result<TriangleSet> {
    match model {
        Explicit::TriangleSet(m) => Ok(m.clone()),
        Explicit::QuadSet(m) => tessellate_mesh(m),
        Explicit::FaceSet(m) => tessellate_mesh(m),
        Explicit::PointSet(_) | Explicit::Polyline(_) => Err(Error::unsupported(
            "a point set or polyline has no faces to tessellate",
        )),
    }
}

/// Discretises a geometry tree and tessellates the result.
pub fn tessellate_geometry(
    geometry: &Geometry,
    discretizer: &mut Discretizer,
) -> Result<TriangleSet> {
    tessellate(&discretizer.discretize(geometry)?)
}

fn tessellate_mesh<I: FaceIndex>(mesh: &IndexedMesh<I>) -> Result<TriangleSet> {
    let mut indices: Vec<Index3> = Vec::with_capacity(mesh.face_count() * 2);
    let mut normal_indices: Vec<Index3> = Vec::new();
    let mut color_indices: Vec<Index3> = Vec::new();
    let mut tex_coord_indices: Vec<Index3> = Vec::new();

    let has_normals = mesh.normal_indices.is_some();
    let has_colors = mesh.color_indices.is_some();
    let has_tex = mesh.tex_coord_indices.is_some();

    for i in 0..mesh.face_count() {
        let size = mesh
            .face_size(i)
            .ok_or_else(|| Error::invalid_index(format!("face {i}")))?;
        if size < 3 {
            return Err(Error::degenerate(format!(
                "face {i} has {size} corners, need at least 3"
            )));
        }

        let corners: Vec<Point3> = (0..size)
            .map(|j| {
                mesh.face_point_at(i, j)
                    .ok_or_else(|| Error::invalid_index(format!("face {i} corner {j}")))
            })
            .collect::<Result<_>>()?;

        // The triangulation is computed once, as *corner ordinals*, and then
        // applied to every index list — so normals, colours and texture
        // coordinates are cut the same way the positions are.
        let fan = triangulate_corners(&corners);

        for [a, b, c] in &fan {
            let side = |list: &dyn Fn(usize, usize) -> Option<u32>| -> Result<Index3> {
                Ok([
                    list(i, *a).ok_or_else(|| Error::invalid_index(format!("face {i}")))?,
                    list(i, *b).ok_or_else(|| Error::invalid_index(format!("face {i}")))?,
                    list(i, *c).ok_or_else(|| Error::invalid_index(format!("face {i}")))?,
                ])
            };
            indices.push(side(&|f, j| mesh.face_point_index_at(f, j))?);
            if has_normals {
                normal_indices.push(side(&|f, j| mesh.face_normal_index_at(f, j))?);
            }
            if has_colors {
                color_indices.push(side(&|f, j| mesh.face_color_index_at(f, j))?);
            }
            if has_tex {
                tex_coord_indices.push(side(&|f, j| mesh.face_tex_coord_index_at(f, j))?);
            }
        }
    }

    let mut triangles = TriangleSet::new(Vec::new(), indices);
    triangles.model = mesh.model.clone();
    // Per-face side lists no longer line up: one source face has become
    // several triangles, so the face ordinal is not the list index any more.
    // Writing the indices out explicitly says the same thing and survives it.
    if has_normals {
        triangles.model.normal_per_vertex = true;
        triangles.normal_indices = Some(normal_indices);
    } else if !mesh.model.normal_per_vertex && mesh.model.normals.is_some() {
        // Per-face normals with no explicit list: re-expand them so each
        // triangle keeps the normal of the face it came from.
        let mut expanded = Vec::with_capacity(triangles.face_count());
        let mut cursor = 0;
        for i in 0..mesh.face_count() {
            let fan_size = mesh.face_size(i).unwrap_or(3).saturating_sub(2);
            for _ in 0..fan_size {
                expanded.push([i as u32; 3]);
                cursor += 1;
            }
        }
        debug_assert_eq!(cursor, triangles.face_count());
        triangles.model.normal_per_vertex = true;
        triangles.normal_indices = Some(expanded);
    }
    if has_colors {
        triangles.model.color_per_vertex = true;
        triangles.color_indices = Some(color_indices);
    }
    if has_tex {
        triangles.tex_coord_indices = Some(tex_coord_indices);
    }
    Ok(triangles)
}

/// Triangulates one face, returning triangles as **corner ordinals** into the
/// face rather than as vertex indices.
fn triangulate_corners(corners: &[Point3]) -> Vec<[usize; 3]> {
    let n = corners.len();
    if n < 3 {
        return Vec::new();
    }
    if n == 3 {
        return vec![[0, 1, 2]];
    }
    if is_convex(corners) {
        return fan(n);
    }
    ear_clip(corners).unwrap_or_else(|| fan(n))
}

/// Upstream's `triangulate()` — a fan from corner 0.
fn fan(n: usize) -> Vec<[usize; 3]> {
    (1..n - 1).map(|j| [0, j, j + 1]).collect()
}

/// The face's normal, from the Newell sum so a slightly non-planar face still
/// gets a sensible plane to work in.
fn newell_normal(corners: &[Point3]) -> Vec3 {
    let mut normal = Vec3::zeros();
    for i in 0..corners.len() {
        let a = corners[i];
        let b = corners[(i + 1) % corners.len()];
        normal.x += (a.y - b.y) * (a.z + b.z);
        normal.y += (a.z - b.z) * (a.x + b.x);
        normal.z += (a.x - b.x) * (a.y + b.y);
    }
    normal
}

/// Whether every turn goes the same way — the fast path's precondition.
fn is_convex(corners: &[Point3]) -> bool {
    let Some(normal) = newell_normal(corners).try_normalize(EPSILON) else {
        // A face with no plane is degenerate; the fan is as good as anything.
        return true;
    };
    let n = corners.len();
    let mut sign = 0.0;
    for i in 0..n {
        let a = corners[i];
        let b = corners[(i + 1) % n];
        let c = corners[(i + 2) % n];
        let turn = (b - a).cross(&(c - b)).dot(&normal);
        if turn.abs() <= EPSILON {
            continue; // Collinear corner, no information either way.
        }
        if sign == 0.0 {
            sign = turn.signum();
        } else if turn.signum() != sign {
            return false;
        }
    }
    true
}

/// Ear clipping in the face's own plane, for concave faces.
///
/// Returns `None` when no ear can be found — a self-intersecting face, say —
/// so the caller can fall back rather than loop.
fn ear_clip(corners: &[Point3]) -> Option<Vec<[usize; 3]>> {
    let normal = newell_normal(corners).try_normalize(EPSILON)?;
    // Project into the plane so the whole thing is a 2D problem with a
    // consistent orientation.
    let u = pick_tangent(&normal);
    let v = normal.cross(&u);
    let flat: Vec<(Real, Real)> = corners
        .iter()
        .map(|p| (p.coords.dot(&u), p.coords.dot(&v)))
        .collect();

    let n = flat.len();
    let mut remaining: Vec<usize> = (0..n).collect();
    let mut triangles = Vec::with_capacity(n - 2);

    // Each pass must remove at least one corner, so `n` passes is a hard
    // bound and a self-intersecting polygon terminates rather than spins.
    let mut guard = n * n;
    while remaining.len() > 3 {
        let mut clipped = false;
        for k in 0..remaining.len() {
            let prev = remaining[(k + remaining.len() - 1) % remaining.len()];
            let cur = remaining[k];
            let next = remaining[(k + 1) % remaining.len()];

            if !is_ear(&flat, &remaining, prev, cur, next) {
                continue;
            }
            triangles.push([prev, cur, next]);
            remaining.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            return None;
        }
        guard = guard.checked_sub(1)?;
    }
    triangles.push([remaining[0], remaining[1], remaining[2]]);
    Some(triangles)
}

/// Any unit vector perpendicular to `normal`.
fn pick_tangent(normal: &Vec3) -> Vec3 {
    let axis = if normal.x.abs() < 0.9 {
        Vec3::x()
    } else {
        Vec3::y()
    };
    normal
        .cross(&axis)
        .try_normalize(EPSILON)
        .unwrap_or_else(Vec3::x)
}

/// Twice the signed area of a 2D triangle; positive when wound the same way as
/// the projected polygon.
#[inline]
fn cross2((ax, ay): (Real, Real), (bx, by): (Real, Real), (cx, cy): (Real, Real)) -> Real {
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

/// Whether corner `cur` is an ear: convex, and with no other remaining corner
/// inside the triangle it would cut off.
fn is_ear(
    flat: &[(Real, Real)],
    remaining: &[usize],
    prev: usize,
    cur: usize,
    next: usize,
) -> bool {
    let (a, b, c) = (flat[prev], flat[cur], flat[next]);
    let area = cross2(a, b, c);
    // The projection is wound counter-clockwise by construction, so a convex
    // corner turns positively. A collinear one is not an ear: clipping it
    // would emit a zero-area triangle.
    if area <= EPSILON {
        return false;
    }
    for &other in remaining {
        if other == prev || other == cur || other == next {
            continue;
        }
        let p = flat[other];
        if cross2(a, b, p) >= 0.0 && cross2(b, c, p) >= 0.0 && cross2(c, a, p) >= 0.0 {
            return false;
        }
    }
    true
}

/// Tessellates and then rewrites the positions so the triangle set owns them —
/// used where a mesh is handed to a renderer that cannot follow index lists.
pub fn tessellate_flattened(model: &Explicit) -> Result<TriangleSet> {
    let mut triangles = tessellate(model)?;
    triangles.model.points = Arc::new(model.points().to_vec());
    Ok(triangles)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::discretize::discretize;
    use crate::algo::measure::surface_area;
    use crate::scenegraph::mesh::{FaceSet, PointSet, QuadSet};
    use crate::scenegraph::primitive::{Box3, Cylinder};
    use approx::assert_relative_eq;

    #[test]
    fn a_quad_becomes_two_triangles_by_a_fan() {
        let quads = QuadSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2, 3]],
        );
        let triangles = tessellate(&Explicit::QuadSet(quads)).unwrap();
        assert_eq!(triangles.indices, vec![[0, 1, 2], [0, 2, 3]]);
    }

    #[test]
    fn a_triangle_set_passes_straight_through() {
        let mesh = discretize(&Geometry::from(Cylinder::default().with_solid(false))).unwrap();
        let quads = mesh.face_count();
        let triangles = tessellate(&mesh).unwrap();
        assert_eq!(triangles.face_count(), quads * 2);
        assert_eq!(tessellate(&Explicit::TriangleSet(triangles.clone())).unwrap(), triangles);
    }

    #[test]
    fn tessellation_preserves_total_area() {
        let mesh = discretize(&Geometry::from(Box3::cube(2.0))).unwrap();
        let before = surface_area(&mesh).unwrap();
        let after = surface_area(&Explicit::TriangleSet(tessellate(&mesh).unwrap())).unwrap();
        assert_relative_eq!(before, after, epsilon = 1e-5);
    }

    #[test]
    fn texture_indices_are_cut_the_same_way_as_positions() {
        let mesh = discretize(&Geometry::from(Box3::cube(1.0))).unwrap();
        let triangles = tessellate(&mesh).unwrap();
        let tex = triangles.tex_coord_indices.as_ref().unwrap();
        assert_eq!(tex.len(), triangles.face_count());
        // The box gives every face the same unit square, fanned the same way.
        assert_eq!(tex[0], [0, 1, 2]);
        assert_eq!(tex[1], [0, 2, 3]);
        assert!(triangles.is_valid().is_ok());
    }

    #[test]
    fn per_face_normals_survive_the_fan() {
        let mut mesh = discretize(&Geometry::from(Box3::cube(1.0))).unwrap();
        let Explicit::QuadSet(quads) = &mut mesh else {
            panic!("a box discretises to quads");
        };
        quads.compute_normal_list(false).unwrap();
        let normals = quads.model.normals.clone().unwrap();

        let triangles = tessellate(&mesh).unwrap();
        assert_eq!(triangles.face_count(), 12);
        // Both triangles of face 3 keep face 3's normal.
        let indices = triangles.normal_indices.as_ref().unwrap();
        assert_eq!(indices[6], [3, 3, 3]);
        assert_eq!(indices[7], [3, 3, 3]);
        assert_relative_eq!(
            triangles.model.normals.as_ref().unwrap()[3],
            normals[3],
            epsilon = 1e-6
        );
    }

    /// Acceptance (T8.5): ear-clipping handles a 20-vertex concave star
    /// without self-intersection.
    #[test]
    fn a_twenty_vertex_concave_star_clips_without_self_intersection() {
        let star = star_polygon(10, 1.0, 0.4);
        assert_eq!(star.len(), 20);
        assert!(!is_convex(&star), "the fixture must actually be concave");

        let triangles = triangulate_corners(&star);
        assert_eq!(triangles.len(), 18, "n - 2 triangles for an n-gon");

        // Every corner is used, and no triangle is degenerate.
        let mut used = vec![false; star.len()];
        let mut total = 0.0;
        for [a, b, c] in &triangles {
            used[*a] = true;
            used[*b] = true;
            used[*c] = true;
            let area = crate::algo::measure::triangle_area(star[*a], star[*b], star[*c]);
            assert!(area > 1e-6, "degenerate triangle {a} {b} {c}");
            total += area;
        }
        assert!(used.iter().all(|u| *u), "every corner must be used");

        // The one test that actually catches self-intersection: the pieces
        // must sum to the polygon's own area. A fan over this star overshoots
        // by more than 60%, because its triangles overlap and spill outside.
        let expected = polygon_area(&star);
        assert_relative_eq!(total, expected, max_relative = 1e-4);

        let fanned: f32 = fan(star.len())
            .iter()
            .map(|[a, b, c]| crate::algo::measure::triangle_area(star[*a], star[*b], star[*c]))
            .sum();
        assert!(
            fanned > expected * 1.5,
            "the fan should visibly overshoot here: {fanned} vs {expected}"
        );
    }

    #[test]
    fn a_concave_face_in_a_face_set_is_ear_clipped() {
        let star = star_polygon(5, 1.0, 0.4);
        let face: Vec<u32> = (0..star.len() as u32).collect();
        let mesh = FaceSet::new(star.clone(), vec![face]);

        let triangles = tessellate(&Explicit::FaceSet(mesh)).unwrap();
        assert_eq!(triangles.face_count(), star.len() - 2);

        let mut flat = triangles.clone();
        flat.model.points = Arc::new(star.clone());
        assert_relative_eq!(
            surface_area(&Explicit::TriangleSet(flat)).unwrap(),
            polygon_area(&star),
            max_relative = 1e-4
        );
    }

    #[test]
    fn convex_faces_take_the_fan_fast_path() {
        // A convex hexagon must come out as the plain fan, so the port stays
        // byte-identical to upstream where upstream is right.
        let hexagon: Vec<Point3> = (0..6)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 6.0;
                Point3::new(a.cos(), a.sin(), 0.0)
            })
            .collect();
        assert!(is_convex(&hexagon));
        assert_eq!(triangulate_corners(&hexagon), fan(6));
    }

    #[test]
    fn a_face_with_no_plane_falls_back_to_the_fan() {
        let collinear = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
            Point3::new(3.0, 0.0, 0.0),
        ];
        assert_eq!(triangulate_corners(&collinear), fan(4));
    }

    #[test]
    fn an_undersized_face_is_rejected() {
        let mesh = FaceSet::new(
            vec![
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1]],
        );
        assert!(matches!(
            tessellate(&Explicit::FaceSet(mesh)),
            Err(Error::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn a_point_set_has_nothing_to_tessellate() {
        let points = Explicit::PointSet(PointSet::new(vec![Point3::origin()]));
        assert!(matches!(tessellate(&points), Err(Error::Unsupported(_))));
    }

    #[test]
    fn flattening_copies_the_positions_onto_the_triangle_set() {
        let mesh = discretize(&Geometry::from(Box3::cube(1.0))).unwrap();
        let triangles = tessellate_flattened(&mesh).unwrap();
        assert_eq!(triangles.model.points.len(), 8);
        assert!(triangles.is_valid().is_ok());
    }

    /// A `points`-pointed star: alternating outer and inner radii, so every
    /// second corner is reflex.
    fn star_polygon(points: usize, outer: f32, inner: f32) -> Vec<Point3> {
        (0..points * 2)
            .map(|i| {
                let angle = std::f32::consts::TAU * i as f32 / (points * 2) as f32;
                let r = if i % 2 == 0 { outer } else { inner };
                Point3::new(r * angle.cos(), r * angle.sin(), 0.0)
            })
            .collect()
    }

    /// The shoelace area of a polygon in the z = 0 plane.
    fn polygon_area(corners: &[Point3]) -> f32 {
        let mut sum = 0.0;
        for i in 0..corners.len() {
            let a = corners[i];
            let b = corners[(i + 1) % corners.len()];
            sum += a.x * b.y - b.x * a.y;
        }
        (sum / 2.0).abs()
    }
}
