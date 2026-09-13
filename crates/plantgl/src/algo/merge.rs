//! Concatenation of explicit models.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/base/merge.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Merging is what turns a plant's few hundred organs into the handful of draw
//! calls the renderer wants, so it has to preserve everything a draw call
//! needs: positions rebased, faces re-indexed, and the side lists — normals,
//! colours, texture coordinates — carried across with *their own* offsets
//! rather than the vertex offset.
//!
//! # Two corrections to upstream
//!
//! **The reversed-winding path is broken upstream.** `Merge::apply(TriangleSet&)`
//! reverses a mesh whose `CCW` disagrees with the model's by emitting
//! `Index3(getAt(3), getAt(2), getAt(1))` — but an `Index3` has no element 3,
//! and element 0 is dropped. The port reverses the corner order properly.
//!
//! **Side lists are normalised rather than assumed compatible.** Upstream's
//! `checkNormals` recomputes normals whenever the two operands disagree about
//! per-vertex versus per-face, which silently discards authored normals. The
//! port instead gives every operand an explicit index list, which expresses
//! per-face normals exactly and needs no recomputation.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point3, Vec2, Vec3};
use crate::scenegraph::appearance::{AppearanceRef, Color4};
use crate::scenegraph::geometry::Geometry;
use crate::scenegraph::mesh::{
    ExplicitModel, FaceIndex, FaceSet, Index, Index3, Index4, IndexedMesh, PointSet, Polyline,
};
use crate::scenegraph::scene::Scene;

use super::discretize::{Discretizer, Explicit};

/// One mesh flattened to variable-arity faces with every side list indexed
/// explicitly — the form merging is closed under.
struct Neutral {
    points: Vec<Point3>,
    faces: Vec<Index>,
    normals: Option<(Vec<Vec3>, Vec<Index>)>,
    colors: Option<(Vec<Color4>, Vec<Index>)>,
    tex_coords: Option<(Vec<Vec2>, Vec<Index>)>,
    ccw: bool,
    solid: bool,
}

impl Neutral {
    fn from_mesh<I: FaceIndex>(mesh: &IndexedMesh<I>) -> Result<Self> {
        let faces: Vec<Index> = mesh.indices.iter().map(|f| f.iter().collect()).collect();

        // A side list with no index list of its own is indexed by the position
        // indices, or by the face ordinal in per-face mode; both are written
        // out here so the merged mesh needs no fallbacks.
        let side = |present: bool, per_vertex: bool, own: Option<&Vec<I>>| -> Option<Vec<Index>> {
            if !present {
                return None;
            }
            Some(match own {
                Some(list) => list.iter().map(|f| f.iter().collect()).collect(),
                None if per_vertex => faces.clone(),
                None => faces
                    .iter()
                    .enumerate()
                    .map(|(i, f)| vec![i as u32; f.len()])
                    .collect(),
            })
        };

        let model = &mesh.model;
        let normals = model.normals.as_ref().and_then(|list| {
            side(
                true,
                model.normal_per_vertex,
                mesh.normal_indices.as_ref(),
            )
            .map(|indices| (list.as_ref().clone(), indices))
        });
        let colors = model.colors.as_ref().and_then(|list| {
            side(true, model.color_per_vertex, mesh.color_indices.as_ref())
                .map(|indices| (list.as_ref().clone(), indices))
        });
        let tex_coords = model.tex_coords.as_ref().and_then(|list| {
            side(true, true, mesh.tex_coord_indices.as_ref())
                .map(|indices| (list.as_ref().clone(), indices))
        });

        Ok(Self {
            points: model.points.as_ref().clone(),
            faces,
            normals,
            colors,
            tex_coords,
            ccw: model.ccw,
            solid: model.solid,
        })
    }

    /// Appends `other`, rebasing each index list by that list's own offset.
    ///
    /// A side list present on only one operand is dropped: there is nothing
    /// truthful to invent for the other half, and half a texture-coordinate
    /// list is worse than none.
    fn append(&mut self, mut other: Neutral) {
        let point_offset = self.points.len() as u32;
        let reverse = self.ccw != other.ccw;

        self.points.append(&mut other.points);
        self.solid = self.solid && other.solid;

        append_side(
            &mut self.normals,
            other.normals,
            &other.faces,
            reverse,
        );
        append_side(&mut self.colors, other.colors, &other.faces, reverse);
        append_side(
            &mut self.tex_coords,
            other.tex_coords,
            &other.faces,
            reverse,
        );

        for face in other.faces {
            self.faces.push(rebase(&face, point_offset, reverse));
        }
    }

    fn into_explicit(self, shape: FaceShape) -> Result<Explicit> {
        let mut model = ExplicitModel::new(self.points);
        model.ccw = self.ccw;
        model.solid = self.solid;
        // Every side list carries explicit indices now, so the per-face modes
        // that would reinterpret them are off.
        model.normal_per_vertex = true;
        model.color_per_vertex = true;

        let (normals, normal_indices) = split(self.normals);
        let (colors, color_indices) = split(self.colors);
        let (tex_coords, tex_coord_indices) = split(self.tex_coords);
        model.normals = normals.map(Arc::new);
        model.colors = colors.map(Arc::new);
        model.tex_coords = tex_coords.map(Arc::new);

        match shape {
            FaceShape::Triangles => Ok(Explicit::TriangleSet(narrow(
                model,
                self.faces,
                normal_indices,
                color_indices,
                tex_coord_indices,
                to_index3,
            )?)),
            FaceShape::Quads => Ok(Explicit::QuadSet(narrow(
                model,
                self.faces,
                normal_indices,
                color_indices,
                tex_coord_indices,
                to_index4,
            )?)),
            FaceShape::Mixed => {
                let mut mesh = FaceSet::new(Vec::new(), self.faces);
                mesh.model = model;
                mesh.normal_indices = normal_indices;
                mesh.color_indices = color_indices;
                mesh.tex_coord_indices = tex_coord_indices;
                Ok(Explicit::FaceSet(mesh))
            }
        }
    }
}

/// The narrowest index type every operand fits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaceShape {
    Triangles,
    Quads,
    Mixed,
}

fn rebase(face: &Index, offset: u32, reverse: bool) -> Index {
    let mut rebased: Index = face.iter().map(|i| i + offset).collect();
    if reverse {
        rebased.reverse();
    }
    rebased
}

fn append_side<T>(
    into: &mut Option<(Vec<T>, Vec<Index>)>,
    from: Option<(Vec<T>, Vec<Index>)>,
    _faces: &[Index],
    reverse: bool,
) {
    match (into.as_mut(), from) {
        (Some((values, indices)), Some((mut more_values, more_indices))) => {
            let offset = values.len() as u32;
            values.append(&mut more_values);
            for face in more_indices {
                indices.push(rebase(&face, offset, reverse));
            }
        }
        // One side has it and the other does not: drop rather than fabricate.
        _ => *into = None,
    }
}

fn split<T>(side: Option<(Vec<T>, Vec<Index>)>) -> (Option<Vec<T>>, Option<Vec<Index>>) {
    match side {
        Some((values, indices)) => (Some(values), Some(indices)),
        None => (None, None),
    }
}

fn narrow<I: FaceIndex>(
    model: ExplicitModel,
    faces: Vec<Index>,
    normal_indices: Option<Vec<Index>>,
    color_indices: Option<Vec<Index>>,
    tex_coord_indices: Option<Vec<Index>>,
    convert: fn(&Index) -> Result<I>,
) -> Result<IndexedMesh<I>> {
    let narrow_all = |lists: Option<Vec<Index>>| -> Result<Option<Vec<I>>> {
        lists
            .map(|list| list.iter().map(convert).collect::<Result<Vec<I>>>())
            .transpose()
    };
    let mut mesh = IndexedMesh::new(
        Vec::new(),
        faces.iter().map(convert).collect::<Result<Vec<I>>>()?,
    );
    mesh.model = model;
    mesh.normal_indices = narrow_all(normal_indices)?;
    mesh.color_indices = narrow_all(color_indices)?;
    mesh.tex_coord_indices = narrow_all(tex_coord_indices)?;
    Ok(mesh)
}

fn to_index3(face: &Index) -> Result<Index3> {
    face.as_slice()
        .try_into()
        .map_err(|_| Error::invalid_index(format!("expected 3 corners, got {}", face.len())))
}

fn to_index4(face: &Index) -> Result<Index4> {
    face.as_slice()
        .try_into()
        .map_err(|_| Error::invalid_index(format!("expected 4 corners, got {}", face.len())))
}

/// Concatenates explicit models into one — upstream's `Merge`.
///
/// The result keeps the operands' type when they agree and widens to a
/// [`FaceSet`] when they do not, exactly as upstream's `Merge::apply` does by
/// converting through `FaceSet`. Point sets and polylines merge only with
/// their own kind; mixing them with a mesh is an error rather than upstream's
/// silent `false`.
pub fn merge_explicit(parts: Vec<Explicit>) -> Result<Explicit> {
    let mut parts = parts.into_iter();
    let first = parts
        .next()
        .ok_or_else(|| Error::degenerate("nothing to merge"))?;

    match first {
        Explicit::PointSet(first) => {
            let mut points = first.points.as_ref().clone();
            for part in parts {
                let Explicit::PointSet(next) = part else {
                    return Err(Error::unsupported(
                        "a point set cannot be merged with a mesh",
                    ));
                };
                points.extend_from_slice(&next.points);
            }
            let mut merged = PointSet::new(points);
            merged.width = first.width;
            Ok(Explicit::PointSet(merged))
        }
        Explicit::Polyline(first) => {
            let mut points = first.points.as_ref().clone();
            for part in parts {
                let Explicit::Polyline(next) = part else {
                    return Err(Error::unsupported(
                        "a polyline cannot be merged with a mesh",
                    ));
                };
                points.extend_from_slice(&next.points);
            }
            let mut merged = Polyline::new(points);
            merged.width = first.width;
            Ok(Explicit::Polyline(merged))
        }
        mesh => {
            let mut shape = shape_of(&mesh);
            let mut acc = neutral(&mesh)?;
            for part in parts {
                if matches!(part, Explicit::PointSet(_) | Explicit::Polyline(_)) {
                    return Err(Error::unsupported(
                        "a mesh cannot be merged with a point set or polyline",
                    ));
                }
                if shape_of(&part) != shape {
                    shape = FaceShape::Mixed;
                }
                acc.append(neutral(&part)?);
            }
            acc.into_explicit(shape)
        }
    }
}

fn shape_of(model: &Explicit) -> FaceShape {
    match model {
        Explicit::TriangleSet(_) => FaceShape::Triangles,
        Explicit::QuadSet(_) => FaceShape::Quads,
        _ => FaceShape::Mixed,
    }
}

fn neutral(model: &Explicit) -> Result<Neutral> {
    match model {
        Explicit::TriangleSet(m) => Neutral::from_mesh(m),
        Explicit::QuadSet(m) => Neutral::from_mesh(m),
        Explicit::FaceSet(m) => Neutral::from_mesh(m),
        Explicit::PointSet(_) | Explicit::Polyline(_) => Err(Error::unsupported(
            "point sets and polylines have no faces to merge",
        )),
    }
}

/// One appearance's worth of merged geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct MergedBatch {
    /// The appearance every shape in this batch carried, or `None` for the
    /// shapes that had none.
    pub appearance: Option<AppearanceRef>,
    /// Every shape in the batch, discretised, placed and concatenated.
    pub geometry: Explicit,
}

/// Discretises a scene and merges its shapes into one model per appearance —
/// the batching step that turns a plant into a few draw calls.
///
/// Shapes are grouped by appearance *identity*, so two shapes sharing an
/// [`Arc`] batch together and two equal-but-separate materials do not; that is
/// the grouping a renderer's material binding actually cares about. Batches
/// come back in first-appearance order, which keeps the output deterministic
/// for the golden tests.
pub fn merge_scene(scene: &Scene, discretizer: &mut Discretizer) -> Result<Vec<MergedBatch>> {
    let mut batches: Vec<(Option<AppearanceRef>, Vec<Explicit>)> = Vec::new();

    for shape in scene.iter() {
        let model = discretizer.discretize(&shape.geometry)?;
        let slot = batches.iter_mut().find(|(appearance, _)| {
            match (appearance, &shape.appearance) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
        });
        match slot {
            Some((_, parts)) => parts.push(model),
            None => batches.push((shape.appearance.clone(), vec![model])),
        }
    }

    batches
        .into_iter()
        .map(|(appearance, parts)| {
            Ok(MergedBatch {
                appearance,
                geometry: merge_explicit(parts)?,
            })
        })
        .collect()
}

/// Merges a geometry tree's leaves into a single explicit model.
pub fn merge_geometry(geometry: &Geometry, discretizer: &mut Discretizer) -> Result<Explicit> {
    discretizer.discretize(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::bbox::bounding_box;
    use crate::algo::discretize::{discretize, DiscretizeCtx};
    use crate::algo::measure::surface_area;
    use crate::math::Vec3;
    use crate::scenegraph::appearance::{Appearance, Material};
    use crate::scenegraph::mesh::{QuadSet, TriangleSet};
    use crate::scenegraph::primitive::Box3;
    use crate::scenegraph::scene::Shape;
    use approx::assert_relative_eq;

    fn square(offset: f32) -> TriangleSet {
        TriangleSet::new(
            vec![
                Point3::new(offset, 0.0, 0.0),
                Point3::new(offset + 1.0, 0.0, 0.0),
                Point3::new(offset + 1.0, 1.0, 0.0),
                Point3::new(offset, 1.0, 0.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    }

    /// Acceptance (T8.5): `merge` of *n* sets preserves total triangle count
    /// and bbox.
    #[test]
    fn merging_preserves_triangle_count_and_bbox() {
        let parts: Vec<Explicit> = (0..5)
            .map(|i| Explicit::TriangleSet(square(i as f32 * 2.0)))
            .collect();
        let expected_faces: usize = parts.iter().map(Explicit::face_count).sum();

        let merged = merge_explicit(parts).unwrap();
        assert_eq!(merged.face_count(), expected_faces);
        assert_eq!(merged.points().len(), 20);

        let bbox = bounding_box(&merged.clone().into_geometry().into_ref())
            .unwrap()
            .unwrap();
        assert_relative_eq!(bbox.lower_left, Point3::origin(), epsilon = 1e-6);
        assert_relative_eq!(bbox.upper_right, Point3::new(9.0, 1.0, 0.0), epsilon = 1e-6);
    }

    #[test]
    fn merging_preserves_total_surface_area() {
        let parts: Vec<Explicit> = (0..4)
            .map(|i| Explicit::TriangleSet(square(i as f32 * 2.0)))
            .collect();
        let total: f32 = parts.iter().map(|p| surface_area(p).unwrap()).sum();
        let merged = merge_explicit(parts).unwrap();
        assert_relative_eq!(surface_area(&merged).unwrap(), total, epsilon = 1e-5);
    }

    #[test]
    fn indices_are_rebased_onto_the_appended_points() {
        let merged = merge_explicit(vec![
            Explicit::TriangleSet(square(0.0)),
            Explicit::TriangleSet(square(5.0)),
        ])
        .unwrap();
        let Explicit::TriangleSet(mesh) = &merged else {
            panic!("two triangle sets merge to a triangle set");
        };
        assert_eq!(mesh.indices[2], [4, 5, 6]);
        assert_eq!(mesh.indices[3], [4, 6, 7]);
        assert_relative_eq!(
            mesh.face_point_at(2, 0).unwrap(),
            Point3::new(5.0, 0.0, 0.0),
            epsilon = 1e-6
        );
    }

    #[test]
    fn a_mesh_wound_the_other_way_is_reversed_rather_than_mangled() {
        // Upstream reads element 3 of an Index3 here and drops element 0.
        let mut reversed = square(5.0);
        reversed.model.ccw = false;
        // Declaring the corner list clockwise flips what the mesh's faces
        // actually point at, so this operand's outward normal is -z.
        assert_relative_eq!(reversed.face_normal(0).unwrap(), -Vec3::z(), epsilon = 1e-5);

        let merged = merge_explicit(vec![
            Explicit::TriangleSet(square(0.0)),
            Explicit::TriangleSet(reversed),
        ])
        .unwrap();
        let Explicit::TriangleSet(mesh) = &merged else {
            panic!("triangle sets merge to a triangle set");
        };
        // Corner order reversed, and every corner kept — upstream's version
        // emits [_, 6, 5] from an out-of-range read and loses corner 4.
        assert_eq!(mesh.indices[2], [6, 5, 4]);
        assert_eq!(mesh.indices[3], [7, 6, 4]);

        // The merged model is counter-clockwise, and reversing is what makes
        // each operand keep the normal it had before the merge.
        assert!(mesh.model.ccw);
        assert_relative_eq!(mesh.face_normal(0).unwrap(), Vec3::z(), epsilon = 1e-5);
        assert_relative_eq!(mesh.face_normal(2).unwrap(), -Vec3::z(), epsilon = 1e-5);
    }

    #[test]
    fn mixed_face_arities_widen_to_a_face_set() {
        let triangles = discretize(&Geometry::from(Box3::cube(1.0))).unwrap();
        let quads = Explicit::QuadSet(QuadSet::new(
            vec![
                Point3::new(5.0, 0.0, 0.0),
                Point3::new(6.0, 0.0, 0.0),
                Point3::new(6.0, 1.0, 0.0),
                Point3::new(5.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2, 3]],
        ));
        // A box is quads, so pair it with triangles to force the widening.
        let merged = merge_explicit(vec![
            Explicit::TriangleSet(square(0.0)),
            quads,
            triangles,
        ])
        .unwrap();
        assert!(matches!(merged, Explicit::FaceSet(_)));
        assert_eq!(merged.face_count(), 2 + 1 + 6);
    }

    #[test]
    fn quad_sets_stay_quad_sets() {
        let merged = merge_explicit(vec![
            discretize(&Geometry::from(Box3::cube(1.0))).unwrap(),
            discretize(&Geometry::from(Box3::cube(2.0))).unwrap(),
        ])
        .unwrap();
        assert!(matches!(merged, Explicit::QuadSet(_)));
        assert_eq!(merged.face_count(), 12);
    }

    #[test]
    fn texture_coordinates_are_rebased_by_their_own_offset() {
        let a = discretize(&Geometry::from(Box3::cube(1.0))).unwrap();
        let b = discretize(&Geometry::from(Box3::cube(2.0))).unwrap();
        let merged = merge_explicit(vec![a, b]).unwrap();
        let Explicit::QuadSet(mesh) = &merged else {
            panic!("boxes merge to a quad set");
        };
        // Each box contributes 4 texture coordinates and 6 faces; the second
        // box's faces must point at the second block of coordinates.
        assert_eq!(mesh.model.tex_coords.as_ref().unwrap().len(), 8);
        assert_eq!(mesh.tex_coord_indices.as_ref().unwrap()[6], [4, 5, 6, 7]);
        assert!(mesh.is_valid().is_ok());
    }

    #[test]
    fn a_side_list_only_one_operand_has_is_dropped() {
        let with_uvs = discretize(&Geometry::from(Box3::cube(1.0))).unwrap();
        let without = Discretizer::with_defaults()
            .with_tex_coords(false)
            .discretize(&Geometry::from(Box3::cube(2.0)))
            .unwrap();
        let merged = merge_explicit(vec![with_uvs, without]).unwrap();
        assert!(!merged.model().has_tex_coords());
    }

    #[test]
    fn merging_a_mesh_with_a_point_set_is_an_error() {
        let mesh = Explicit::TriangleSet(square(0.0));
        let points = Explicit::PointSet(PointSet::new(vec![Point3::origin()]));
        assert!(matches!(
            merge_explicit(vec![mesh, points.clone()]),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            merge_explicit(vec![points, Explicit::TriangleSet(square(0.0))]),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn point_sets_merge_with_point_sets() {
        let merged = merge_explicit(vec![
            Explicit::PointSet(PointSet::new(vec![Point3::origin()])),
            Explicit::PointSet(PointSet::new(vec![Point3::new(1.0, 0.0, 0.0)])),
        ])
        .unwrap();
        assert_eq!(merged.points().len(), 2);
    }

    #[test]
    fn merging_nothing_is_an_error() {
        assert!(matches!(
            merge_explicit(Vec::new()),
            Err(Error::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn a_scene_batches_one_model_per_appearance() {
        let leaf: AppearanceRef = Arc::new(Appearance::Material(Material::new("leaf")));
        let bark: AppearanceRef = Arc::new(Appearance::Material(Material::new("bark")));
        let scene = Scene::from_shapes(vec![
            Shape::new(Geometry::from(Box3::cube(1.0)).into_ref())
                .with_appearance(leaf.clone()),
            Shape::new(Geometry::from(Box3::cube(2.0)).into_ref())
                .with_appearance(bark.clone()),
            Shape::new(Geometry::from(Box3::cube(3.0)).into_ref()).with_appearance(leaf),
            Shape::new(Geometry::from(Box3::cube(4.0)).into_ref()),
        ]);

        let mut discretizer = Discretizer::new(DiscretizeCtx::default());
        let batches = merge_scene(&scene, &mut discretizer).unwrap();

        assert_eq!(batches.len(), 3);
        // First-appearance order: leaf, bark, then the unpainted shape.
        assert_eq!(batches[0].geometry.face_count(), 12);
        assert_eq!(batches[1].geometry.face_count(), 6);
        assert!(batches[2].appearance.is_none());
        assert!(Arc::ptr_eq(
            batches[1].appearance.as_ref().unwrap(),
            &bark
        ));
    }

    #[test]
    fn batching_an_empty_scene_yields_no_batches() {
        let mut discretizer = Discretizer::with_defaults();
        assert!(merge_scene(&Scene::new(), &mut discretizer).unwrap().is_empty());
    }
}
