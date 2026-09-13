//! Axis-aligned bounding boxes and the computer that builds them.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/boundingbox.{h,cpp}`
//! and `src/cpp/plantgl/algo/base/bboxcomputer.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream's `BBoxComputer` reaches parametric primitives through a
//! `Discretizer`, and so does this one now that Phase B (#18) has one. The box
//! of a discretised primitive is the box of its *sampled* points, which is
//! marginally tighter than the box of the ideal surface — a discretised sphere
//! is inscribed in the true one — and is the box the renderer actually draws.

use crate::error::Result;
use crate::math::{Mat4, Point3, Real, Vec3};
use crate::scenegraph::geometry::{Geometry, GeometryRef};
use crate::scenegraph::transform::Deformation;

use super::discretize::Discretizer;
use super::matrix::{MatrixComputer, Placement};

/// An axis-aligned bounding box — upstream's `BoundingBox`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BoundingBox {
    /// `getLowerLeftCorner()`.
    pub lower_left: Point3,
    /// `getUpperRightCorner()`.
    pub upper_right: Point3,
}

impl BoundingBox {
    pub fn new(lower_left: Point3, upper_right: Point3) -> Self {
        Self {
            lower_left,
            upper_right,
        }
    }

    /// The box around a single point.
    pub fn from_point(point: Point3) -> Self {
        Self::new(point, point)
    }

    /// The box around a point set, or `None` if it is empty.
    pub fn from_points(points: &[Point3]) -> Option<Self> {
        let mut iter = points.iter();
        let first = *iter.next()?;
        let mut bbox = Self::from_point(first);
        for point in iter {
            bbox.extend_point(*point);
        }
        Some(bbox)
    }

    /// `extend(const Vector3&)`.
    pub fn extend_point(&mut self, point: Point3) -> &mut Self {
        self.lower_left = Point3::new(
            self.lower_left.x.min(point.x),
            self.lower_left.y.min(point.y),
            self.lower_left.z.min(point.z),
        );
        self.upper_right = Point3::new(
            self.upper_right.x.max(point.x),
            self.upper_right.y.max(point.y),
            self.upper_right.z.max(point.z),
        );
        self
    }

    /// `extend(const BoundingBox&)`.
    pub fn extend(&mut self, other: &BoundingBox) -> &mut Self {
        self.extend_point(other.lower_left);
        self.extend_point(other.upper_right);
        self
    }

    /// The union of two boxes — upstream's `operator|`.
    pub fn union(mut self, other: &BoundingBox) -> Self {
        self.extend(other);
        self
    }

    /// `getCenter()`.
    pub fn center(&self) -> Point3 {
        Point3::from((self.lower_left.coords + self.upper_right.coords) / 2.0)
    }

    /// `getSize()` — the **half**-extent, as upstream. [`BoundingBox::extent`]
    /// is the full one.
    pub fn size(&self) -> Vec3 {
        (self.upper_right - self.lower_left) / 2.0
    }

    /// The full extent along each axis.
    pub fn extent(&self) -> Vec3 {
        self.upper_right - self.lower_left
    }

    /// `getVolume()`.
    pub fn volume(&self) -> Real {
        let e = self.extent();
        e.x * e.y * e.z
    }

    /// `getSurface()` — the total area of the six faces.
    pub fn surface(&self) -> Real {
        let e = self.extent();
        2.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
    }

    /// Whether a point lies inside or on the box.
    pub fn contains(&self, point: Point3) -> bool {
        (self.lower_left.x..=self.upper_right.x).contains(&point.x)
            && (self.lower_left.y..=self.upper_right.y).contains(&point.y)
            && (self.lower_left.z..=self.upper_right.z).contains(&point.z)
    }

    /// `intersect(const BoundingBox&)` — whether the two boxes overlap.
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.lower_left.x <= other.upper_right.x
            && other.lower_left.x <= self.upper_right.x
            && self.lower_left.y <= other.upper_right.y
            && other.lower_left.y <= self.upper_right.y
            && self.lower_left.z <= other.upper_right.z
            && other.lower_left.z <= self.upper_right.z
    }

    /// The box around this box's eight corners after `matrix` is applied.
    ///
    /// Transforming the corners and re-bounding is what upstream does, and
    /// what keeps a rotated box axis-aligned.
    pub fn transformed(&self, matrix: &Mat4) -> Self {
        let (l, u) = (self.lower_left, self.upper_right);
        let corners = [
            Point3::new(l.x, l.y, l.z),
            Point3::new(u.x, l.y, l.z),
            Point3::new(l.x, u.y, l.z),
            Point3::new(u.x, u.y, l.z),
            Point3::new(l.x, l.y, u.z),
            Point3::new(u.x, l.y, u.z),
            Point3::new(l.x, u.y, u.z),
            Point3::new(u.x, u.y, u.z),
        ];
        let transformed: Vec<Point3> = corners
            .iter()
            .map(|c| Point3::from_homogeneous(matrix * c.to_homogeneous()).unwrap_or(*c))
            .collect();
        Self::from_points(&transformed).expect("eight corners")
    }
}

/// Upstream's `BBoxComputer` Action.
#[derive(Debug, Clone, Default)]
pub struct BBoxComputer {
    matrix: MatrixComputer,
    discretizer: Discretizer,
}

impl BBoxComputer {
    pub fn new() -> Self {
        Self {
            matrix: MatrixComputer::new(),
            // Texture coordinates cost work and cannot move a bounding box.
            discretizer: Discretizer::with_defaults().with_tex_coords(false),
        }
    }

    /// A computer that discretises parametric primitives at the given density.
    ///
    /// The density changes the box slightly — a coarser sphere is inscribed in
    /// a finer one — so a culling volume should be built at the density the
    /// renderer will actually draw.
    pub fn with_discretizer(discretizer: Discretizer) -> Self {
        Self {
            matrix: MatrixComputer::new(),
            discretizer,
        }
    }

    /// The bounding box of a whole geometry tree in the tree's own frame, or
    /// `None` when the tree contains no points at all.
    pub fn compute(&mut self, geometry: &GeometryRef) -> Result<Option<BoundingBox>> {
        self.matrix.clear();
        let leaves = self.matrix.flatten(geometry)?;
        let mut result: Option<BoundingBox> = None;
        for (placement, leaf) in leaves {
            let Some(bbox) = self.leaf_bbox(&placement, &leaf)? else {
                continue;
            };
            result = Some(match result {
                Some(acc) => acc.union(&bbox),
                None => bbox,
            });
        }
        Ok(result)
    }

    /// The bounding box of one placed leaf.
    fn leaf_bbox(
        &mut self,
        placement: &Placement,
        leaf: &Geometry,
    ) -> Result<Option<BoundingBox>> {
        // Explicit models are read straight off; a parametric primitive is
        // discretised first, as upstream's computer does.
        let owned;
        let points: &[Point3] = match leaf {
            Geometry::TriangleSet(m) => &m.model.points,
            Geometry::QuadSet(m) => &m.model.points,
            Geometry::FaceSet(m) => &m.model.points,
            Geometry::PointSet(p) => &p.points,
            Geometry::Polyline(p) => &p.points,
            parametric => {
                owned = self.discretizer.discretize(parametric)?;
                owned.points()
            }
        };
        if points.is_empty() {
            return Ok(None);
        }

        // Deformations act on the points, so they must be applied before the
        // box is taken — folding them into a matrix is exactly the mistake
        // this stack exists to prevent.
        let deformed: Vec<Point3> = placement
            .deformations
            .iter()
            .fold(points.to_vec(), |acc, d| match d {
                Deformation::Taper(taper) => taper.transform(&acc),
            });

        let bbox = BoundingBox::from_points(&deformed).expect("non-empty");
        Ok(Some(bbox.transformed(&placement.matrix)))
    }
}

/// The bounding box of a geometry tree.
pub fn bounding_box(geometry: &GeometryRef) -> Result<Option<BoundingBox>> {
    BBoxComputer::new().compute(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::scenegraph::mesh::{Group, Polyline, TriangleSet};
    use crate::scenegraph::primitive::Sphere;
    use crate::scenegraph::transform::{Transform, Transformed};
    use approx::assert_relative_eq;

    fn unit_triangle() -> GeometryRef {
        Geometry::from(TriangleSet::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
        ))
        .into_ref()
    }

    #[test]
    fn box_measures_match_upstream_conventions() {
        let b = BoundingBox::new(Point3::origin(), Point3::new(2.0, 4.0, 6.0));
        assert_relative_eq!(b.center(), Point3::new(1.0, 2.0, 3.0), epsilon = 1e-6);
        // getSize() is the half-extent upstream.
        assert_relative_eq!(b.size(), Vec3::new(1.0, 2.0, 3.0), epsilon = 1e-6);
        assert_relative_eq!(b.extent(), Vec3::new(2.0, 4.0, 6.0), epsilon = 1e-6);
        assert_relative_eq!(b.volume(), 48.0, epsilon = 1e-6);
        assert_relative_eq!(b.surface(), 88.0, epsilon = 1e-6);
    }

    #[test]
    fn extend_grows_in_both_directions() {
        let mut b = BoundingBox::from_point(Point3::origin());
        b.extend_point(Point3::new(1.0, -2.0, 0.0));
        assert_eq!(b.lower_left, Point3::new(0.0, -2.0, 0.0));
        assert_eq!(b.upper_right, Point3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn containment_and_intersection() {
        let a = BoundingBox::new(Point3::origin(), Point3::new(1.0, 1.0, 1.0));
        let b = BoundingBox::new(Point3::new(0.5, 0.5, 0.5), Point3::new(2.0, 2.0, 2.0));
        let c = BoundingBox::new(Point3::new(3.0, 3.0, 3.0), Point3::new(4.0, 4.0, 4.0));
        assert!(a.contains(Point3::new(0.5, 0.5, 0.5)));
        assert!(!a.contains(Point3::new(1.5, 0.5, 0.5)));
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }

    #[test]
    fn transforming_a_rotated_box_stays_axis_aligned() {
        let b = BoundingBox::new(Point3::origin(), Point3::new(1.0, 1.0, 0.0));
        let rotation = Transform::AxisRotated {
            axis: Vec3::z(),
            angle: std::f32::consts::FRAC_PI_4,
        }
        .to_matrix4()
        .unwrap();
        let rotated = b.transformed(&rotation);
        let half = std::f32::consts::SQRT_2 / 2.0;
        assert_relative_eq!(rotated.lower_left.x, -half, epsilon = 1e-5);
        assert_relative_eq!(rotated.upper_right.y, std::f32::consts::SQRT_2, epsilon = 1e-5);
    }

    #[test]
    fn computer_unions_the_leaves_under_their_transforms() {
        let tree = Geometry::Group(Group::new(vec![
            unit_triangle(),
            Geometry::from(Transformed::new(
                Transform::Translated(Vec3::new(3.0, 0.0, 0.0)),
                unit_triangle(),
            ))
            .into_ref(),
        ]))
        .into_ref();

        let bbox = bounding_box(&tree).unwrap().unwrap();
        assert_relative_eq!(bbox.lower_left, Point3::origin(), epsilon = 1e-6);
        assert_relative_eq!(bbox.upper_right, Point3::new(4.0, 1.0, 0.0), epsilon = 1e-6);
    }

    #[test]
    fn computer_applies_a_taper_to_the_points() {
        // A unit square in the xz plane, tapered to nothing at the top.
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

        let bbox = bounding_box(&tree).unwrap().unwrap();
        assert_relative_eq!(bbox.lower_left, Point3::new(0.0, 0.0, 0.0), epsilon = 1e-6);
        assert_relative_eq!(bbox.upper_right, Point3::new(1.0, 0.0, 1.0), epsilon = 1e-6);
    }

    /// Phase B gave the computer a discretizer, so parametric primitives are
    /// in range now.
    #[test]
    fn computer_reaches_a_parametric_primitive() {
        let tree = Geometry::from(Sphere::sized(2.0)).into_ref();
        let bbox = bounding_box(&tree).unwrap().unwrap();
        assert_relative_eq!(bbox.center(), Point3::origin(), epsilon = 1e-4);
        // The discretised sphere is inscribed in the ideal one, so the box is
        // at most 2r on a side and no less than the mid-latitude ring's.
        assert!(bbox.extent().x <= 4.0 + 1e-4, "{:?}", bbox.extent());
        assert!(bbox.extent().z > 3.8, "{:?}", bbox.extent());
    }

    #[test]
    fn computer_reports_a_geometry_it_cannot_discretise() {
        // A `Swung` of degree above 1 needs upstream's `ProfileInterpolation`,
        // which is not ported; the computer propagates that rather than
        // reporting an empty box.
        let profile = crate::scenegraph::curve::Curve2D::from(
            crate::scenegraph::curve::Polyline2D::new(vec![
                crate::math::Point2::new(1.0, 0.0),
                crate::math::Point2::new(1.0, 1.0),
            ]),
        )
        .into_ref();
        let swung = crate::scenegraph::primitive::Swung::new(
            vec![profile.clone(), profile],
            vec![0.0, 1.0],
            8,
            true,
            3,
            0,
        );
        let tree = Geometry::from(swung).into_ref();
        assert!(matches!(bounding_box(&tree), Err(Error::Unsupported(_))));
    }

    #[test]
    fn an_empty_point_list_has_no_box() {
        let tree = Geometry::from(Polyline::new(vec![])).into_ref();
        assert_eq!(bounding_box(&tree).unwrap(), None);
    }
}
