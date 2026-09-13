//! Bounding spheres.
//!
//! Ported from PlantGL
//! `src/cpp/plantgl/scenegraph/geometry/boundingsphere.{h,cpp}` and
//! `src/cpp/plantgl/algo/base/bspherecomputer.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! A sphere beats a box for the thing plants are actually culled and picked
//! against: it is rotation-invariant, so a swaying stem does not need its
//! volume rebuilt every frame.
//!
//! # Fitting diverges from upstream
//!
//! Upstream's `BSphereComputer` grows a sphere point by point with
//! `BoundingSphere::extend(const Vector3&)`, which moves the radius but never
//! the centre — so the result depends on which point happened to be first and
//! can be far larger than necessary. [`BoundingSphere::ritter`] uses Ritter's
//! two-pass method instead, which is barely more work and within about 10% of
//! the true minimum. `extend` is still here, matching upstream exactly, for
//! callers growing a sphere incrementally.

use crate::error::Result;
use crate::math::{Mat4, Point3, Real, Vec3};
use crate::scenegraph::geometry::{Geometry, GeometryRef};

use super::bbox::BoundingBox;
use super::discretize::Discretizer;
use super::matrix::{MatrixComputer, Placement};

/// Upstream's `BoundingSphere`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundingSphere {
    pub center: Point3,
    pub radius: Real,
}

impl BoundingSphere {
    pub fn new(center: Point3, radius: Real) -> Self {
        Self {
            center,
            radius: radius.max(0.0),
        }
    }

    /// The sphere around a single point.
    pub fn from_point(point: Point3) -> Self {
        Self::new(point, 0.0)
    }

    /// `BoundingSphere(point1, point2)` — the sphere on the segment's
    /// diameter.
    pub fn from_points_pair(a: Point3, b: Point3) -> Self {
        Self::new(nalgebra::center(&a, &b), (b - a).norm() / 2.0)
    }

    /// The sphere circumscribing a box.
    pub fn from_bbox(bbox: &BoundingBox) -> Self {
        Self::from_points_pair(bbox.lower_left, bbox.upper_right)
    }

    /// Ritter's bounding sphere: a first pass picks the most separated pair
    /// along the cardinal axes, a second grows the sphere to reach the rest,
    /// moving the centre each time rather than only the radius.
    pub fn ritter(points: &[Point3]) -> Option<Self> {
        let first = *points.first()?;

        // Pass one: the two points furthest apart along any one axis.
        let mut min = [first; 3];
        let mut max = [first; 3];
        for point in points {
            for axis in 0..3 {
                if point[axis] < min[axis][axis] {
                    min[axis] = *point;
                }
                if point[axis] > max[axis][axis] {
                    max[axis] = *point;
                }
            }
        }
        let widest = (0..3)
            .max_by(|a, b| {
                let span = |i: usize| (max[i] - min[i]).norm_squared();
                span(*a)
                    .partial_cmp(&span(*b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .expect("three axes");
        let mut sphere = Self::from_points_pair(min[widest], max[widest]);

        // Pass two: absorb anything still outside, keeping the old sphere
        // enclosed by moving the centre half the overshoot.
        for point in points {
            let offset = point - sphere.center;
            let distance = offset.norm();
            if distance <= sphere.radius {
                continue;
            }
            let new_radius = (sphere.radius + distance) / 2.0;
            let shift = new_radius - sphere.radius;
            sphere.center += offset * (shift / distance);
            sphere.radius = new_radius;
        }
        Some(sphere)
    }

    /// `extend(const Vector3&)` — grows the radius to reach `point`, leaving
    /// the centre alone, exactly as upstream.
    pub fn extend_point(&mut self, point: Point3) -> &mut Self {
        self.radius = self.radius.max((point - self.center).norm());
        self
    }

    /// `extend(const BoundingSphere&)` — upstream's radius-weighted blend of
    /// the two centres, then a radius reaching both.
    pub fn extend(&mut self, other: &BoundingSphere) -> &mut Self {
        let total = self.radius + other.radius;
        let center = if total <= crate::math::EPSILON {
            nalgebra::center(&self.center, &other.center)
        } else {
            Point3::from(
                (self.center.coords * self.radius + other.center.coords * other.radius) / total,
            )
        };
        self.radius = ((self.center - center).norm() + self.radius)
            .max((other.center - center).norm() + other.radius);
        self.center = center;
        self
    }

    /// The union of two spheres.
    pub fn union(mut self, other: &BoundingSphere) -> Self {
        self.extend(other);
        self
    }

    /// Whether a point lies inside or on the sphere.
    pub fn contains(&self, point: Point3) -> bool {
        (point - self.center).norm() <= self.radius + crate::math::EPSILON
    }

    /// Whether two spheres overlap.
    pub fn intersects(&self, other: &BoundingSphere) -> bool {
        (other.center - self.center).norm() <= self.radius + other.radius
    }

    /// `getVolume()`.
    pub fn volume(&self) -> Real {
        4.0 / 3.0 * std::f32::consts::PI * self.radius.powi(3)
    }

    /// `getSurface()`.
    pub fn surface(&self) -> Real {
        4.0 * std::f32::consts::PI * self.radius * self.radius
    }

    /// The sphere around this one's points after `matrix` is applied.
    ///
    /// A non-uniform scale turns a sphere into an ellipsoid, so the radius is
    /// scaled by the largest of the matrix's column norms — the smallest
    /// factor that is guaranteed still to enclose.
    pub fn transformed(&self, matrix: &Mat4) -> Self {
        let center = Point3::from_homogeneous(matrix * self.center.to_homogeneous())
            .unwrap_or(self.center);
        let scale = (0..3)
            .map(|i| matrix.fixed_view::<3, 1>(0, i).norm())
            .fold(0.0 as Real, Real::max);
        Self::new(center, self.radius * scale)
    }

    /// The axis-aligned box around this sphere.
    pub fn bbox(&self) -> BoundingBox {
        let r = Vec3::repeat(self.radius);
        BoundingBox::new(self.center - r, self.center + r)
    }
}

/// Upstream's `BSphereComputer` Action, reaching parametric primitives through
/// a [`Discretizer`] as upstream's does.
#[derive(Debug, Clone, Default)]
pub struct BSphereComputer {
    matrix: MatrixComputer,
    discretizer: Discretizer,
}

impl BSphereComputer {
    pub fn new() -> Self {
        Self {
            matrix: MatrixComputer::new(),
            discretizer: Discretizer::with_defaults().with_tex_coords(false),
        }
    }

    /// A computer that discretises at the given density.
    pub fn with_discretizer(discretizer: Discretizer) -> Self {
        Self {
            matrix: MatrixComputer::new(),
            discretizer,
        }
    }

    /// The bounding sphere of a whole geometry tree, or `None` when the tree
    /// contains no points.
    pub fn compute(&mut self, geometry: &GeometryRef) -> Result<Option<BoundingSphere>> {
        self.matrix.clear();
        let leaves = self.matrix.flatten(geometry)?;
        let mut result: Option<BoundingSphere> = None;
        for (placement, leaf) in leaves {
            let Some(sphere) = self.leaf_sphere(&placement, &leaf)? else {
                continue;
            };
            result = Some(match result {
                Some(acc) => acc.union(&sphere),
                None => sphere,
            });
        }
        Ok(result)
    }

    fn leaf_sphere(
        &mut self,
        placement: &Placement,
        leaf: &Geometry,
    ) -> Result<Option<BoundingSphere>> {
        let model = self.discretizer.discretize(leaf)?;
        let points = model.points();
        if points.is_empty() {
            return Ok(None);
        }
        // Deformations act on points, so they are applied before the fit —
        // the same reason the bbox computer keeps a separate stack.
        let deformed = placement
            .deformations
            .iter()
            .fold(points.to_vec(), |acc, d| d.transform(&acc));
        let sphere = BoundingSphere::ritter(&deformed).expect("non-empty");
        Ok(Some(sphere.transformed(&placement.matrix)))
    }
}

/// The bounding sphere of a geometry tree.
pub fn bounding_sphere(geometry: &GeometryRef) -> Result<Option<BoundingSphere>> {
    BSphereComputer::new().compute(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenegraph::mesh::{Group, Polyline, TriangleSet};
    use crate::scenegraph::primitive::{Box3, Sphere};
    use crate::scenegraph::transform::{Transform, Transformed};
    use approx::assert_relative_eq;

    #[test]
    fn ritter_fits_a_sphere_around_its_points() {
        let points = vec![
            Point3::new(-1.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.0, 1.0, 0.0),
            Point3::new(0.0, -1.0, 0.0),
        ];
        let sphere = BoundingSphere::ritter(&points).unwrap();
        for p in &points {
            assert!(sphere.contains(*p), "{p:?} outside {sphere:?}");
        }
        assert_relative_eq!(sphere.center, Point3::origin(), epsilon = 1e-5);
        assert_relative_eq!(sphere.radius, 1.0, epsilon = 1e-5);
    }

    #[test]
    fn ritter_beats_the_centre_fixed_growth_upstream_uses() {
        // Points strung out along +x: upstream's extend keeps the centre at
        // the first point, so its radius is the whole span.
        let points: Vec<Point3> = (0..10).map(|i| Point3::new(i as f32, 0.0, 0.0)).collect();

        let ritter = BoundingSphere::ritter(&points).unwrap();
        assert_relative_eq!(ritter.radius, 4.5, epsilon = 1e-5);

        let mut upstream = BoundingSphere::from_point(points[0]);
        for p in &points {
            upstream.extend_point(*p);
        }
        assert_relative_eq!(upstream.radius, 9.0, epsilon = 1e-5);
    }

    #[test]
    fn ritter_on_one_point_is_a_zero_radius_sphere() {
        let sphere = BoundingSphere::ritter(&[Point3::new(1.0, 2.0, 3.0)]).unwrap();
        assert_relative_eq!(sphere.radius, 0.0);
        assert_eq!(BoundingSphere::ritter(&[]), None);
    }

    #[test]
    fn extend_blends_the_centres_as_upstream_does() {
        let mut a = BoundingSphere::new(Point3::origin(), 1.0);
        let b = BoundingSphere::new(Point3::new(4.0, 0.0, 0.0), 1.0);
        a.extend(&b);
        assert_relative_eq!(a.center, Point3::new(2.0, 0.0, 0.0), epsilon = 1e-5);
        assert_relative_eq!(a.radius, 3.0, epsilon = 1e-5);
    }

    #[test]
    fn extending_two_zero_radius_spheres_still_reaches_both() {
        let mut a = BoundingSphere::from_point(Point3::origin());
        let b = BoundingSphere::from_point(Point3::new(2.0, 0.0, 0.0));
        a.extend(&b);
        assert!(a.contains(Point3::origin()));
        assert!(a.contains(Point3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn measures_and_predicates() {
        let unit = BoundingSphere::new(Point3::origin(), 1.0);
        assert_relative_eq!(unit.volume(), 4.0 / 3.0 * std::f32::consts::PI, epsilon = 1e-5);
        assert_relative_eq!(unit.surface(), 4.0 * std::f32::consts::PI, epsilon = 1e-5);
        assert!(unit.contains(Point3::new(0.5, 0.5, 0.0)));
        assert!(!unit.contains(Point3::new(2.0, 0.0, 0.0)));
        assert!(unit.intersects(&BoundingSphere::new(Point3::new(1.5, 0.0, 0.0), 1.0)));
        assert!(!unit.intersects(&BoundingSphere::new(Point3::new(5.0, 0.0, 0.0), 1.0)));
        assert_relative_eq!(unit.bbox().extent(), Vec3::repeat(2.0), epsilon = 1e-6);
    }

    #[test]
    fn a_non_uniform_scale_still_encloses() {
        let unit = BoundingSphere::new(Point3::origin(), 1.0);
        let matrix = Transform::Scaled(Vec3::new(1.0, 3.0, 2.0))
            .to_matrix4()
            .unwrap();
        let scaled = unit.transformed(&matrix);
        assert_relative_eq!(scaled.radius, 3.0, epsilon = 1e-5);
        assert!(scaled.contains(Point3::new(0.0, 3.0, 0.0)));
    }

    /// The computer reaches parametric primitives now that Phase B has
    /// discretisation — Phase A could only see explicit meshes.
    #[test]
    fn the_computer_reaches_a_parametric_primitive() {
        let tree = Geometry::from(Sphere::sized(2.0)).into_ref();
        let sphere = bounding_sphere(&tree).unwrap().unwrap();
        assert_relative_eq!(sphere.center, Point3::origin(), epsilon = 1e-4);
        // The discretised sphere is inscribed, so its fit is at most the
        // radius and no less than the mid-latitude ring.
        assert!(sphere.radius <= 2.0 + 1e-4, "{}", sphere.radius);
        assert!(sphere.radius > 1.9, "{}", sphere.radius);
    }

    #[test]
    fn the_computer_unions_leaves_under_their_transforms() {
        let unit = || Geometry::from(Box3::cube(1.0)).into_ref();
        let tree = Geometry::from(Group::new(vec![
            unit(),
            Geometry::from(Transformed::new(
                Transform::Translated(Vec3::new(10.0, 0.0, 0.0)),
                unit(),
            ))
            .into_ref(),
        ]))
        .into_ref();

        let sphere = bounding_sphere(&tree).unwrap().unwrap();
        assert!(sphere.contains(Point3::new(-0.5, -0.5, -0.5)));
        assert!(sphere.contains(Point3::new(10.5, 0.5, 0.5)));
    }

    #[test]
    fn a_taper_is_applied_before_the_fit() {
        let bar = Geometry::from(Polyline::new(vec![
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 1.0),
        ]))
        .into_ref();
        let tree = Geometry::from(Transformed::new(
            Transform::Tapered {
                base_radius: 1.0,
                top_radius: 0.0,
            },
            bar,
        ))
        .into_ref();
        let sphere = bounding_sphere(&tree).unwrap().unwrap();
        assert!(sphere.contains(Point3::new(1.0, 0.0, 0.0)));
        assert!(sphere.contains(Point3::new(0.0, 0.0, 1.0)));
        assert!(sphere.radius < 1.0);
    }

    #[test]
    fn an_empty_point_list_has_no_sphere() {
        let tree = Geometry::from(Polyline::new(vec![])).into_ref();
        assert_eq!(bounding_sphere(&tree).unwrap(), None);
    }

    #[test]
    fn a_sphere_circumscribes_the_box_it_came_from() {
        let mesh = TriangleSet::new(
            vec![
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
        );
        let bbox = super::super::bbox::bounding_box(&Geometry::from(mesh).into_ref())
            .unwrap()
            .unwrap();
        let sphere = BoundingSphere::from_bbox(&bbox);
        assert!(sphere.contains(bbox.lower_left));
        assert!(sphere.contains(bbox.upper_right));
    }
}
