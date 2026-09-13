//! Transformations applied to a child geometry.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/transformation/{transformed,
//! orthotransformed,deformed,translated,scaled,axisrotated,eulerrotated,
//! oriented,mattransformed,tapered}.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! **The `Tapered` trap.** Upstream derives `Tapered` from `Deformed`, not
//! from `OrthoTransformed`: a taper is a radial scale whose factor depends on
//! the point's own z, so it is not affine and cannot be folded into an
//! accumulated 4×4. That is why [`Transform::to_matrix4`] returns an `Option`
//! and why [`crate::algo::matrix::MatrixComputer`] keeps a separate
//! deformation stack. Silently folding it in produces cylinders where frusta
//! were intended.

use crate::math::{euler_rotation_zyx, orthonormal_basis, Mat4, Point3, Real, Vec3, EPSILON};
use crate::scenegraph::geometry::GeometryRef;

/// A placement or deformation, as upstream's `Transformation` hierarchy.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Transform {
    /// `Translated`.
    Translated(Vec3),
    /// `Scaled` — per-axis, possibly non-uniform.
    Scaled(Vec3),
    /// `AxisRotated`.
    AxisRotated { axis: Vec3, angle: Real },
    /// `EulerRotated` — applied as Z, then Y, then X.
    EulerRotated {
        azimuth: Real,
        elevation: Real,
        roll: Real,
    },
    /// `Oriented` — the basis whose first two columns are `primary` and
    /// `secondary`.
    Oriented { primary: Vec3, secondary: Vec3 },
    /// `MatTransformed` — an arbitrary 4×4.
    Matrix(Mat4),
    /// `Tapered` — **not affine**; see the module docs.
    Tapered { base_radius: Real, top_radius: Real },
}

impl Transform {
    /// Upstream's `getMatrix()`, or `None` for a deformation that cannot be
    /// expressed as one.
    ///
    /// Also `None` when the transform is degenerate — a zero rotation axis, or
    /// an `Oriented` whose two vectors are parallel.
    pub fn to_matrix4(&self) -> Option<Mat4> {
        match self {
            Transform::Translated(t) => Some(Mat4::new_translation(t)),
            Transform::Scaled(s) => Some(Mat4::new_nonuniform_scaling(s)),
            Transform::AxisRotated { axis, angle } => {
                let axis = nalgebra::Unit::try_new(*axis, 1e-10)?;
                Some(nalgebra::Rotation3::from_axis_angle(&axis, *angle).to_homogeneous())
            }
            Transform::EulerRotated {
                azimuth,
                elevation,
                roll,
            } => Some(euler_rotation_zyx(*azimuth, *elevation, *roll).to_homogeneous()),
            Transform::Oriented { primary, secondary } => {
                Some(orthonormal_basis(primary, secondary)?.to_homogeneous())
            }
            Transform::Matrix(m) => Some(*m),
            Transform::Tapered { .. } => None,
        }
    }

    /// The deformation this transform represents, if it is one.
    pub fn to_deformation(&self) -> Option<Deformation> {
        match *self {
            Transform::Tapered {
                base_radius,
                top_radius,
            } => Some(Deformation::Taper(Taper::new(base_radius, top_radius))),
            _ => None,
        }
    }

    /// Whether this transform is a non-affine deformation.
    pub fn is_deformation(&self) -> bool {
        matches!(self, Transform::Tapered { .. })
    }
}

/// A transform applied to a child geometry, as upstream's `Transformed`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transformed {
    pub transform: Transform,
    pub child: GeometryRef,
}

impl Transformed {
    pub fn new(transform: Transform, child: GeometryRef) -> Self {
        Self { transform, child }
    }
}

/// Upstream's `Deformation` hierarchy — transforms that act on sampled points
/// rather than on a matrix. Only `Taper` is in scope for this port.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Deformation {
    Taper(Taper),
}

impl Deformation {
    /// Applies the deformation to a point set, in place of a matrix multiply.
    pub fn transform(&self, points: &[Point3]) -> Vec<Point3> {
        match self {
            Deformation::Taper(taper) => taper.transform(points),
        }
    }
}

/// Upstream's `Taper`: a radial scale that runs from `base_radius` at the
/// point set's minimum z to `top_radius` at its maximum z.
///
/// Upstream stores `base_radius` and `delta_radius = base - top`; the same
/// split is kept so the arithmetic matches term for term.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Taper {
    pub base_radius: Real,
    pub delta_radius: Real,
}

impl Taper {
    /// `Tapered::DEFAULT_BASE_RADIUS`.
    pub const DEFAULT_BASE_RADIUS: Real = 1.0;
    /// `Tapered::DEFAULT_TOP_RADIUS`.
    pub const DEFAULT_TOP_RADIUS: Real = 0.5;

    pub fn new(base_radius: Real, top_radius: Real) -> Self {
        Self {
            base_radius,
            delta_radius: base_radius - top_radius,
        }
    }

    pub fn top_radius(&self) -> Real {
        self.base_radius - self.delta_radius
    }

    /// `Taper::transform(const Point3ArrayPtr&)`.
    ///
    /// The z extent is taken from the points themselves, so the same taper
    /// gives different results on different geometry — that is upstream's
    /// behaviour, not an accident. A point set with no z extent is returned
    /// unchanged, as upstream does.
    pub fn transform(&self, points: &[Point3]) -> Vec<Point3> {
        if points.is_empty() {
            return Vec::new();
        }
        let z_min = points.iter().fold(Real::INFINITY, |acc, p| acc.min(p.z));
        let z_max = points.iter().fold(Real::NEG_INFINITY, |acc, p| acc.max(p.z));
        let delta_z = z_max - z_min;
        if delta_z <= EPSILON {
            return points.to_vec();
        }
        points
            .iter()
            .map(|p| {
                let factor = self.base_radius - self.delta_radius * ((p.z - z_min) / delta_z);
                Point3::new(p.x * factor, p.y * factor, p.z)
            })
            .collect()
    }

    /// `isValid()` — both radii must be non-negative.
    pub fn is_valid(&self) -> bool {
        self.base_radius >= 0.0 && self.top_radius() >= 0.0
    }
}

impl Default for Taper {
    fn default() -> Self {
        Taper::new(Self::DEFAULT_BASE_RADIUS, Self::DEFAULT_TOP_RADIUS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use std::f32::consts::FRAC_PI_2;

    fn apply(t: &Transform, p: Point3) -> Point3 {
        Point3::from_homogeneous(t.to_matrix4().unwrap() * p.to_homogeneous()).unwrap()
    }

    #[test]
    fn translation_moves_a_point() {
        let t = Transform::Translated(Vec3::new(1.0, 2.0, 3.0));
        assert_relative_eq!(apply(&t, Point3::origin()), Point3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn scaling_is_per_axis() {
        let t = Transform::Scaled(Vec3::new(2.0, 3.0, 4.0));
        assert_relative_eq!(
            apply(&t, Point3::new(1.0, 1.0, 1.0)),
            Point3::new(2.0, 3.0, 4.0)
        );
    }

    #[test]
    fn axis_rotation_turns_about_the_axis() {
        let t = Transform::AxisRotated {
            axis: Vec3::z(),
            angle: FRAC_PI_2,
        };
        assert_relative_eq!(
            apply(&t, Point3::new(1.0, 0.0, 0.0)),
            Point3::new(0.0, 1.0, 0.0),
            epsilon = 1e-6
        );
    }

    #[test]
    fn a_degenerate_axis_has_no_matrix() {
        let t = Transform::AxisRotated {
            axis: Vec3::zeros(),
            angle: 1.0,
        };
        assert!(t.to_matrix4().is_none());
    }

    #[test]
    fn oriented_builds_the_basis_from_its_two_vectors() {
        let t = Transform::Oriented {
            primary: Vec3::z(),
            secondary: Vec3::x(),
        };
        let m = t.to_matrix4().unwrap();
        assert_relative_eq!(
            m.fixed_view::<3, 1>(0, 0).into_owned(),
            Vec3::z(),
            epsilon = 1e-6
        );
        assert_relative_eq!(
            m.fixed_view::<3, 1>(0, 1).into_owned(),
            Vec3::x(),
            epsilon = 1e-6
        );
    }

    #[test]
    fn oriented_rejects_parallel_vectors() {
        let t = Transform::Oriented {
            primary: Vec3::z(),
            secondary: Vec3::new(0.0, 0.0, -2.0),
        };
        assert!(t.to_matrix4().is_none());
    }

    #[test]
    fn tapered_is_a_deformation_not_a_matrix() {
        let t = Transform::Tapered {
            base_radius: 1.0,
            top_radius: 0.5,
        };
        assert!(t.to_matrix4().is_none());
        assert!(t.is_deformation());
        assert_eq!(
            t.to_deformation(),
            Some(Deformation::Taper(Taper::new(1.0, 0.5)))
        );
    }

    #[test]
    fn every_affine_transform_reports_no_deformation() {
        for t in [
            Transform::Translated(Vec3::x()),
            Transform::Scaled(Vec3::new(1.0, 1.0, 1.0)),
            Transform::AxisRotated {
                axis: Vec3::z(),
                angle: 0.3,
            },
            Transform::EulerRotated {
                azimuth: 0.1,
                elevation: 0.2,
                roll: 0.3,
            },
            Transform::Oriented {
                primary: Vec3::x(),
                secondary: Vec3::y(),
            },
            Transform::Matrix(Mat4::identity()),
        ] {
            assert!(!t.is_deformation(), "{t:?} should not be a deformation");
            assert!(t.to_deformation().is_none());
            assert!(t.to_matrix4().is_some());
        }
    }

    #[test]
    fn taper_scales_radially_between_the_z_extremes() {
        let taper = Taper::new(1.0, 0.5);
        let points = [
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 1.0),
            Point3::new(1.0, 0.0, 2.0),
        ];
        let out = taper.transform(&points);
        assert_relative_eq!(out[0], Point3::new(1.0, 0.0, 0.0), epsilon = 1e-6);
        assert_relative_eq!(out[1], Point3::new(0.75, 0.0, 1.0), epsilon = 1e-6);
        assert_relative_eq!(out[2], Point3::new(0.5, 0.0, 2.0), epsilon = 1e-6);
    }

    #[test]
    fn taper_leaves_a_flat_point_set_alone() {
        let taper = Taper::new(1.0, 0.5);
        let points = [Point3::new(1.0, 0.0, 0.0), Point3::new(0.0, 1.0, 0.0)];
        assert_eq!(taper.transform(&points), points.to_vec());
    }

    #[test]
    fn taper_round_trips_its_radii() {
        let taper = Taper::new(2.0, 0.25);
        assert_relative_eq!(taper.base_radius, 2.0);
        assert_relative_eq!(taper.top_radius(), 0.25);
        assert!(taper.is_valid());
        assert!(!Taper::new(1.0, -0.5).is_valid());
    }
}
