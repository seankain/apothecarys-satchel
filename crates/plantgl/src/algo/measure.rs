//! Surface area and volume.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/base/{surfcomputer,volcomputer}.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! These two numbers are the reason the port is worth doing for the game as
//! well as for the renderer: leaf area is a principled harvest-yield driver
//! that ties the reward to the phenotype the player can actually see, rather
//! than to a genotype scalar they cannot.
//!
//! # Volume diverges from upstream, deliberately
//!
//! Upstream's `VolComputer` sums `|dot(c - v0, cross(v1 - v0, v2 - v0))| / 6`
//! over the faces, where `c` is the mean of the vertices. Taking the absolute
//! value per face throws away the sign that makes the sum telescope, so it is
//! right only for solids that are star-shaped about that mean — a torus, a
//! crescent-sectioned stem or any concave organ comes out too large, with no
//! indication that anything went wrong.
//!
//! [`volume`] instead sums the **signed** tetrahedron volumes about the origin,
//! which is the divergence theorem and is exact for any closed orientable
//! mesh, and reports the total as positive regardless of winding. It is also
//! translation-invariant, which the acceptance test pins down. On the solids
//! where upstream is right the two agree; where they differ, upstream is wrong.
//!
//! Upstream also returns `0` for a mesh whose `Solid` flag is false. Zero is a
//! legitimate volume, so a caller cannot tell that apart from a genuinely flat
//! solid; [`volume`] returns [`Error::Unsupported`] instead.

use crate::error::{Error, Result};
use crate::math::{Point3, Real, Vec3};
use crate::scenegraph::geometry::Geometry;
use crate::scenegraph::mesh::{FaceIndex, IndexedMesh};

use super::discretize::{Discretizer, Explicit};

/// The area of one triangle — upstream's `surface(v0, v1, v2)`.
#[inline]
pub fn triangle_area(a: Point3, b: Point3, c: Point3) -> Real {
    (b - a).cross(&(c - a)).norm() / 2.0
}

/// Six times the signed volume of the tetrahedron the triangle spans with the
/// origin.
#[inline]
fn signed_tetra_volume_6(a: Point3, b: Point3, c: Point3) -> Real {
    a.coords.dot(&b.coords.cross(&c.coords))
}

/// Upstream's `SurfComputer` — the sum of the face areas.
///
/// Faces with more than three corners are fan-triangulated from corner 0, as
/// upstream does. A non-planar face therefore measures as the fan does, which
/// is the same convention [`super::tessellate`] renders it with, so the area
/// reported and the area drawn agree.
pub fn surface_area(model: &Explicit) -> Result<Real> {
    match model {
        Explicit::TriangleSet(m) => mesh_surface(m),
        Explicit::QuadSet(m) => mesh_surface(m),
        Explicit::FaceSet(m) => mesh_surface(m),
        // A point set or a polyline has no area, which is a fact rather than
        // an error — upstream reports 0 for both.
        Explicit::PointSet(_) | Explicit::Polyline(_) => Ok(0.0),
    }
}

fn mesh_surface<I: FaceIndex>(mesh: &IndexedMesh<I>) -> Result<Real> {
    let mut total = 0.0;
    for i in 0..mesh.face_count() {
        let size = mesh.face_size(i).unwrap_or(0);
        if size < 3 {
            continue;
        }
        let corner = |j: usize| {
            mesh.face_point_at(i, j)
                .ok_or_else(|| Error::invalid_index(format!("face {i} corner {j}")))
        };
        let origin = corner(0)?;
        for j in 1..size - 1 {
            total += triangle_area(origin, corner(j)?, corner(j + 1)?);
        }
    }
    Ok(total)
}

/// Upstream's `VolComputer`, corrected — the enclosed volume of a solid mesh.
///
/// Returns [`Error::Unsupported`] for a mesh that does not claim to bound a
/// volume; see the module docs.
pub fn volume(model: &Explicit) -> Result<Real> {
    if !model.is_solid() {
        return Err(Error::unsupported(
            "volume is only defined for a mesh whose `solid` flag is set",
        ));
    }
    match model {
        Explicit::TriangleSet(m) => mesh_volume(m),
        Explicit::QuadSet(m) => mesh_volume(m),
        Explicit::FaceSet(m) => mesh_volume(m),
        Explicit::PointSet(_) | Explicit::Polyline(_) => Err(Error::unsupported(
            "a point set or polyline encloses no volume",
        )),
    }
}

fn mesh_volume<I: FaceIndex>(mesh: &IndexedMesh<I>) -> Result<Real> {
    let mut total = 0.0;
    for i in 0..mesh.face_count() {
        let size = mesh.face_size(i).unwrap_or(0);
        if size < 3 {
            continue;
        }
        let corner = |j: usize| {
            mesh.face_point_at(i, j)
                .ok_or_else(|| Error::invalid_index(format!("face {i} corner {j}")))
        };
        let origin = corner(0)?;
        for j in 1..size - 1 {
            total += signed_tetra_volume_6(origin, corner(j)?, corner(j + 1)?);
        }
    }
    // The sign follows the winding; the volume does not.
    Ok((total / 6.0).abs())
}

/// The centroid of a mesh's vertices — upstream's `Point3Array::getCenter()`,
/// which its volume computation pivots about.
pub fn centroid(points: &[Point3]) -> Option<Point3> {
    if points.is_empty() {
        return None;
    }
    let sum: Vec3 = points.iter().map(|p| p.coords).sum();
    Some(Point3::from(sum / points.len() as Real))
}

/// Discretises a geometry tree and measures its surface area.
pub fn geometry_surface_area(geometry: &Geometry, discretizer: &mut Discretizer) -> Result<Real> {
    surface_area(&discretizer.discretize(geometry)?)
}

/// Discretises a geometry tree and measures its enclosed volume.
pub fn geometry_volume(geometry: &Geometry, discretizer: &mut Discretizer) -> Result<Real> {
    volume(&discretizer.discretize(geometry)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::discretize::{discretize, discretize_with, DiscretizeCtx};
    use crate::math::Vec3;
    use crate::scenegraph::mesh::{PointSet, TriangleSet};
    use crate::scenegraph::primitive::{Box3, Cylinder, Sphere};
    use crate::scenegraph::transform::{Transform, Transformed};
    use approx::assert_relative_eq;

    fn cube(edge: f32) -> Explicit {
        discretize(&Geometry::from(Box3::cube(edge))).unwrap()
    }

    #[test]
    fn a_triangles_area_is_half_its_parallelogram() {
        assert_relative_eq!(
            triangle_area(
                Point3::origin(),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(0.0, 3.0, 0.0)
            ),
            3.0,
            epsilon = 1e-6
        );
    }

    /// Acceptance (T8.5): `SurfComputer` matches analytic areas within 1%.
    #[test]
    fn a_unit_cube_has_area_six() {
        assert_relative_eq!(surface_area(&cube(1.0)).unwrap(), 6.0, epsilon = 1e-5);
        assert_relative_eq!(surface_area(&cube(2.0)).unwrap(), 24.0, epsilon = 1e-5);
    }

    /// Acceptance (T8.5): `VolComputer` on a translated unit cube returns 1.0
    /// regardless of translation.
    #[test]
    fn a_unit_cubes_volume_is_one_wherever_it_sits() {
        for offset in [0.0, 1.0, -7.0, 1000.0] {
            let tree = Geometry::from(Transformed::new(
                Transform::Translated(Vec3::new(offset, offset * 2.0, -offset)),
                Geometry::from(Box3::cube(1.0)).into_ref(),
            ));
            let mesh = discretize(&tree).unwrap();
            assert_relative_eq!(
                volume(&mesh).unwrap(),
                1.0,
                max_relative = 1e-4,
                epsilon = 1e-4
            );
        }
    }

    #[test]
    fn volume_does_not_depend_on_winding() {
        let ccw = cube(1.0);
        let Explicit::QuadSet(mut cw) = cube(1.0) else {
            panic!("a box discretises to quads");
        };
        for face in &mut cw.indices {
            face.reverse();
        }
        assert_relative_eq!(
            volume(&ccw).unwrap(),
            volume(&Explicit::QuadSet(cw)).unwrap(),
            epsilon = 1e-5
        );
    }

    #[test]
    fn a_cylinders_area_and_volume_converge_on_the_closed_forms() {
        let ctx = DiscretizeCtx {
            slices: 64,
            ..Default::default()
        };
        let mesh = discretize_with(
            &Geometry::from(Cylinder::sized(1.0, 2.0)),
            ctx,
        )
        .unwrap();
        // 2πrh + two caps of πr².
        let expected_area = std::f32::consts::TAU * 2.0 + 2.0 * std::f32::consts::PI;
        assert_relative_eq!(
            surface_area(&mesh).unwrap(),
            expected_area,
            max_relative = 0.01
        );
        assert_relative_eq!(
            volume(&mesh).unwrap(),
            std::f32::consts::PI * 2.0,
            max_relative = 0.01
        );
    }

    #[test]
    fn a_spheres_area_and_volume_converge_on_the_closed_forms() {
        let ctx = DiscretizeCtx {
            slices: 64,
            stacks: 64,
            ..Default::default()
        };
        let mesh = discretize_with(&Geometry::from(Sphere::sized(1.0)), ctx).unwrap();
        assert_relative_eq!(
            surface_area(&mesh).unwrap(),
            4.0 * std::f32::consts::PI,
            max_relative = 0.01
        );
        assert_relative_eq!(
            volume(&mesh).unwrap(),
            4.0 * std::f32::consts::PI / 3.0,
            max_relative = 0.01
        );
    }

    #[test]
    fn volume_of_a_non_solid_mesh_is_an_error_rather_than_a_wrong_number() {
        let open = discretize(&Geometry::from(Cylinder::default().with_solid(false))).unwrap();
        assert!(matches!(volume(&open), Err(Error::Unsupported(_))));
    }

    #[test]
    fn a_point_set_has_no_area_and_no_volume() {
        let points = Explicit::PointSet(PointSet::new(vec![Point3::origin()]));
        assert_relative_eq!(surface_area(&points).unwrap(), 0.0);
        assert!(matches!(volume(&points), Err(Error::Unsupported(_))));
    }

    #[test]
    fn a_degenerate_face_contributes_no_area() {
        let flat = TriangleSet::new(
            vec![
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
            ],
            vec![[0, 1, 2]],
        );
        assert_relative_eq!(
            surface_area(&Explicit::TriangleSet(flat)).unwrap(),
            0.0,
            epsilon = 1e-6
        );
    }

    #[test]
    fn the_centroid_is_the_mean_of_the_points() {
        assert_relative_eq!(
            centroid(&[Point3::origin(), Point3::new(2.0, 4.0, 6.0)]).unwrap(),
            Point3::new(1.0, 2.0, 3.0),
            epsilon = 1e-6
        );
        assert_eq!(centroid(&[]), None);
    }

    /// The case upstream's absolute-value-per-face sum gets wrong: a solid
    /// that is not star-shaped about its own centroid.
    #[test]
    fn volume_is_exact_for_a_solid_upstream_would_overcount() {
        // A hollow square annulus extruded along z: the centroid sits in the
        // hole, so upstream's |tetrahedron| sum counts the hole's volume in
        // rather than cancelling it out.
        let mesh = annulus_prism();
        // Outer 4×4 minus inner 2×2, one unit tall.
        assert_relative_eq!(volume(&mesh).unwrap(), 12.0, epsilon = 1e-4);

        // What upstream would have reported, computed the same way it does.
        let Explicit::FaceSet(faces) = &mesh else {
            unreachable!()
        };
        let centre = centroid(&faces.model.points).unwrap();
        let mut upstream = 0.0;
        for i in 0..faces.face_count() {
            let origin = faces.face_point_at(i, 0).unwrap();
            let to_centre = centre - origin;
            for j in 1..faces.face_size(i).unwrap() - 1 {
                let v1 = faces.face_point_at(i, j).unwrap() - origin;
                let v2 = faces.face_point_at(i, j + 1).unwrap() - origin;
                upstream += to_centre.dot(&v1.cross(&v2)).abs() / 6.0;
            }
        }
        assert!(
            upstream > 13.0,
            "upstream's formula should overcount here, got {upstream}"
        );
    }

    /// A square tube: a 4×4 outer wall and a 2×2 inner wall, from z = 0 to
    /// z = 1, capped top and bottom by four quads each.
    fn annulus_prism() -> Explicit {
        use crate::scenegraph::mesh::FaceSet;

        let outer = [(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)];
        let inner = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        let mut points = Vec::new();
        for z in [0.0, 1.0] {
            for (x, y) in outer {
                points.push(Point3::new(x, y, z));
            }
            for (x, y) in inner {
                points.push(Point3::new(x, y, z));
            }
        }
        // 0..4 outer bottom, 4..8 inner bottom, 8..12 outer top, 12..16 inner top.
        let mut faces: Vec<Vec<u32>> = Vec::new();
        for i in 0..4u32 {
            let j = (i + 1) % 4;
            // Outer wall, outward normals.
            faces.push(vec![i, j, j + 8, i + 8]);
            // Inner wall, normals pointing into the hole.
            faces.push(vec![j + 4, i + 4, i + 12, j + 12]);
            // Bottom ring, facing -z.
            faces.push(vec![i, i + 4, j + 4, j]);
            // Top ring, facing +z.
            faces.push(vec![i + 8, j + 8, j + 12, i + 12]);
        }
        let mut mesh = FaceSet::new(points, faces);
        mesh.model.solid = true;
        Explicit::FaceSet(mesh)
    }
}
