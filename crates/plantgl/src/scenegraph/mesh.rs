//! Explicit (vertex-list) geometry: indexed meshes, point sets, polylines and
//! groups.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/{explicitmodel,
//! mesh,triangleset,quadset,faceset,pointset,polyline,group}.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream splits this across an `ExplicitModel` → `Mesh` → `IndexedMesh<I>`
//! inheritance chain whose intermediate levels exist only to share fields.
//! The port keeps one data struct, [`ExplicitModel`], and composes it into
//! [`IndexedMesh`]; the accessor semantics — in particular the index fallbacks
//! in [`IndexedMesh::face_normal_index_at`] and its siblings — are preserved
//! exactly, because discretisation and export depend on them.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point3, Real, Vec2, Vec3, EPSILON};
use crate::scenegraph::appearance::Color4;
use crate::scenegraph::geometry::GeometryRef;

/// The vertex payload shared by every explicit model.
///
/// Merges upstream's `ExplicitModel` (points, colours) with the extra fields
/// `Mesh` adds (normals, texture coordinates, the four flags, the skeleton).
/// Point arrays are `Arc`ed so a discretised primitive can be shared between
/// shapes without copying.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExplicitModel {
    pub points: Arc<Vec<Point3>>,
    pub normals: Option<Arc<Vec<Vec3>>>,
    pub tex_coords: Option<Arc<Vec<Vec2>>>,
    pub colors: Option<Arc<Vec<Color4>>>,
    /// `NormalPerVertex`: whether `normals` is indexed per vertex of a face
    /// or per face. Upstream default `true`.
    pub normal_per_vertex: bool,
    /// `ColorPerVertex`. Upstream default `true`.
    pub color_per_vertex: bool,
    /// `CCW`: face winding. Upstream default `true`.
    pub ccw: bool,
    /// `Solid`: whether the mesh bounds a closed volume. Upstream default
    /// `false`.
    pub solid: bool,
    /// The `Skeleton` field — the axis a mesh was swept along, kept for
    /// downstream measurement. Upstream default null.
    pub skeleton: Option<Arc<Polyline>>,
}

impl ExplicitModel {
    pub const DEFAULT_CCW: bool = true;
    pub const DEFAULT_SOLID: bool = false;
    pub const DEFAULT_NORMAL_PER_VERTEX: bool = true;
    pub const DEFAULT_COLOR_PER_VERTEX: bool = true;
    /// `Mesh::DEFAULT_NORMAL_VALUE` — substituted wherever a computed normal
    /// comes out degenerate.
    pub const DEFAULT_NORMAL_VALUE: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    pub fn new(points: Vec<Point3>) -> Self {
        Self {
            points: Arc::new(points),
            normals: None,
            tex_coords: None,
            colors: None,
            normal_per_vertex: Self::DEFAULT_NORMAL_PER_VERTEX,
            color_per_vertex: Self::DEFAULT_COLOR_PER_VERTEX,
            ccw: Self::DEFAULT_CCW,
            solid: Self::DEFAULT_SOLID,
            skeleton: None,
        }
    }

    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    pub fn has_normals(&self) -> bool {
        self.normals.is_some()
    }

    pub fn has_tex_coords(&self) -> bool {
        self.tex_coords.is_some()
    }
}

/// One face's vertex indices. Implemented for fixed-arity triangles and
/// quads and for the variable-arity faces of a `FaceSet`.
pub trait FaceIndex: Clone + std::fmt::Debug + PartialEq {
    /// Number of corners in the face.
    fn len(&self) -> usize;

    /// The `j`-th corner, or `None` when `j` is out of range.
    fn get(&self, j: usize) -> Option<u32>;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn iter(&self) -> FaceIndexIter<'_, Self> {
        FaceIndexIter {
            index: self,
            next: 0,
        }
    }
}

/// Iterator over the corners of a [`FaceIndex`].
pub struct FaceIndexIter<'a, I: FaceIndex> {
    index: &'a I,
    next: usize,
}

impl<I: FaceIndex> Iterator for FaceIndexIter<'_, I> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        let value = self.index.get(self.next)?;
        self.next += 1;
        Some(value)
    }
}

/// A triangle's three corners — upstream's `Index3`.
pub type Index3 = [u32; 3];
/// A quad's four corners — upstream's `Index4`.
pub type Index4 = [u32; 4];
/// A variable-arity face — upstream's `Index`.
pub type Index = Vec<u32>;

impl FaceIndex for Index3 {
    fn len(&self) -> usize {
        3
    }
    fn get(&self, j: usize) -> Option<u32> {
        self.as_slice().get(j).copied()
    }
}

impl FaceIndex for Index4 {
    fn len(&self) -> usize {
        4
    }
    fn get(&self, j: usize) -> Option<u32> {
        self.as_slice().get(j).copied()
    }
}

impl FaceIndex for Index {
    fn len(&self) -> usize {
        self.as_slice().len()
    }
    fn get(&self, j: usize) -> Option<u32> {
        self.as_slice().get(j).copied()
    }
}

/// An indexed mesh — upstream's `IndexedMesh<IndexArrayType>`.
///
/// The optional `*_indices` lists let normals, colours and texture
/// coordinates be indexed independently of positions; when absent the
/// position indices are reused, exactly as upstream.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(serialize = "I: serde::Serialize")))]
#[cfg_attr(
    feature = "serde",
    serde(bound(deserialize = "I: serde::de::DeserializeOwned"))
)]
pub struct IndexedMesh<I> {
    pub model: ExplicitModel,
    pub indices: Vec<I>,
    pub normal_indices: Option<Vec<I>>,
    pub color_indices: Option<Vec<I>>,
    pub tex_coord_indices: Option<Vec<I>>,
}

/// Upstream's `TriangleSet`.
pub type TriangleSet = IndexedMesh<Index3>;
/// Upstream's `QuadSet`.
pub type QuadSet = IndexedMesh<Index4>;
/// Upstream's `FaceSet`.
pub type FaceSet = IndexedMesh<Index>;

impl<I: FaceIndex> IndexedMesh<I> {
    pub fn new(points: Vec<Point3>, indices: Vec<I>) -> Self {
        Self {
            model: ExplicitModel::new(points),
            indices,
            normal_indices: None,
            color_indices: None,
            tex_coord_indices: None,
        }
    }

    /// `getIndexListSize()` — the number of faces.
    pub fn face_count(&self) -> usize {
        self.indices.len()
    }

    /// `getFaceSize(i)` — the number of corners of face `i`.
    pub fn face_size(&self, i: usize) -> Option<usize> {
        self.indices.get(i).map(FaceIndex::len)
    }

    /// `getFacePointIndexAt(i, j)`.
    pub fn face_point_index_at(&self, i: usize, j: usize) -> Option<u32> {
        self.indices.get(i)?.get(j)
    }

    /// `getFaceNormalIndexAt(i, j)`.
    ///
    /// With per-face normals the face's own ordinal indexes the normal list;
    /// otherwise the dedicated normal index list is used when present, and
    /// the position index otherwise.
    pub fn face_normal_index_at(&self, i: usize, j: usize) -> Option<u32> {
        if i >= self.indices.len() {
            return None;
        }
        if !self.model.normal_per_vertex {
            return Some(i as u32);
        }
        match &self.normal_indices {
            Some(list) => list.get(i)?.get(j),
            None => self.face_point_index_at(i, j),
        }
    }

    /// `getFaceColorIndexAt(i, j)`, with the same fallback as normals.
    pub fn face_color_index_at(&self, i: usize, j: usize) -> Option<u32> {
        if i >= self.indices.len() {
            return None;
        }
        if !self.model.color_per_vertex {
            return Some(i as u32);
        }
        match &self.color_indices {
            Some(list) => list.get(i)?.get(j),
            None => self.face_point_index_at(i, j),
        }
    }

    /// `getFaceTexCoordIndexAt(i, j)`. Texture coordinates have no
    /// per-face mode, so this falls straight back to the position index.
    pub fn face_tex_coord_index_at(&self, i: usize, j: usize) -> Option<u32> {
        if i >= self.indices.len() {
            return None;
        }
        match &self.tex_coord_indices {
            Some(list) => list.get(i)?.get(j),
            None => self.face_point_index_at(i, j),
        }
    }

    /// `getFacePointAt(i, j)`.
    pub fn face_point_at(&self, i: usize, j: usize) -> Option<Point3> {
        let index = self.face_point_index_at(i, j)? as usize;
        self.model.points.get(index).copied()
    }

    /// `getFaceCenter(i)` — the mean of the face's corners.
    pub fn face_center(&self, i: usize) -> Option<Point3> {
        let size = self.face_size(i)?;
        if size == 0 {
            return None;
        }
        let mut center = Vec3::zeros();
        for j in 0..size {
            center += self.face_point_at(i, j)?.coords;
        }
        Some(Point3::from(center / size as Real))
    }

    /// `computeNormalPerFace()` — one normal per face, from the first three
    /// corners, wound according to `ccw`.
    pub fn compute_normal_per_face(&self) -> Result<Vec<Vec3>> {
        let mut normals = Vec::with_capacity(self.face_count());
        for i in 0..self.face_count() {
            normals.push(self.face_normal(i)?);
        }
        Ok(normals)
    }

    /// `computeNormalPerVertex()` — area-weighted accumulation of the
    /// adjacent face normals, as upstream (which sums the *unnormalised*
    /// cross products, so larger faces count for more).
    ///
    /// Vertices touched by no face get `(1, 0, 0)`, matching upstream.
    pub fn compute_normal_per_vertex(&self) -> Result<Vec<Vec3>> {
        let point_count = self.model.point_count();
        let mut normals = vec![Vec3::zeros(); point_count];
        let mut touched = vec![false; point_count];

        for i in 0..self.face_count() {
            let raw = self.face_normal_unnormalized(i)?;
            let size = self.face_size(i).unwrap_or(0);
            for j in 0..size {
                let index = self
                    .face_point_index_at(i, j)
                    .ok_or_else(|| Error::invalid_index(format!("face {i} corner {j}")))?
                    as usize;
                let slot = normals
                    .get_mut(index)
                    .ok_or_else(|| Error::invalid_index(format!("point index {index}")))?;
                *slot += raw;
                touched[index] = true;
            }
        }

        for (normal, touched) in normals.iter_mut().zip(&touched) {
            if !touched {
                *normal = Vec3::new(1.0, 0.0, 0.0);
            }
            *normal = normalize_or_default(*normal);
        }

        Ok(normals)
    }

    /// `computeNormalList(pervertex)` — fills `normals` in the requested mode
    /// and records the mode on the model.
    pub fn compute_normal_list(&mut self, per_vertex: bool) -> Result<()> {
        let normals = if per_vertex {
            self.compute_normal_per_vertex()?
        } else {
            self.compute_normal_per_face()?
        };
        self.model.normal_per_vertex = per_vertex;
        self.model.normals = Some(Arc::new(normals));
        Ok(())
    }

    /// `checkNormalList()` — computes normals only if there are none.
    pub fn check_normal_list(&mut self) -> Result<()> {
        if self.model.has_normals() {
            return Ok(());
        }
        let per_vertex = self.model.normal_per_vertex;
        self.compute_normal_list(per_vertex)
    }

    /// The unit normal of face `i`.
    pub fn face_normal(&self, i: usize) -> Result<Vec3> {
        Ok(normalize_or_default(self.face_normal_unnormalized(i)?))
    }

    fn face_normal_unnormalized(&self, i: usize) -> Result<Vec3> {
        let size = self
            .face_size(i)
            .ok_or_else(|| Error::invalid_index(format!("face {i}")))?;
        if size < 3 {
            return Err(Error::degenerate(format!(
                "face {i} has {size} corners, need at least 3"
            )));
        }
        let corner = |j: usize| {
            self.face_point_at(i, j)
                .ok_or_else(|| Error::invalid_index(format!("face {i} corner {j}")))
        };
        let origin = corner(0)?;
        // Upstream swaps the two edge vectors rather than negating the cross
        // product, which is the same thing and keeps the winding explicit.
        let first = corner(if self.model.ccw { 1 } else { 2 })?;
        let second = corner(if self.model.ccw { 2 } else { 1 })?;
        Ok((first - origin).cross(&(second - origin)))
    }

    /// `isValid()` — indices in range, faces wide enough, parallel lists the
    /// right length.
    pub fn is_valid(&self) -> Result<()> {
        if self.model.points.len() < 3 {
            return Err(Error::degenerate("a mesh needs at least 3 points"));
        }
        if self.indices.is_empty() {
            return Err(Error::degenerate("a mesh needs at least 1 face"));
        }
        let point_count = self.model.points.len() as u32;
        for (i, face) in self.indices.iter().enumerate() {
            if face.len() < 3 {
                return Err(Error::degenerate(format!(
                    "face {i} has {} corners, need at least 3",
                    face.len()
                )));
            }
            for j in 0..face.len() {
                let index = face.get(j).expect("j < len");
                if index >= point_count {
                    return Err(Error::invalid_index(format!(
                        "face {i} corner {j} references point {index} of {point_count}"
                    )));
                }
            }
        }
        self.check_side_list(
            self.normal_indices.as_deref(),
            self.model.normals.as_ref().map(|n| n.len()),
            "normal",
        )?;
        self.check_side_list(
            self.color_indices.as_deref(),
            self.model.colors.as_ref().map(|c| c.len()),
            "color",
        )?;
        self.check_side_list(
            self.tex_coord_indices.as_deref(),
            self.model.tex_coords.as_ref().map(|t| t.len()),
            "texture coordinate",
        )?;
        Ok(())
    }

    fn check_side_list(
        &self,
        indices: Option<&[I]>,
        target_len: Option<usize>,
        what: &str,
    ) -> Result<()> {
        let Some(indices) = indices else {
            return Ok(());
        };
        if indices.len() != self.indices.len() {
            return Err(Error::invalid_index(format!(
                "{what} index list has {} faces, positions have {}",
                indices.len(),
                self.indices.len()
            )));
        }
        let Some(target_len) = target_len else {
            return Err(Error::invalid_index(format!(
                "{what} index list present but there is no {what} list"
            )));
        };
        for (i, face) in indices.iter().enumerate() {
            for j in 0..face.len() {
                let index = face.get(j).expect("j < len") as usize;
                if index >= target_len {
                    return Err(Error::invalid_index(format!(
                        "{what} face {i} corner {j} references {index} of {target_len}"
                    )));
                }
            }
        }
        Ok(())
    }
}

impl TriangleSet {
    /// The number of triangles. A `TriangleSet` has one per face.
    pub fn triangle_count(&self) -> usize {
        self.face_count()
    }
}

/// Normalises, falling back to `Mesh::DEFAULT_NORMAL_VALUE` when the result is
/// not unit length — upstream's guard against degenerate faces.
///
/// Upstream writes this as `normalize(); if (fabs(norm(n) - 1) > eps) n =
/// DEFAULT`, which does not actually catch the case it is aimed at: a
/// zero-length cross product normalises to NaN, and every comparison against
/// NaN is false, so the NaN survives into the vertex buffer. The check is done
/// on the input here instead.
fn normalize_or_default(v: Vec3) -> Vec3 {
    match v.try_normalize(EPSILON) {
        Some(normalized) if (normalized.norm() - 1.0).abs() <= EPSILON => normalized,
        _ => ExplicitModel::DEFAULT_NORMAL_VALUE,
    }
}

/// Upstream's `PointSet` — positions with optional per-point colours and a
/// point width.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointSet {
    pub points: Arc<Vec<Point3>>,
    pub colors: Option<Arc<Vec<Color4>>>,
    pub width: u32,
}

impl PointSet {
    /// `PointSet::DEFAULT_WIDTH`.
    pub const DEFAULT_WIDTH: u32 = 1;

    pub fn new(points: Vec<Point3>) -> Self {
        Self {
            points: Arc::new(points),
            colors: None,
            width: Self::DEFAULT_WIDTH,
        }
    }

    pub fn is_valid(&self) -> Result<()> {
        if self.points.is_empty() {
            return Err(Error::degenerate("a point set needs at least 1 point"));
        }
        Ok(())
    }
}

/// Upstream's `Polyline` — an open 3D polyline, also used as a mesh skeleton.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Polyline {
    pub points: Arc<Vec<Point3>>,
    pub colors: Option<Arc<Vec<Color4>>>,
    pub width: u32,
}

impl Polyline {
    /// `Polyline::DEFAULT_WIDTH`.
    pub const DEFAULT_WIDTH: u32 = 1;

    pub fn new(points: Vec<Point3>) -> Self {
        Self {
            points: Arc::new(points),
            colors: None,
            width: Self::DEFAULT_WIDTH,
        }
    }

    pub fn is_valid(&self) -> Result<()> {
        if self.points.len() < 2 {
            return Err(Error::degenerate("a polyline needs at least 2 points"));
        }
        Ok(())
    }

    /// The sum of the segment lengths — upstream's `getLength()`.
    pub fn length(&self) -> Real {
        self.points
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).norm())
            .sum()
    }
}

/// Upstream's `Group` — several geometries treated as one, with an optional
/// shared skeleton.
///
/// The plan sketches this variant as a bare `Vec<GeometryRef>`; the skeleton
/// is kept because upstream's group carries one and later phases measure
/// against it.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Group {
    pub geometries: Vec<GeometryRef>,
    pub skeleton: Option<Arc<Polyline>>,
}

impl Group {
    pub fn new(geometries: Vec<GeometryRef>) -> Self {
        Self {
            geometries,
            skeleton: None,
        }
    }

    pub fn is_valid(&self) -> Result<()> {
        if self.geometries.is_empty() {
            return Err(Error::degenerate("a group needs at least 1 geometry"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// A unit square in the z = 0 plane, wound counter-clockwise.
    fn square() -> TriangleSet {
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

    #[test]
    fn face_accessors_follow_the_index_list() {
        let mesh = square();
        assert_eq!(mesh.face_count(), 2);
        assert_eq!(mesh.face_size(0), Some(3));
        assert_eq!(mesh.face_size(2), None);
        assert_eq!(mesh.face_point_index_at(1, 2), Some(3));
        assert_eq!(mesh.face_point_at(1, 2), Some(Point3::new(0.0, 1.0, 0.0)));
        assert_relative_eq!(
            mesh.face_center(0).unwrap(),
            Point3::new(2.0 / 3.0, 1.0 / 3.0, 0.0),
            epsilon = 1e-6
        );
    }

    #[test]
    fn missing_side_index_lists_fall_back_to_positions() {
        let mesh = square();
        assert_eq!(mesh.face_normal_index_at(1, 0), mesh.face_point_index_at(1, 0));
        assert_eq!(mesh.face_color_index_at(1, 1), mesh.face_point_index_at(1, 1));
        assert_eq!(
            mesh.face_tex_coord_index_at(1, 2),
            mesh.face_point_index_at(1, 2)
        );
    }

    #[test]
    fn per_face_modes_index_by_face_ordinal() {
        let mut mesh = square();
        mesh.model.normal_per_vertex = false;
        mesh.model.color_per_vertex = false;
        assert_eq!(mesh.face_normal_index_at(1, 0), Some(1));
        assert_eq!(mesh.face_color_index_at(1, 2), Some(1));
        // Texture coordinates have no per-face mode.
        assert_eq!(mesh.face_tex_coord_index_at(1, 0), Some(0));
    }

    #[test]
    fn ccw_faces_point_along_positive_z() {
        let mesh = square();
        assert_relative_eq!(mesh.face_normal(0).unwrap(), Vec3::z(), epsilon = 1e-6);
    }

    #[test]
    fn cw_flag_flips_the_normal() {
        let mut mesh = square();
        mesh.model.ccw = false;
        assert_relative_eq!(mesh.face_normal(0).unwrap(), -Vec3::z(), epsilon = 1e-6);
    }

    #[test]
    fn degenerate_faces_get_the_default_normal() {
        let mesh = TriangleSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
            ],
            vec![[0, 1, 2]],
        );
        assert_eq!(
            mesh.face_normal(0).unwrap(),
            ExplicitModel::DEFAULT_NORMAL_VALUE
        );
    }

    #[test]
    fn normals_per_vertex_average_the_adjacent_faces() {
        let mut mesh = square();
        mesh.compute_normal_list(true).unwrap();
        let normals = mesh.model.normals.clone().unwrap();
        assert_eq!(normals.len(), 4);
        for n in normals.iter() {
            assert_relative_eq!(*n, Vec3::z(), epsilon = 1e-6);
        }
        assert!(mesh.model.normal_per_vertex);
    }

    #[test]
    fn normals_per_face_produce_one_per_face() {
        let mut mesh = square();
        mesh.compute_normal_list(false).unwrap();
        assert_eq!(mesh.model.normals.as_ref().unwrap().len(), 2);
        assert!(!mesh.model.normal_per_vertex);
    }

    #[test]
    fn untouched_vertices_get_upstreams_fallback_normal() {
        let mut mesh = square();
        // A fifth point no face references.
        Arc::make_mut(&mut mesh.model.points).push(Point3::new(9.0, 9.0, 9.0));
        let normals = mesh.compute_normal_per_vertex().unwrap();
        assert_eq!(normals[4], Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn check_normal_list_does_not_recompute() {
        let mut mesh = square();
        mesh.model.normals = Some(Arc::new(vec![Vec3::x(); 4]));
        mesh.check_normal_list().unwrap();
        assert_eq!(mesh.model.normals.as_ref().unwrap()[0], Vec3::x());
    }

    #[test]
    fn validity_rejects_out_of_range_indices() {
        let mesh = TriangleSet::new(
            vec![
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 7]],
        );
        assert!(matches!(mesh.is_valid(), Err(Error::InvalidIndex(_))));
    }

    #[test]
    fn validity_rejects_undersized_faces() {
        let mesh = FaceSet::new(
            vec![
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![vec![0, 1]],
        );
        assert!(matches!(mesh.is_valid(), Err(Error::DegenerateGeometry(_))));
    }

    #[test]
    fn validity_rejects_a_mismatched_side_index_list() {
        let mut mesh = square();
        mesh.model.normals = Some(Arc::new(vec![Vec3::z(); 4]));
        mesh.normal_indices = Some(vec![[0, 1, 2]]);
        assert!(matches!(mesh.is_valid(), Err(Error::InvalidIndex(_))));
    }

    #[test]
    fn a_valid_mesh_passes() {
        assert!(square().is_valid().is_ok());
    }

    #[test]
    fn quad_and_face_sets_share_the_accessors() {
        let quads = QuadSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2, 3]],
        );
        assert_eq!(quads.face_size(0), Some(4));
        assert_relative_eq!(quads.face_normal(0).unwrap(), Vec3::z(), epsilon = 1e-6);

        let faces = FaceSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(-1.0, 0.5, 0.0),
            ],
            vec![vec![0, 1, 2, 3, 4]],
        );
        assert_eq!(faces.face_size(0), Some(5));
        assert_eq!(faces.indices[0].iter().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn polyline_length_sums_the_segments() {
        let line = Polyline::new(vec![
            Point3::origin(),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(0.0, 3.0, 1.0),
        ]);
        assert_relative_eq!(line.length(), 4.0, epsilon = 1e-6);
        assert!(line.is_valid().is_ok());
        assert!(Polyline::new(vec![Point3::origin()]).is_valid().is_err());
    }

    #[test]
    fn point_set_needs_a_point() {
        assert!(PointSet::new(vec![Point3::origin()]).is_valid().is_ok());
        assert!(PointSet::new(vec![]).is_valid().is_err());
    }
}
