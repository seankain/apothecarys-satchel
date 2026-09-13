//! Face and vertex normals.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/mesh.{h,cpp}`
//! (`computeNormalPerFace`, `computeNormalPerVertex`, `computeNormalList`)
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! The per-face and per-vertex computations live on [`IndexedMesh`] itself,
//! translated in Phase A. What this module adds is the third mode upstream
//! lacks: **crease-angle smoothing**, which averages the faces meeting at a
//! vertex only where they meet gently and splits the vertex where they meet
//! sharply.
//!
//! Upstream offers the two extremes and nothing between. Per-face normals
//! facet a stem into visible rings; per-vertex normals round off the rim of a
//! leaf and the edge of a capped cylinder, where the silhouette should be
//! sharp. A threshold gets both from one mesh, which is what a discretised
//! plant needs — smooth along the sweep, hard at the caps.

use std::sync::Arc;

use crate::error::Result;
use crate::math::{Real, Vec3, EPSILON};
use crate::scenegraph::mesh::{ExplicitModel, FaceIndex, Index3, IndexedMesh, TriangleSet};

use super::discretize::Explicit;

/// `Mesh::computeNormalPerFace()` — one normal per face.
pub fn face_normals<I: FaceIndex>(mesh: &IndexedMesh<I>) -> Result<Vec<Vec3>> {
    mesh.compute_normal_per_face()
}

/// `Mesh::computeNormalPerVertex()` — the area-weighted mean of the adjacent
/// face normals.
///
/// Upstream sums the *unnormalised* cross products, so a large face counts for
/// more than a small one; that weighting is preserved.
pub fn vertex_normals<I: FaceIndex>(mesh: &IndexedMesh<I>) -> Result<Vec<Vec3>> {
    mesh.compute_normal_per_vertex()
}

/// Fills a model's normals in the requested mode, as
/// `Mesh::computeNormalList(pervertex)`.
pub fn compute_normals(model: &mut Explicit, per_vertex: bool) -> Result<()> {
    match model {
        Explicit::TriangleSet(m) => m.compute_normal_list(per_vertex),
        Explicit::QuadSet(m) => m.compute_normal_list(per_vertex),
        Explicit::FaceSet(m) => m.compute_normal_list(per_vertex),
        Explicit::PointSet(_) | Explicit::Polyline(_) => Ok(()),
    }
}

/// Fills a model's normals only if it has none — `Mesh::checkNormalList()`.
pub fn check_normals(model: &mut Explicit) -> Result<()> {
    match model {
        Explicit::TriangleSet(m) => m.check_normal_list(),
        Explicit::QuadSet(m) => m.check_normal_list(),
        Explicit::FaceSet(m) => m.check_normal_list(),
        Explicit::PointSet(_) | Explicit::Polyline(_) => Ok(()),
    }
}

/// The crease angle beyond which two faces are treated as a hard edge.
///
/// 60° keeps a sweep of any reasonable slice count smooth — an 8-slice
/// cylinder turns 45° per facet — while splitting the 90° joins at a cap.
pub const DEFAULT_CREASE_ANGLE: Real = std::f32::consts::FRAC_PI_3;

/// Smooths a triangle set's normals up to a crease angle, splitting vertices
/// where the faces meeting there turn more sharply than that.
///
/// The result is a new [`TriangleSet`] with its own point list: smoothing that
/// preserves hard edges *has* to duplicate the vertices on them, because one
/// position needs two normals. Positions, texture coordinates and colours are
/// duplicated alongside, so the mesh stays valid.
///
/// `crease_angle` is in radians; `0` gives per-face normals and `π` gives the
/// fully smoothed per-vertex ones.
pub fn smooth_normals(mesh: &TriangleSet, crease_angle: Real) -> Result<TriangleSet> {
    let face_normals = mesh.compute_normal_per_face()?;
    let unnormalised: Vec<Vec3> = (0..mesh.face_count())
        .map(|i| {
            let a = corner(mesh, i, 0)?;
            let b = corner(mesh, i, 1)?;
            let c = corner(mesh, i, 2)?;
            let (first, second) = if mesh.model.ccw { (b, c) } else { (c, b) };
            Ok((first - a).cross(&(second - a)))
        })
        .collect::<Result<_>>()?;

    let cos_threshold = crease_angle.cos();

    // Which faces touch each original vertex.
    let mut incident: Vec<Vec<usize>> = vec![Vec::new(); mesh.model.point_count()];
    for i in 0..mesh.face_count() {
        for j in 0..3 {
            if let Some(index) = mesh.face_point_index_at(i, j) {
                if let Some(slot) = incident.get_mut(index as usize) {
                    slot.push(i);
                }
            }
        }
    }

    // Group each vertex's incident faces into smoothing groups: a face joins
    // a group when its normal is within the crease angle of the group's
    // running normal.
    let mut points: Vec<crate::math::Point3> = Vec::with_capacity(mesh.model.point_count());
    let mut normals: Vec<Vec3> = Vec::with_capacity(mesh.model.point_count());
    let mut tex_coords: Vec<crate::math::Vec2> = Vec::new();
    let mut colors: Vec<crate::scenegraph::appearance::Color4> = Vec::new();
    // For each (original vertex, face), the new vertex index to use.
    let mut remap: Vec<Vec<(usize, u32)>> = vec![Vec::new(); mesh.model.point_count()];

    let source_tex = mesh.model.tex_coords.clone();
    let source_colors = mesh.model.colors.clone();

    for (vertex, faces) in incident.iter().enumerate() {
        let mut groups: Vec<(Vec3, Vec<usize>)> = Vec::new();
        for &face in faces {
            let normal = face_normals[face];
            let slot = groups.iter_mut().find(|(accumulated, _)| {
                accumulated
                    .try_normalize(EPSILON)
                    .map(|n| n.dot(&normal) >= cos_threshold)
                    .unwrap_or(true)
            });
            match slot {
                Some((accumulated, members)) => {
                    *accumulated += unnormalised[face];
                    members.push(face);
                }
                None => groups.push((unnormalised[face], vec![face])),
            }
        }

        if groups.is_empty() {
            // A vertex no face references keeps upstream's fallback normal.
            points.push(mesh.model.points[vertex]);
            normals.push(Vec3::new(1.0, 0.0, 0.0));
            if let Some(list) = &source_tex {
                tex_coords.push(list.get(vertex).copied().unwrap_or_default());
            }
            if let Some(list) = &source_colors {
                colors.push(list.get(vertex).copied().unwrap_or_default());
            }
            continue;
        }

        for (accumulated, members) in groups {
            let new_index = points.len() as u32;
            points.push(mesh.model.points[vertex]);
            normals.push(
                accumulated
                    .try_normalize(EPSILON)
                    .unwrap_or(ExplicitModel::DEFAULT_NORMAL_VALUE),
            );
            if let Some(list) = &source_tex {
                tex_coords.push(list.get(vertex).copied().unwrap_or_default());
            }
            if let Some(list) = &source_colors {
                colors.push(list.get(vertex).copied().unwrap_or_default());
            }
            for face in members {
                remap[vertex].push((face, new_index));
            }
        }
    }

    let mut indices: Vec<Index3> = Vec::with_capacity(mesh.face_count());
    for i in 0..mesh.face_count() {
        let mut triangle = [0u32; 3];
        for (j, corner) in triangle.iter_mut().enumerate() {
            let original = mesh
                .face_point_index_at(i, j)
                .ok_or_else(|| {
                    crate::error::Error::invalid_index(format!("face {i} corner {j}"))
                })? as usize;
            *corner = remap[original]
                .iter()
                .find(|(face, _)| *face == i)
                .map(|(_, index)| *index)
                .ok_or_else(|| {
                    crate::error::Error::invalid_index(format!(
                        "vertex {original} is not in any smoothing group of face {i}"
                    ))
                })?;
        }
        indices.push(triangle);
    }

    let mut out = TriangleSet::new(points, indices);
    out.model.ccw = mesh.model.ccw;
    out.model.solid = mesh.model.solid;
    out.model.skeleton = mesh.model.skeleton.clone();
    out.model.normal_per_vertex = true;
    out.model.normals = Some(Arc::new(normals));
    if source_tex.is_some() {
        out.model.tex_coords = Some(Arc::new(tex_coords));
    }
    if source_colors.is_some() {
        out.model.color_per_vertex = true;
        out.model.colors = Some(Arc::new(colors));
    }
    Ok(out)
}

fn corner(mesh: &TriangleSet, i: usize, j: usize) -> Result<crate::math::Point3> {
    mesh.face_point_at(i, j)
        .ok_or_else(|| crate::error::Error::invalid_index(format!("face {i} corner {j}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::discretize::discretize;
    use crate::algo::tessellate::tessellate_flattened;
    use crate::math::Point3;
    use crate::scenegraph::geometry::Geometry;
    use crate::scenegraph::primitive::{Box3, Cylinder};
    use approx::assert_relative_eq;

    /// A flat square, split into two triangles.
    fn flat_square() -> TriangleSet {
        TriangleSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    /// Two unit squares meeting at a right angle along the y axis.
    fn right_angle() -> TriangleSet {
        TriangleSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
                Point3::new(0.0, 1.0, 1.0),
            ],
            // The z = 0 sheet, then the x = 0 sheet, sharing the edge 0-1.
            vec![[0, 2, 3], [0, 3, 1], [0, 1, 5], [0, 5, 4]],
        )
    }

    #[test]
    fn face_and_vertex_normals_match_the_phase_a_computations() {
        let mesh = flat_square();
        assert_eq!(face_normals(&mesh).unwrap().len(), 2);
        assert_eq!(vertex_normals(&mesh).unwrap().len(), 4);
        for n in vertex_normals(&mesh).unwrap() {
            assert_relative_eq!(n, Vec3::z(), epsilon = 1e-6);
        }
    }

    #[test]
    fn check_normals_leaves_an_existing_list_alone() {
        let mut model = discretize(&Geometry::from(Box3::cube(1.0))).unwrap();
        check_normals(&mut model).unwrap();
        assert!(model.model().has_normals());
        let first = model.model().normals.clone().unwrap()[0];
        check_normals(&mut model).unwrap();
        assert_eq!(model.model().normals.as_ref().unwrap()[0], first);
    }

    #[test]
    fn a_flat_surface_smooths_without_duplicating_anything() {
        let smoothed = smooth_normals(&flat_square(), DEFAULT_CREASE_ANGLE).unwrap();
        assert_eq!(smoothed.model.point_count(), 4);
        for n in smoothed.model.normals.as_ref().unwrap().iter() {
            assert_relative_eq!(*n, Vec3::z(), epsilon = 1e-6);
        }
    }

    #[test]
    fn a_hard_edge_splits_its_shared_vertices() {
        let smoothed = smooth_normals(&right_angle(), DEFAULT_CREASE_ANGLE).unwrap();
        // Vertices 0 and 1 are on the 90° crease, so each becomes two.
        assert_eq!(smoothed.model.point_count(), 8);

        // Every normal is one of the two sheet normals, not an average.
        for n in smoothed.model.normals.as_ref().unwrap().iter() {
            let is_sheet_normal = (n.dot(&Vec3::z()).abs() - 1.0).abs() < 1e-4
                || (n.dot(&Vec3::x()).abs() - 1.0).abs() < 1e-4;
            assert!(is_sheet_normal, "{n:?} is an averaged normal");
        }
        assert!(smoothed.is_valid().is_ok());
    }

    #[test]
    fn a_wide_enough_threshold_smooths_the_crease_instead() {
        let smoothed = smooth_normals(&right_angle(), std::f32::consts::PI).unwrap();
        assert_eq!(smoothed.model.point_count(), 6);
        // The shared edge now carries the average of the two sheets.
        let normals = smoothed.model.normals.as_ref().unwrap();
        let shared = normals[0];
        assert_relative_eq!(shared.dot(&Vec3::z()), shared.dot(&Vec3::x()), epsilon = 1e-4);
    }

    #[test]
    fn a_zero_threshold_gives_every_face_its_own_normal() {
        let smoothed = smooth_normals(&flat_square(), 0.0).unwrap();
        // Coplanar faces still merge at zero threshold, because their normals
        // are identical — the threshold splits what actually differs.
        assert_eq!(smoothed.model.point_count(), 4);

        let smoothed = smooth_normals(&right_angle(), 0.0).unwrap();
        assert_eq!(smoothed.model.point_count(), 8);
    }

    #[test]
    fn smoothing_preserves_the_geometry_and_the_face_count() {
        let mesh = tessellate_flattened(
            &discretize(&Geometry::from(Cylinder::new(1.0, 2.0, true, 16))).unwrap(),
        )
        .unwrap();
        let smoothed = smooth_normals(&mesh, DEFAULT_CREASE_ANGLE).unwrap();
        assert_eq!(smoothed.face_count(), mesh.face_count());
        assert!(smoothed.model.point_count() >= mesh.model.point_count());

        // The cylinder's side is smooth and its caps are hard, so smoothing
        // must duplicate the rim vertices but keep the sweep continuous.
        assert!(smoothed.model.point_count() > mesh.model.point_count());
        assert!(smoothed.is_valid().is_ok());
        assert_relative_eq!(
            crate::algo::measure::surface_area(&Explicit::TriangleSet(smoothed)).unwrap(),
            crate::algo::measure::surface_area(&Explicit::TriangleSet(mesh)).unwrap(),
            epsilon = 1e-4
        );
    }

    #[test]
    fn texture_coordinates_follow_the_split_vertices() {
        let mesh = tessellate_flattened(
            &discretize(&Geometry::from(Box3::cube(1.0))).unwrap(),
        )
        .unwrap();
        let smoothed = smooth_normals(&mesh, DEFAULT_CREASE_ANGLE).unwrap();
        assert_eq!(
            smoothed.model.tex_coords.as_ref().unwrap().len(),
            smoothed.model.point_count()
        );
    }

    #[test]
    fn a_vertex_no_face_references_keeps_upstreams_fallback() {
        let mut mesh = flat_square();
        Arc::make_mut(&mut mesh.model.points).push(Point3::new(9.0, 9.0, 9.0));
        let smoothed = smooth_normals(&mesh, DEFAULT_CREASE_ANGLE).unwrap();
        let normals = smoothed.model.normals.as_ref().unwrap();
        assert_eq!(normals[normals.len() - 1], Vec3::new(1.0, 0.0, 0.0));
    }
}
