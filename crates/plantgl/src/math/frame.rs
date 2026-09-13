//! The turtle/sweep frame and rotation-minimising frame propagation.
//!
//! The `Frame` type and its matrix conventions are ported from PlantGL
//! `src/cpp/plantgl/algo/modelling/turtleparam.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189, whose `TurtleParam` carries the
//! same `position`/`heading`/`left`/`up` quadruple and builds its orientation
//! as `Matrix3(heading, left, up)`.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! [`Frame::propagate`] is *not* from upstream. It is the double-reflection
//! method of Wang, Jüttler, Zheng and Liu, "Computation of Rotation Minimizing
//! Frames", ACM TOG 27(1), 2008 — needed because the naive Frenet frame flips
//! at inflection points and twists a swept mesh.

use crate::math::{Mat3, Mat4, Point3, Real, Vec3, TOLERANCE};

/// A right-handed orthonormal frame: `up == heading × left`.
///
/// Upstream's default turtle frame is `heading = +Z`, `left = -Y`, `up = +X`,
/// which [`Frame::default`] reproduces.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frame {
    pub position: Point3,
    pub heading: Vec3,
    pub left: Vec3,
    pub up: Vec3,
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            position: Point3::origin(),
            heading: Vec3::new(0.0, 0.0, 1.0),
            left: Vec3::new(0.0, -1.0, 0.0),
            up: Vec3::new(1.0, 0.0, 0.0),
        }
    }
}

impl Frame {
    /// Builds a frame from vectors that are already orthonormal.
    ///
    /// No check is performed; use [`Frame::from_heading_up`] or
    /// [`Frame::orthonormalized`] when the inputs may be skewed.
    pub fn new(position: Point3, heading: Vec3, left: Vec3, up: Vec3) -> Self {
        Self {
            position,
            heading,
            left,
            up,
        }
    }

    /// Builds a frame from a heading and an approximate up vector, projecting
    /// `up` onto the plane normal to `heading`.
    ///
    /// Returns `None` when either vector is degenerate or they are parallel.
    pub fn from_heading_up(position: Point3, heading: Vec3, up: Vec3) -> Option<Self> {
        let heading = heading.try_normalize(TOLERANCE)?;
        let left = up.cross(&heading).try_normalize(TOLERANCE)?;
        Some(Self {
            position,
            heading,
            left,
            up: heading.cross(&left),
        })
    }

    /// Builds a frame from a heading alone, picking an arbitrary but stable
    /// `left` perpendicular to it.
    ///
    /// Returns `None` for a degenerate heading.
    pub fn from_heading(position: Point3, heading: Vec3) -> Option<Self> {
        let heading = heading.try_normalize(TOLERANCE)?;
        // Cross with whichever axis the heading is least aligned to, so the
        // cross product never collapses.
        let reference = if heading.x.abs() < heading.y.abs() && heading.x.abs() < heading.z.abs() {
            Vec3::x()
        } else if heading.y.abs() < heading.z.abs() {
            Vec3::y()
        } else {
            Vec3::z()
        };
        let left = heading.cross(&reference).normalize();
        Some(Self {
            position,
            heading,
            left,
            up: heading.cross(&left),
        })
    }

    /// Gram-Schmidt: normalises `heading`, removes the `heading` component
    /// from `left`, and rebuilds `up` as `heading × left`.
    ///
    /// Returns `false` and leaves the frame untouched if the vectors are too
    /// degenerate to fix.
    pub fn orthonormalize(&mut self) -> bool {
        let Some(heading) = self.heading.try_normalize(TOLERANCE) else {
            return false;
        };
        let projected = self.left - heading * heading.dot(&self.left);
        let Some(left) = projected.try_normalize(TOLERANCE) else {
            return false;
        };
        self.heading = heading;
        self.left = left;
        self.up = heading.cross(&left);
        true
    }

    /// [`Frame::orthonormalize`] as a value transform.
    pub fn orthonormalized(mut self) -> Option<Self> {
        self.orthonormalize().then_some(self)
    }

    /// Whether the three axes are unit length, mutually perpendicular and
    /// right-handed, to within `tolerance`.
    pub fn is_orthonormal(&self, tolerance: Real) -> bool {
        let unit = |v: &Vec3| (v.norm() - 1.0).abs() < tolerance;
        unit(&self.heading)
            && unit(&self.left)
            && unit(&self.up)
            && self.heading.dot(&self.left).abs() < tolerance
            && self.heading.dot(&self.up).abs() < tolerance
            && self.left.dot(&self.up).abs() < tolerance
            && (self.heading.cross(&self.left) - self.up).norm() < tolerance
    }

    /// The orientation as upstream's `TurtleParam::getOrientationMatrix`:
    /// `Matrix3(heading, left, up)`, whose columns are the three axes.
    pub fn orientation(&self) -> Mat3 {
        Mat3::from_columns(&[self.heading, self.left, self.up])
    }

    /// The placement as upstream's `TurtleParam::getTransformationMatrix`
    /// with unit scale.
    pub fn to_matrix4(&self) -> Mat4 {
        self.to_matrix4_scaled(&Vec3::new(1.0, 1.0, 1.0))
    }

    /// The placement with a per-axis scale, as upstream's
    /// `Matrix4(heading*scale.x, left*scale.y, up*scale.z, position)`.
    pub fn to_matrix4_scaled(&self, scale: &Vec3) -> Mat4 {
        let mut m = Mat4::identity();
        m.fixed_view_mut::<3, 1>(0, 0).copy_from(&(self.heading * scale.x));
        m.fixed_view_mut::<3, 1>(0, 1).copy_from(&(self.left * scale.y));
        m.fixed_view_mut::<3, 1>(0, 2).copy_from(&(self.up * scale.z));
        m.fixed_view_mut::<3, 1>(0, 3).copy_from(&self.position.coords);
        m
    }

    /// Propagates this frame to `position` with `heading` by double
    /// reflection, minimising the rotation about the heading.
    ///
    /// Wang et al. 2008, §4.1: reflect the frame in the plane bisecting the
    /// two sample points, then in the plane bisecting the reflected tangent
    /// and the target tangent. The result is a frame whose `left` has no
    /// angular velocity about `heading`.
    ///
    /// Returns `None` for a degenerate heading.
    pub fn propagate(&self, position: Point3, heading: Vec3) -> Option<Self> {
        let target_heading = heading.try_normalize(TOLERANCE)?;

        // First reflection: in the plane bisecting the two positions.
        let v1 = position - self.position;
        let c1 = v1.dot(&v1);
        let (reflected_left, reflected_heading) = if c1 > TOLERANCE {
            (
                self.left - v1 * (2.0 / c1) * v1.dot(&self.left),
                self.heading - v1 * (2.0 / c1) * v1.dot(&self.heading),
            )
        } else {
            // Coincident samples: nothing to reflect through.
            (self.left, self.heading)
        };

        // Second reflection: in the plane bisecting the reflected tangent and
        // the target tangent.
        let v2 = target_heading - reflected_heading;
        let c2 = v2.dot(&v2);
        let left = if c2 > TOLERANCE {
            reflected_left - v2 * (2.0 / c2) * v2.dot(&reflected_left)
        } else {
            reflected_left
        };

        // Re-project and renormalise: f32 double reflection drifts slowly.
        let projected = left - target_heading * target_heading.dot(&left);
        let left = projected.try_normalize(TOLERANCE)?;

        Some(Self {
            position,
            heading: target_heading,
            left,
            up: target_heading.cross(&left),
        })
    }
}

/// Rotation-minimising frames along a sampled curve.
///
/// Headings are forward differences, with the last sample reusing the previous
/// heading. `initial` supplies the starting `left`; its heading is replaced by
/// the curve's first heading, and its `left` is re-projected onto the plane
/// normal to it. Returns `None` if the curve has fewer than two distinct
/// points or the initial frame cannot be fixed up.
pub fn rotation_minimizing_frames(points: &[Point3], initial: &Frame) -> Option<Vec<Frame>> {
    if points.len() < 2 {
        return None;
    }

    let heading_at = |i: usize| -> Vec3 {
        if i + 1 < points.len() {
            points[i + 1] - points[i]
        } else {
            points[i] - points[i - 1]
        }
    };

    let mut frames = Vec::with_capacity(points.len());
    let mut current = Frame {
        position: points[0],
        heading: heading_at(0),
        left: initial.left,
        up: initial.up,
    };
    if !current.orthonormalize() {
        return None;
    }
    frames.push(current);

    for (i, point) in points.iter().enumerate().skip(1) {
        current = current.propagate(*point, heading_at(i))?;
        frames.push(current);
    }

    Some(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use std::f32::consts::PI;

    fn helix(a: Real, b: Real, t: Real) -> Point3 {
        Point3::new(a * t.cos(), a * t.sin(), b * t)
    }

    #[test]
    fn default_frame_matches_upstream_turtle() {
        let f = Frame::default();
        assert_eq!(f.heading, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(f.left, Vec3::new(0.0, -1.0, 0.0));
        assert_eq!(f.up, Vec3::new(1.0, 0.0, 0.0));
        assert!(f.is_orthonormal(1e-6));
    }

    #[test]
    fn from_heading_up_is_right_handed() {
        let f = Frame::from_heading_up(Point3::origin(), Vec3::z(), Vec3::x()).unwrap();
        assert!(f.is_orthonormal(1e-6));
        assert_relative_eq!(f.left, Vec3::new(0.0, -1.0, 0.0), epsilon = 1e-6);
    }

    #[test]
    fn from_heading_picks_a_perpendicular_left() {
        for heading in [Vec3::x(), Vec3::y(), Vec3::z(), Vec3::new(1.0, 1.0, 1.0)] {
            let f = Frame::from_heading(Point3::origin(), heading).unwrap();
            assert!(f.is_orthonormal(1e-6), "not orthonormal for {heading:?}");
        }
        assert!(Frame::from_heading(Point3::origin(), Vec3::zeros()).is_none());
    }

    #[test]
    fn orthonormalize_fixes_a_skewed_frame() {
        let mut f = Frame::new(
            Point3::origin(),
            Vec3::new(0.0, 0.0, 2.0),
            Vec3::new(0.3, -1.0, 0.7),
            Vec3::zeros(),
        );
        assert!(f.orthonormalize());
        assert!(f.is_orthonormal(1e-6));
    }

    #[test]
    fn orthonormalize_rejects_degenerate_input() {
        let mut f = Frame::new(Point3::origin(), Vec3::zeros(), Vec3::y(), Vec3::z());
        assert!(!f.orthonormalize());

        // left parallel to heading leaves nothing to project.
        let mut f = Frame::new(Point3::origin(), Vec3::z(), Vec3::new(0.0, 0.0, 3.0), Vec3::x());
        assert!(!f.orthonormalize());
    }

    #[test]
    fn transformation_matrix_columns_are_the_axes() {
        let f = Frame::from_heading_up(Point3::new(1.0, 2.0, 3.0), Vec3::z(), Vec3::x()).unwrap();
        let m = f.to_matrix4_scaled(&Vec3::new(2.0, 3.0, 4.0));
        assert_relative_eq!(m.fixed_view::<3, 1>(0, 0).into_owned(), f.heading * 2.0);
        assert_relative_eq!(m.fixed_view::<3, 1>(0, 1).into_owned(), f.left * 3.0);
        assert_relative_eq!(m.fixed_view::<3, 1>(0, 2).into_owned(), f.up * 4.0);
        assert_relative_eq!(
            m.fixed_view::<3, 1>(0, 3).into_owned(),
            f.position.coords
        );
        assert_eq!(m.row(3).into_owned(), nalgebra::RowVector4::new(0.0, 0.0, 0.0, 1.0));
    }

    /// Acceptance (T8.1): orthonormality survives 10 000 random rotations.
    ///
    /// Each step rotates the frame and re-orthonormalises it, the way a
    /// turtle applies a turn; the assertion is that the frame is still
    /// orthonormal to 1e-5 after every one of them, so f32 drift never
    /// accumulates into a skewed basis.
    #[test]
    fn orthonormality_survives_ten_thousand_rotations() {
        // A fixed xorshift keeps the test deterministic without a dependency.
        let mut state: u32 = 0x2545_f491;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32) * 2.0 - 1.0
        };

        let mut f = Frame::default();
        for _ in 0..10_000 {
            let axis = Vec3::new(next(), next(), next());
            let Some(axis) = axis.try_normalize(1e-6) else {
                continue;
            };
            let angle = next() * PI;
            let r = nalgebra::Rotation3::from_axis_angle(&nalgebra::Unit::new_unchecked(axis), angle);
            f = Frame::new(f.position, r * f.heading, r * f.left, r * f.up);
            assert!(f.orthonormalize(), "frame collapsed: {f:?}");
            assert!(
                f.is_orthonormal(1e-5),
                "frame drifted out of orthonormality: {f:?}"
            );
        }
    }

    /// Acceptance (T8.1): an RMF over a helix accumulates < 1e-4 twist
    /// against the closed-form rotation-minimising frame.
    ///
    /// For a curve with torsion `tau`, the RMF is the Frenet frame rotated
    /// about the tangent by `-tau * s`. A circular helix has constant
    /// `tau = b / (a^2 + b^2)` and `s = t * sqrt(a^2 + b^2)`, so the expected
    /// angle between our reference vector and the Frenet normal is
    /// `-b * t / sqrt(a^2 + b^2)`.
    #[test]
    fn rmf_over_a_helix_matches_the_closed_form() {
        let (a, b) = (1.0_f32, 0.35_f32);
        let samples = 2000;
        let t_max = 4.0 * PI;
        let points: Vec<Point3> = (0..=samples)
            .map(|i| helix(a, b, t_max * i as f32 / samples as f32))
            .collect();

        // Start aligned with the Frenet normal at t = 0, which is (-1, 0, 0).
        let initial = Frame::new(
            points[0],
            Vec3::new(0.0, a, b).normalize(),
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::zeros(),
        );
        let frames = rotation_minimizing_frames(&points, &initial).unwrap();

        let scale = (a * a + b * b).sqrt();
        let mut worst = 0.0_f32;
        for (i, frame) in frames.iter().enumerate() {
            let t = t_max * i as f32 / samples as f32;
            let normal = Vec3::new(-t.cos(), -t.sin(), 0.0);
            let binormal = frame.heading.cross(&normal);
            // Signed angle from the Frenet normal to our reference vector,
            // measured in the plane normal to the tangent.
            let angle = frame
                .left
                .dot(&binormal)
                .atan2(frame.left.dot(&normal));
            let expected = -b * t / scale;
            let error = (angle - expected).sin().abs();
            worst = worst.max(error);
        }
        assert!(worst < 1e-4, "worst twist error {worst} exceeds 1e-4");
    }

    #[test]
    fn rmf_keeps_every_frame_orthonormal() {
        let points: Vec<Point3> = (0..=200)
            .map(|i| helix(1.0, 0.2, 6.0 * PI * i as f32 / 200.0))
            .collect();
        let frames = rotation_minimizing_frames(&points, &Frame::default()).unwrap();
        assert_eq!(frames.len(), points.len());
        for f in &frames {
            assert!(f.is_orthonormal(1e-5));
        }
    }

    #[test]
    fn rmf_on_a_straight_line_does_not_rotate() {
        let points: Vec<Point3> = (0..10).map(|i| Point3::new(0.0, 0.0, i as f32)).collect();
        let initial = Frame::from_heading_up(points[0], Vec3::z(), Vec3::x()).unwrap();
        let frames = rotation_minimizing_frames(&points, &initial).unwrap();
        for f in &frames {
            assert_relative_eq!(f.left, initial.left, epsilon = 1e-6);
        }
    }

    #[test]
    fn rmf_needs_at_least_two_points() {
        assert!(rotation_minimizing_frames(&[], &Frame::default()).is_none());
        assert!(rotation_minimizing_frames(&[Point3::origin()], &Frame::default()).is_none());
    }
}
