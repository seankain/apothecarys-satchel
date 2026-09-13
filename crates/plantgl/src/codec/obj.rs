//! Wavefront OBJ/MTL import and export.
//!
//! Original to this port. PlantGL ships PLY, VRML, X3D, POV-Ray, Geomview,
//! VGStar, LIG, DTA, binary and XML codecs (`src/cpp/plantgl/algo/codec/`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189) but no OBJ writer, so nothing
//! here is translated from upstream. It lives in this crate, so it carries the
//! crate's license, CeCILL-C; see crates/plantgl/LICENSE.
//!
//! OBJ has no notion of a transform, so writing flattens the scene: each
//! leaf's accumulated matrix and deformations are baked into its vertices.
//! Floats are written with a fixed precision, and `-0` is normalised to `0`,
//! so the output is byte-stable across runs and platforms — which is what
//! makes the golden tests in `tests/golden.rs` diffable.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::algo::matrix::{MatrixComputer, Placement};
use crate::error::{Error, Result};
use crate::math::{Mat4, Point3, Real, Vec2, Vec3};
use crate::scenegraph::appearance::{Appearance, Material};
use crate::scenegraph::geometry::Geometry;
use crate::scenegraph::mesh::{FaceSet, Index, TriangleSet};
use crate::scenegraph::scene::{Scene, Shape};
use crate::scenegraph::transform::Deformation;

/// How to write an OBJ file.
#[derive(Debug, Clone)]
pub struct ObjOptions {
    /// The `.mtl` filename to reference from the `.obj`. `None` writes no
    /// `mtllib` line and no material file.
    pub mtllib: Option<String>,
    /// Digits after the decimal point. Six is enough for millimetre precision
    /// on a metre-scale plant and short enough to diff.
    pub precision: usize,
    /// Whether to write `vn` lines, computing normals where a mesh has none.
    pub write_normals: bool,
    /// Whether to write `vt` lines for meshes that carry texture coordinates.
    pub write_tex_coords: bool,
}

impl Default for ObjOptions {
    fn default() -> Self {
        Self {
            mtllib: Some("scene.mtl".to_string()),
            precision: 6,
            write_normals: true,
            write_tex_coords: true,
        }
    }
}

/// The two files an OBJ export produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjFiles {
    pub obj: String,
    /// Empty when [`ObjOptions::mtllib`] is `None` or no shape has an
    /// appearance.
    pub mtl: String,
}

/// Writes a scene as OBJ (and MTL) with the default options.
pub fn to_obj(scene: &Scene) -> Result<ObjFiles> {
    to_obj_with(scene, &ObjOptions::default())
}

/// Writes a scene as OBJ (and MTL).
pub fn to_obj_with(scene: &Scene, options: &ObjOptions) -> Result<ObjFiles> {
    let mut obj = String::new();
    let mut materials: BTreeMap<String, Material> = BTreeMap::new();

    if let Some(mtllib) = &options.mtllib {
        writeln!(obj, "mtllib {mtllib}").map_err(Error::codec)?;
    }

    // OBJ indices are 1-based and run across the whole file.
    let mut vertex_base: usize = 1;
    let mut normal_base: usize = 1;
    let mut tex_base: usize = 1;

    let mut computer = MatrixComputer::new();
    for (shape_index, shape) in scene.iter().enumerate() {
        let group = group_name(shape, shape_index);
        let material = material_name(shape, shape_index);
        if let (Some(name), Some(appearance)) = (&material, shape.appearance.as_deref()) {
            if let Appearance::Material(m) = appearance {
                materials.entry(name.clone()).or_insert_with(|| m.clone());
            } else {
                materials
                    .entry(name.clone())
                    .or_default();
            }
        }

        computer.clear();
        let leaves = computer.flatten(&shape.geometry)?;
        for (placement, leaf) in leaves {
            let Some(mesh) = as_face_mesh(&leaf)? else {
                continue;
            };
            let mesh = place(&mesh, &placement);

            writeln!(obj, "g {group}").map_err(Error::codec)?;
            if let Some(name) = &material {
                writeln!(obj, "usemtl {name}").map_err(Error::codec)?;
            }

            for point in mesh.model.points.iter() {
                writeln!(
                    obj,
                    "v {} {} {}",
                    real(point.x, options.precision),
                    real(point.y, options.precision),
                    real(point.z, options.precision)
                )
                .map_err(Error::codec)?;
            }

            let normals = if options.write_normals {
                match &mesh.model.normals {
                    Some(normals) => normals.as_ref().clone(),
                    None => mesh.compute_normal_per_vertex()?,
                }
            } else {
                Vec::new()
            };
            for normal in &normals {
                writeln!(
                    obj,
                    "vn {} {} {}",
                    real(normal.x, options.precision),
                    real(normal.y, options.precision),
                    real(normal.z, options.precision)
                )
                .map_err(Error::codec)?;
            }

            let tex_coords: Vec<Vec2> = if options.write_tex_coords {
                mesh.model
                    .tex_coords
                    .as_ref()
                    .map(|t| t.as_ref().clone())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            for uv in &tex_coords {
                writeln!(
                    obj,
                    "vt {} {}",
                    real(uv.x, options.precision),
                    real(uv.y, options.precision)
                )
                .map_err(Error::codec)?;
            }

            for face in 0..mesh.face_count() {
                let size = mesh.face_size(face).unwrap_or(0);
                let mut line = String::from("f");
                for corner in 0..size {
                    let v = mesh
                        .face_point_index_at(face, corner)
                        .ok_or_else(|| Error::invalid_index(format!("face {face}")))?
                        as usize
                        + vertex_base;
                    let vt = (!tex_coords.is_empty())
                        .then(|| mesh.face_tex_coord_index_at(face, corner))
                        .flatten()
                        .map(|i| i as usize + tex_base);
                    let vn = (!normals.is_empty())
                        .then(|| {
                            if mesh.model.normal_per_vertex {
                                mesh.face_normal_index_at(face, corner)
                            } else {
                                Some(face as u32)
                            }
                        })
                        .flatten()
                        .map(|i| i as usize + normal_base);
                    match (vt, vn) {
                        (Some(vt), Some(vn)) => write!(line, " {v}/{vt}/{vn}"),
                        (Some(vt), None) => write!(line, " {v}/{vt}"),
                        (None, Some(vn)) => write!(line, " {v}//{vn}"),
                        (None, None) => write!(line, " {v}"),
                    }
                    .map_err(Error::codec)?;
                }
                writeln!(obj, "{line}").map_err(Error::codec)?;
            }

            vertex_base += mesh.model.points.len();
            normal_base += normals.len();
            tex_base += tex_coords.len();
        }
    }

    let mtl = if options.mtllib.is_some() && !materials.is_empty() {
        write_mtl(&materials, options)?
    } else {
        String::new()
    };

    Ok(ObjFiles { obj, mtl })
}

fn write_mtl(materials: &BTreeMap<String, Material>, options: &ObjOptions) -> Result<String> {
    let mut mtl = String::new();
    for (name, material) in materials {
        writeln!(mtl, "newmtl {name}").map_err(Error::codec)?;
        write_color(&mut mtl, "Ka", material.ambient.to_clamped(), options)?;
        write_color(
            &mut mtl,
            "Kd",
            material.diffuse_color().to_clamped(),
            options,
        )?;
        write_color(&mut mtl, "Ks", material.specular.to_clamped(), options)?;
        write_color(&mut mtl, "Ke", material.emission.to_clamped(), options)?;
        // MTL's Ns runs 0..1000; PlantGL's shininess is a 0..1 coefficient.
        writeln!(
            mtl,
            "Ns {}",
            real(material.shininess * 128.0, options.precision)
        )
        .map_err(Error::codec)?;
        // MTL's d is dissolve — opacity, the complement of transparency.
        writeln!(
            mtl,
            "d {}",
            real(1.0 - material.transparency, options.precision)
        )
        .map_err(Error::codec)?;
    }
    Ok(mtl)
}

fn write_color(
    out: &mut String,
    key: &str,
    rgb: [Real; 3],
    options: &ObjOptions,
) -> Result<()> {
    writeln!(
        out,
        "{key} {} {} {}",
        real(rgb[0], options.precision),
        real(rgb[1], options.precision),
        real(rgb[2], options.precision)
    )
    .map_err(Error::codec)
}

/// Fixed-precision float formatting with `-0` folded to `0`, so two runs of
/// the same scene produce byte-identical files.
fn real(value: Real, precision: usize) -> String {
    let formatted = format!("{value:.precision$}");
    if formatted.starts_with('-') && formatted[1..].chars().all(|c| c == '0' || c == '.') {
        formatted[1..].to_string()
    } else {
        formatted
    }
}

fn group_name(shape: &Shape, index: usize) -> String {
    if shape.id != crate::scenegraph::scene::NOID {
        format!("shape_{}", shape.id)
    } else {
        format!("shape_{index}")
    }
}

fn material_name(shape: &Shape, index: usize) -> Option<String> {
    let appearance = shape.appearance.as_deref()?;
    Some(
        appearance
            .name()
            .map(str::to_string)
            .unwrap_or_else(|| format!("material_{index}")),
    )
}

/// Reduces any face-bearing leaf to a `FaceSet`, so the writer has one shape
/// of data to emit. Returns `Ok(None)` for geometry OBJ cannot represent —
/// point sets and polylines have no faces.
fn as_face_mesh(geometry: &Geometry) -> Result<Option<FaceSet>> {
    let mesh = match geometry {
        Geometry::TriangleSet(m) => FaceSet {
            model: m.model.clone(),
            indices: m.indices.iter().map(|i| i.to_vec()).collect(),
            normal_indices: m
                .normal_indices
                .as_ref()
                .map(|l| l.iter().map(|i| i.to_vec()).collect()),
            color_indices: m
                .color_indices
                .as_ref()
                .map(|l| l.iter().map(|i| i.to_vec()).collect()),
            tex_coord_indices: m
                .tex_coord_indices
                .as_ref()
                .map(|l| l.iter().map(|i| i.to_vec()).collect()),
        },
        Geometry::QuadSet(m) => FaceSet {
            model: m.model.clone(),
            indices: m.indices.iter().map(|i| i.to_vec()).collect(),
            normal_indices: m
                .normal_indices
                .as_ref()
                .map(|l| l.iter().map(|i| i.to_vec()).collect()),
            color_indices: m
                .color_indices
                .as_ref()
                .map(|l| l.iter().map(|i| i.to_vec()).collect()),
            tex_coord_indices: m
                .tex_coord_indices
                .as_ref()
                .map(|l| l.iter().map(|i| i.to_vec()).collect()),
        },
        Geometry::FaceSet(m) => m.clone(),
        Geometry::PointSet(_) | Geometry::Polyline(_) => return Ok(None),
        parametric => {
            return Err(Error::unsupported(format!(
                "OBJ has no parametric geometry: discretise the {} first",
                parametric.type_name()
            )))
        }
    };
    Ok(Some(mesh))
}

/// Bakes a placement into a mesh's vertices: deformations first, on the
/// points, then the affine matrix.
fn place(mesh: &FaceSet, placement: &Placement) -> FaceSet {
    let mut placed = mesh.clone();

    let mut points = mesh.model.points.as_ref().clone();
    for deformation in &placement.deformations {
        points = match deformation {
            Deformation::Taper(taper) => taper.transform(&points),
        };
    }

    let matrix = &placement.matrix;
    if !placement.deformations.is_empty() || matrix != &Mat4::identity() {
        let transformed: Vec<Point3> = points
            .iter()
            .map(|p| Point3::from_homogeneous(matrix * p.to_homogeneous()).unwrap_or(*p))
            .collect();
        placed.model.points = std::sync::Arc::new(transformed);

        // Normals transform by the inverse transpose; a non-uniform scale
        // would skew them otherwise. Fall back to recomputing when the
        // matrix is singular.
        placed.model.normals = match (&mesh.model.normals, matrix.try_inverse()) {
            (Some(normals), Some(inverse)) => {
                let normal_matrix = inverse.transpose().fixed_view::<3, 3>(0, 0).into_owned();
                Some(std::sync::Arc::new(
                    normals
                        .iter()
                        .map(|n| normalize_or_zero(normal_matrix * n))
                        .collect::<Vec<Vec3>>(),
                ))
            }
            _ => None,
        };
    } else {
        placed.model.points = std::sync::Arc::new(points);
    }

    placed
}

fn normalize_or_zero(v: Vec3) -> Vec3 {
    v.try_normalize(1e-10).unwrap_or(v)
}

/// A `.obj` parsed back into a [`Scene`].
///
/// Materials are not read from the `.mtl`; a `usemtl` line becomes a
/// [`Material`] carrying that name, which is enough to round-trip the
/// material assignment.
pub fn from_obj(text: &str) -> Result<Scene> {
    let mut positions: Vec<Point3> = Vec::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut tex_coords: Vec<Vec2> = Vec::new();

    let mut scene = Scene::new();
    let mut group: Option<String> = None;
    let mut material: Option<String> = None;
    let mut faces: Vec<Index> = Vec::new();
    let mut normal_faces: Vec<Index> = Vec::new();
    let mut tex_faces: Vec<Index> = Vec::new();
    let mut any_normal_index = false;
    let mut any_tex_index = false;

    // Each batch of faces becomes one shape carrying only the vertices it
    // actually references, remapped in ascending index order — which is the
    // order the writer emitted them in, so writing a file, reading it back and
    // writing it again reaches a fixed point.
    macro_rules! flush {
        () => {
            if !faces.is_empty() {
                let mesh = compact(
                    std::mem::take(&mut faces),
                    &positions,
                    (any_normal_index && !normals.is_empty())
                        .then(|| std::mem::take(&mut normal_faces)),
                    &normals,
                    (any_tex_index && !tex_coords.is_empty())
                        .then(|| std::mem::take(&mut tex_faces)),
                    &tex_coords,
                );
                normal_faces.clear();
                tex_faces.clear();
                let mut shape = Shape::new(narrow(mesh).into_ref());
                if let Some(name) = &material {
                    shape.appearance = Some(std::sync::Arc::new(Appearance::Material(
                        Material::new(name.clone()),
                    )));
                }
                if let Some(id) = group.as_deref().and_then(parse_shape_id) {
                    shape.id = id;
                }
                scene.push(shape);
            }
        };
    }

    for (number, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let keyword = fields.next().unwrap_or("");
        let bad = |what: &str| Error::codec(format!("line {}: bad {what}", number + 1));
        match keyword {
            "v" => positions.push(Point3::new(
                parse_real(fields.next()).ok_or_else(|| bad("v"))?,
                parse_real(fields.next()).ok_or_else(|| bad("v"))?,
                parse_real(fields.next()).ok_or_else(|| bad("v"))?,
            )),
            "vn" => normals.push(Vec3::new(
                parse_real(fields.next()).ok_or_else(|| bad("vn"))?,
                parse_real(fields.next()).ok_or_else(|| bad("vn"))?,
                parse_real(fields.next()).ok_or_else(|| bad("vn"))?,
            )),
            "vt" => tex_coords.push(Vec2::new(
                parse_real(fields.next()).ok_or_else(|| bad("vt"))?,
                parse_real(fields.next()).unwrap_or(0.0),
            )),
            "g" | "o" => {
                flush!();
                group = fields.next().map(str::to_string);
            }
            "usemtl" => {
                flush!();
                material = fields.next().map(str::to_string);
            }
            "f" => {
                if faces.is_empty() {
                    // A fresh batch: the side-index flags describe this batch
                    // only, and the previous one has already been flushed.
                    any_normal_index = false;
                    any_tex_index = false;
                }
                let mut face = Vec::new();
                let mut normal_face = Vec::new();
                let mut tex_face = Vec::new();
                for corner in fields {
                    let mut parts = corner.split('/');
                    let v = resolve(parts.next(), positions.len()).ok_or_else(|| bad("f"))?;
                    face.push(v);
                    let vt = resolve(parts.next(), tex_coords.len());
                    let vn = resolve(parts.next(), normals.len());
                    tex_face.push(vt.unwrap_or(v));
                    normal_face.push(vn.unwrap_or(v));
                    any_tex_index |= vt.is_some();
                    any_normal_index |= vn.is_some();
                }
                if face.len() < 3 {
                    return Err(bad("f: fewer than 3 corners"));
                }
                faces.push(face);
                normal_faces.push(normal_face);
                tex_faces.push(tex_face);
            }
            // mtllib, s, and anything else this reader does not model.
            _ => {}
        }
    }
    flush!();

    Ok(scene)
}

/// Rebuilds one batch of faces against only the vertices it references.
///
/// Indices are renumbered in ascending order of their position in the file,
/// so a mesh written out in its own vertex order comes back in that same
/// order.
fn compact(
    faces: Vec<Index>,
    positions: &[Point3],
    normal_faces: Option<Vec<Index>>,
    normals: &[Vec3],
    tex_faces: Option<Vec<Index>>,
    tex_coords: &[Vec2],
) -> FaceSet {
    fn used(faces: &[Index]) -> Vec<u32> {
        let mut all: Vec<u32> = faces.iter().flatten().copied().collect();
        all.sort_unstable();
        all.dedup();
        all
    }

    fn remap(faces: &[Index], used: &[u32]) -> Vec<Index> {
        faces
            .iter()
            .map(|face| {
                face.iter()
                    .map(|i| used.partition_point(|u| u < i) as u32)
                    .collect()
            })
            .collect()
    }

    fn gather<T: Copy>(source: &[T], used: &[u32]) -> Vec<T> {
        used.iter()
            .filter_map(|i| source.get(*i as usize).copied())
            .collect()
    }

    let used_positions = used(&faces);
    let mut mesh = FaceSet::new(
        gather(positions, &used_positions),
        remap(&faces, &used_positions),
    );

    if let Some(normal_faces) = normal_faces {
        let used_normals = used(&normal_faces);
        mesh.model.normals = Some(std::sync::Arc::new(gather(normals, &used_normals)));
        mesh.normal_indices = Some(remap(&normal_faces, &used_normals));
    }
    if let Some(tex_faces) = tex_faces {
        let used_tex = used(&tex_faces);
        mesh.model.tex_coords = Some(std::sync::Arc::new(gather(tex_coords, &used_tex)));
        mesh.tex_coord_indices = Some(remap(&tex_faces, &used_tex));
    }
    mesh
}

/// Picks the tightest mesh type the faces allow, so a `TriangleSet` written
/// out comes back as a `TriangleSet`.
fn narrow(mesh: FaceSet) -> Geometry {
    if mesh.indices.iter().all(|f| f.len() == 3) {
        let convert = |list: &Option<Vec<Index>>| {
            list.as_ref()
                .map(|l| l.iter().map(|f| [f[0], f[1], f[2]]).collect())
        };
        return Geometry::TriangleSet(TriangleSet {
            model: mesh.model.clone(),
            indices: mesh.indices.iter().map(|f| [f[0], f[1], f[2]]).collect(),
            normal_indices: convert(&mesh.normal_indices),
            color_indices: convert(&mesh.color_indices),
            tex_coord_indices: convert(&mesh.tex_coord_indices),
        });
    }
    if mesh.indices.iter().all(|f| f.len() == 4) {
        let convert = |list: &Option<Vec<Index>>| {
            list.as_ref()
                .map(|l| l.iter().map(|f| [f[0], f[1], f[2], f[3]]).collect())
        };
        return Geometry::QuadSet(crate::scenegraph::mesh::QuadSet {
            model: mesh.model.clone(),
            indices: mesh
                .indices
                .iter()
                .map(|f| [f[0], f[1], f[2], f[3]])
                .collect(),
            normal_indices: convert(&mesh.normal_indices),
            color_indices: convert(&mesh.color_indices),
            tex_coord_indices: convert(&mesh.tex_coord_indices),
        });
    }
    Geometry::FaceSet(mesh)
}

fn parse_shape_id(group: &str) -> Option<u32> {
    group.strip_prefix("shape_")?.parse().ok()
}

fn parse_real(field: Option<&str>) -> Option<Real> {
    field?.parse().ok()
}

/// Resolves an OBJ index field: 1-based when positive, relative to the end
/// when negative, absent when empty.
fn resolve(field: Option<&str>, count: usize) -> Option<u32> {
    let field = field?.trim();
    if field.is_empty() {
        return None;
    }
    let index: i64 = field.parse().ok()?;
    let zero_based = if index > 0 {
        index - 1
    } else if index < 0 {
        count as i64 + index
    } else {
        return None;
    };
    u32::try_from(zero_based).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::bbox::bounding_box;
    use crate::math::Vec3;
    use crate::scenegraph::appearance::Color3;
    use crate::scenegraph::transform::{Transform, Transformed};
    use approx::assert_relative_eq;
    use std::sync::Arc;

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

    fn square_scene() -> Scene {
        Scene::from_shapes(vec![Shape::new(Geometry::from(square()).into_ref()).with_id(1)])
    }

    /// Acceptance (T8.3): a hand-built `TriangleSet` round-trips to OBJ with
    /// identical vertex and index counts and an identical bbox.
    #[test]
    fn triangle_set_round_trips() {
        let original = square_scene();
        let files = to_obj(&original).unwrap();
        let restored = from_obj(&files.obj).unwrap();

        assert_eq!(restored.len(), 1);
        let Geometry::TriangleSet(mesh) = restored.shapes[0].geometry.as_ref() else {
            panic!("expected a TriangleSet, got {restored:?}");
        };
        assert_eq!(mesh.model.points.len(), 4);
        assert_eq!(mesh.indices.len(), 2);
        assert_eq!(mesh.indices, square().indices);

        let before = bounding_box(&original.shapes[0].geometry).unwrap().unwrap();
        let after = bounding_box(&restored.shapes[0].geometry).unwrap().unwrap();
        assert_relative_eq!(before.lower_left, after.lower_left, epsilon = 1e-6);
        assert_relative_eq!(before.upper_right, after.upper_right, epsilon = 1e-6);
    }

    #[test]
    fn writing_is_byte_stable() {
        let scene = square_scene();
        assert_eq!(to_obj(&scene).unwrap(), to_obj(&scene).unwrap());
    }

    #[test]
    fn negative_zero_is_normalised() {
        assert_eq!(real(-0.0, 6), "0.000000");
        assert_eq!(real(-0.0000001, 6), "0.000000");
        assert_eq!(real(-1.5, 2), "-1.50");
    }

    #[test]
    fn transforms_are_baked_into_the_vertices() {
        let scene = Scene::from_shapes(vec![Shape::new(
            Geometry::from(Transformed::new(
                Transform::Translated(Vec3::new(10.0, 0.0, 0.0)),
                Geometry::from(square()).into_ref(),
            ))
            .into_ref(),
        )]);
        let files = to_obj(&scene).unwrap();
        assert!(files.obj.contains("v 10.000000 0.000000 0.000000"));

        let restored = from_obj(&files.obj).unwrap();
        let bbox = bounding_box(&restored.shapes[0].geometry).unwrap().unwrap();
        assert_relative_eq!(bbox.lower_left, Point3::new(10.0, 0.0, 0.0), epsilon = 1e-6);
        assert_relative_eq!(bbox.upper_right, Point3::new(11.0, 1.0, 0.0), epsilon = 1e-6);
    }

    #[test]
    fn a_taper_is_applied_before_the_matrix() {
        let bar = Geometry::from(TriangleSet::new(
            vec![
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(0.0, 1.0, 1.0),
            ],
            vec![[0, 1, 2]],
        ))
        .into_ref();
        let scene = Scene::from_shapes(vec![Shape::new(
            Geometry::from(Transformed::new(
                Transform::Tapered {
                    base_radius: 1.0,
                    top_radius: 0.0,
                },
                bar,
            ))
            .into_ref(),
        )]);
        let files = to_obj(&scene).unwrap();
        // The z = 1 vertices collapse onto the axis; the z = 0 one does not.
        assert!(files.obj.contains("v 1.000000 0.000000 0.000000"));
        assert!(files.obj.contains("v 0.000000 0.000000 1.000000"));
    }

    #[test]
    fn groups_and_materials_are_written_and_read_back() {
        let material = Arc::new(Appearance::Material(Material {
            name: Some("leaf".into()),
            ambient: Color3::new(10, 200, 30),
            ..Material::default()
        }));
        let scene = Scene::from_shapes(vec![Shape::new(Geometry::from(square()).into_ref())
            .with_appearance(material)
            .with_id(7)]);

        let files = to_obj(&scene).unwrap();
        assert!(files.obj.starts_with("mtllib scene.mtl\n"));
        assert!(files.obj.contains("g shape_7\n"));
        assert!(files.obj.contains("usemtl leaf\n"));
        assert!(files.mtl.contains("newmtl leaf\n"));
        // Kd is ambient * diffuse (2.0), saturating: (20, 255, 60)/255.
        assert!(files.mtl.contains("Kd 0.078431 1.000000 0.235294\n"));

        let restored = from_obj(&files.obj).unwrap();
        assert_eq!(restored.shapes[0].id, 7);
        assert_eq!(restored.shapes[0].appearance_name(), Some("leaf"));
    }

    #[test]
    fn faces_carry_normal_indices() {
        let files = to_obj(&square_scene()).unwrap();
        assert!(files.obj.contains("vn 0.000000 0.000000 1.000000"));
        assert!(files.obj.contains("f 1//1 2//2 3//3"));
    }

    #[test]
    fn omitting_normals_writes_bare_face_indices() {
        let options = ObjOptions {
            write_normals: false,
            mtllib: None,
            ..ObjOptions::default()
        };
        let files = to_obj_with(&square_scene(), &options).unwrap();
        assert!(!files.obj.contains("vn "));
        assert!(!files.obj.contains("mtllib"));
        assert!(files.obj.contains("f 1 2 3"));
    }

    #[test]
    fn quads_survive_the_round_trip_as_quads() {
        let quads = crate::scenegraph::mesh::QuadSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2, 3]],
        );
        let scene = Scene::from_shapes(vec![Shape::new(Geometry::QuadSet(quads).into_ref())]);
        let restored = from_obj(&to_obj(&scene).unwrap().obj).unwrap();
        assert!(matches!(
            restored.shapes[0].geometry.as_ref(),
            Geometry::QuadSet(_)
        ));
    }

    #[test]
    fn mixed_arity_faces_come_back_as_a_face_set() {
        let obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv 2 0 0\nf 1 2 3 4\nf 1 2 5\n";
        let restored = from_obj(obj).unwrap();
        assert!(matches!(
            restored.shapes[0].geometry.as_ref(),
            Geometry::FaceSet(_)
        ));
    }

    #[test]
    fn negative_face_indices_are_relative_to_the_end() {
        let obj = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n";
        let restored = from_obj(obj).unwrap();
        let Geometry::TriangleSet(mesh) = restored.shapes[0].geometry.as_ref() else {
            panic!("expected a TriangleSet");
        };
        assert_eq!(mesh.indices, vec![[0, 1, 2]]);
    }

    #[test]
    fn comments_and_unknown_keywords_are_ignored() {
        let obj = "# a comment\ns off\nv 0 0 0 # trailing\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let restored = from_obj(obj).unwrap();
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn a_malformed_vertex_is_an_error() {
        assert!(matches!(from_obj("v 0 0\n"), Err(Error::Codec(_))));
        assert!(matches!(
            from_obj("v 0 0 0\nv 1 0 0\nf 1 2\n"),
            Err(Error::Codec(_))
        ));
    }

    #[test]
    fn point_sets_and_polylines_are_skipped_rather_than_failing() {
        let scene = Scene::from_shapes(vec![
            Shape::new(
                Geometry::from(crate::scenegraph::mesh::Polyline::new(vec![
                    Point3::origin(),
                    Point3::new(1.0, 0.0, 0.0),
                ]))
                .into_ref(),
            ),
            Shape::new(Geometry::from(square()).into_ref()),
        ]);
        let files = to_obj(&scene).unwrap();
        assert_eq!(files.obj.matches("\ng ").count() + 1, 1 + files.obj.matches("g shape_1").count());
        assert_eq!(from_obj(&files.obj).unwrap().len(), 1);
    }

    #[test]
    fn an_unported_primitive_is_an_error() {
        let scene = Scene::from_shapes(vec![Shape::new(
            Geometry::Sphere(crate::scenegraph::primitive::Sphere::default()).into_ref(),
        )]);
        assert!(matches!(to_obj(&scene), Err(Error::Unsupported(_))));
    }
}
