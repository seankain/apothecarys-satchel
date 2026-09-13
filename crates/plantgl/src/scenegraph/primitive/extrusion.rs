//! The generalized cylinder.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/extrusion.{h,cpp}`
//! and `geometry/profile.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.

use crate::error::{Error, Result};
use crate::math::{Real, Vec2};
use crate::scenegraph::curve::{Curve2DRef, Curve3DRef, ParametricCurve};
use crate::scenegraph::function::QuantisedFunction;

/// Upstream's `Extrusion` — a 2D cross-section swept along a 3D axis.
///
/// This is what makes a stem a stem rather than a stack of cans: one mesh that
/// follows a curve, with a cross-section that can narrow and twist as it goes.
///
/// # Fields
///
/// `scale`, `orientation` and `knot_list` are upstream's
/// `ProfileTransformation`, inlined. Both lists are read at the parameters in
/// `knot_list` — or evenly across `[0, 1]` when it is `None` — and interpolated
/// linearly between them: `scale[i]` is the cross-section's `(x, y)` scale at
/// knot `i` and `orientation[i]` its twist in radians about the axis. An empty
/// list means "no transformation of that kind".
///
/// Upstream's `InitialNormal` field is not carried. Its purpose is to fix the
/// phase of the swept frame, and `orientation` already says that in the units a
/// caller thinks in: a constant `orientation` rotates the whole sweep. The
/// starting frame is derived from the axis, which is upstream's own behaviour
/// whenever `InitialNormal` is left at its default of the zero vector.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Extrusion {
    /// The curve the cross-section is swept along.
    pub axis: Curve3DRef,
    /// The 2D profile, closed for a solid stem and open for a blade.
    pub cross_section: Curve2DRef,
    /// Per-knot cross-section scale. Empty for none.
    pub scale: Vec<Vec2>,
    /// Per-knot twist in radians. Empty for none.
    pub orientation: Vec<Real>,
    /// The parameters `scale` and `orientation` are given at. `None` spreads
    /// them evenly over `[0, 1]`.
    pub knot_list: Option<Vec<Real>>,
    /// `Solid` — whether the two ends are capped.
    pub solid: bool,
    /// `CCW` — the winding of the swept quads.
    pub ccw: bool,
}

impl Extrusion {
    /// `Extrusion::DEFAULT_SOLID` — `Mesh::DEFAULT_SOLID`.
    pub const DEFAULT_SOLID: bool = false;
    /// `Extrusion::DEFAULT_CCW` — `Mesh::DEFAULT_CCW`.
    pub const DEFAULT_CCW: bool = true;

    /// A sweep with no scaling and no twist.
    pub fn new(axis: Curve3DRef, cross_section: Curve2DRef) -> Self {
        Self {
            axis,
            cross_section,
            scale: Vec::new(),
            orientation: Vec::new(),
            knot_list: None,
            solid: Self::DEFAULT_SOLID,
            ccw: Self::DEFAULT_CCW,
        }
    }

    /// Sets the per-knot scale.
    pub fn with_scale(mut self, scale: Vec<Vec2>) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the per-knot twist, in radians.
    pub fn with_orientation(mut self, orientation: Vec<Real>) -> Self {
        self.orientation = orientation;
        self
    }

    /// Sets the parameters the scale and orientation lists are given at.
    pub fn with_knots(mut self, knots: Vec<Real>) -> Self {
        self.knot_list = Some(knots);
        self
    }

    /// `Solid` — caps both ends, which is what makes the sweep enclose a
    /// volume and therefore have one worth measuring.
    pub fn with_solid(mut self, solid: bool) -> Self {
        self.solid = solid;
        self
    }

    pub fn with_ccw(mut self, ccw: bool) -> Self {
        self.ccw = ccw;
        self
    }

    /// A sweep whose cross-section is scaled by a radius profile: `samples`
    /// evenly spaced readings of `radius` become the `scale` list.
    ///
    /// This is what a tapering stem is. A [`QuantisedFunction::ramp`] from 1.0
    /// to 0.0 over a circular cross-section gives a cone, which is the check
    /// `tests/analytic.rs` makes on the whole path.
    pub fn with_radius_profile(
        axis: Curve3DRef,
        cross_section: Curve2DRef,
        radius: &QuantisedFunction,
        samples: usize,
    ) -> Self {
        let samples = samples.max(2);
        let first = radius.first_x();
        let extent = radius.last_x() - first;
        let scale = (0..samples)
            .map(|i| {
                let r = radius.value(first + extent * i as Real / (samples - 1) as Real);
                Vec2::new(r, r)
            })
            .collect();
        Self::new(axis, cross_section).with_scale(scale)
    }

    /// `ProfileTransformation::getUMin()`.
    pub fn u_min(&self) -> Real {
        match &self.knot_list {
            Some(knots) if !knots.is_empty() => knots[0],
            _ => 0.0,
        }
    }

    /// `ProfileTransformation::getUMax()`.
    pub fn u_max(&self) -> Real {
        match &self.knot_list {
            Some(knots) if !knots.is_empty() => knots[knots.len() - 1],
            _ => 1.0,
        }
    }

    /// Whether any cross-section transformation is in play at all.
    pub fn has_profile_transformation(&self) -> bool {
        !self.scale.is_empty() || !self.orientation.is_empty()
    }

    /// `ProfileTransformation::operator()(u)` — the scale and twist to apply to
    /// the cross-section at profile parameter `u`.
    ///
    /// A single entry applies everywhere; several are interpolated linearly
    /// between their knots and held constant outside the knot range.
    pub fn profile_at(&self, u: Real) -> (Vec2, Real) {
        let scale = interpolate_profile(&self.scale, self.knot_list.as_deref(), u, lerp_vec2)
            .unwrap_or_else(|| Vec2::new(1.0, 1.0));
        let orientation =
            interpolate_profile(&self.orientation, self.knot_list.as_deref(), u, lerp_real)
                .unwrap_or(0.0);
        (scale, orientation)
    }

    /// `isValid()`: both curves must be valid, and the lists must line up with
    /// the knots they are indexed by.
    ///
    /// `samples` is the fallback discretisation density, as everywhere else.
    pub fn is_valid(&self, samples: u32) -> Result<()> {
        self.axis.is_valid()?;
        self.cross_section.is_valid()?;
        if self.axis.length(samples)? <= crate::math::EPSILON {
            return Err(Error::degenerate(
                "an extrusion axis of zero length sweeps nothing",
            ));
        }
        if let Some(knots) = &self.knot_list {
            if knots.len() < 2 {
                return Err(Error::BadKnotVector(
                    "an extrusion profile needs at least 2 knots".to_string(),
                ));
            }
            if knots.windows(2).any(|w| w[1] < w[0]) {
                return Err(Error::BadKnotVector(
                    "extrusion profile knots must be non-decreasing".to_string(),
                ));
            }
            for (what, len) in [("scale", self.scale.len()), ("orientation", self.orientation.len())]
            {
                if len > 1 && len != knots.len() {
                    return Err(Error::invalid_index(format!(
                        "the extrusion has {len} {what} entries for {} knots",
                        knots.len()
                    )));
                }
            }
        }
        Ok(())
    }
}

/// One entry of a profile list at parameter `u`.
///
/// Returns `None` for an empty list, which is upstream's "no transformation".
fn interpolate_profile<T: Copy, F: Fn(T, T, Real) -> T>(
    values: &[T],
    knots: Option<&[Real]>,
    u: Real,
    lerp: F,
) -> Option<T> {
    match values.len() {
        0 => return None,
        1 => return Some(values[0]),
        _ => {}
    }

    // Without an explicit knot list the entries are spread evenly over [0, 1],
    // which is upstream's `else` branch — with its indexing bug fixed: it
    // computes `_i = (int)(u / _interval)` for the orientation list where the
    // scale list, three lines up, has the correct `_i = (int)(u * _interval)`.
    // The orientation form divides by the entry count instead of multiplying,
    // so every u below 1 lands in the first interval and the whole list past
    // the second entry is unreachable.
    let (index, t) = match knots {
        Some(knots) if knots.len() == values.len() => {
            let last = knots.len() - 1;
            if u <= knots[0] {
                return Some(values[0]);
            }
            if u >= knots[last] {
                return Some(values[last]);
            }
            let i = knots.iter().position(|k| *k >= u).unwrap_or(last);
            if i == 0 || (knots[i] - u).abs() < crate::math::EPSILON {
                return Some(values[i]);
            }
            let span = knots[i] - knots[i - 1];
            let t = if span > 0.0 { (u - knots[i - 1]) / span } else { 0.0 };
            (i - 1, t)
        }
        _ => {
            let intervals = (values.len() - 1) as Real;
            let scaled = (u.clamp(0.0, 1.0)) * intervals;
            let floor = scaled.floor();
            let i = (floor as usize).min(values.len() - 2);
            (i, scaled - i as Real)
        }
    };
    Some(lerp(values[index], values[index + 1], t))
}

fn lerp_vec2(a: Vec2, b: Vec2, t: Real) -> Vec2 {
    a * (1.0 - t) + b * t
}

fn lerp_real(a: Real, b: Real, t: Real) -> Real {
    a * (1.0 - t) + b * t
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{Point2, Point3};
    use crate::scenegraph::curve::{Curve2D, Curve3D, Polyline2D};
    use crate::scenegraph::mesh::Polyline;
    use approx::assert_relative_eq;

    fn straight_axis() -> Curve3DRef {
        Curve3D::from(Polyline::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.0, 0.0, 1.0),
            Point3::new(0.0, 0.0, 2.0),
        ]))
        .into_ref()
    }

    fn circle() -> Curve2DRef {
        Curve2D::from(Polyline2D::circle(1.0, 8)).into_ref()
    }

    #[test]
    fn an_extrusion_without_a_profile_scales_by_one() {
        let extrusion = Extrusion::new(straight_axis(), circle());
        assert!(!extrusion.has_profile_transformation());
        let (scale, twist) = extrusion.profile_at(0.5);
        assert_relative_eq!(scale, Vec2::new(1.0, 1.0));
        assert_relative_eq!(twist, 0.0);
    }

    #[test]
    fn a_single_profile_entry_applies_everywhere() {
        let extrusion = Extrusion::new(straight_axis(), circle())
            .with_scale(vec![Vec2::new(2.0, 3.0)])
            .with_orientation(vec![0.5]);
        for u in [0.0, 0.3, 1.0] {
            let (scale, twist) = extrusion.profile_at(u);
            assert_relative_eq!(scale, Vec2::new(2.0, 3.0));
            assert_relative_eq!(twist, 0.5);
        }
    }

    #[test]
    fn profile_entries_interpolate_evenly_without_a_knot_list() {
        let extrusion = Extrusion::new(straight_axis(), circle())
            .with_scale(vec![Vec2::new(1.0, 1.0), Vec2::new(0.0, 0.0)])
            .with_orientation(vec![0.0, 1.0, 2.0]);

        assert_relative_eq!(extrusion.profile_at(0.5).0, Vec2::new(0.5, 0.5), epsilon = 1e-6);
        // Three entries over [0, 1]: the middle one lands at u = 0.5, and the
        // list is reachable all the way to its end — which upstream's
        // orientation branch is not.
        assert_relative_eq!(extrusion.profile_at(0.5).1, 1.0, epsilon = 1e-6);
        assert_relative_eq!(extrusion.profile_at(0.75).1, 1.5, epsilon = 1e-6);
        assert_relative_eq!(extrusion.profile_at(1.0).1, 2.0, epsilon = 1e-6);
    }

    #[test]
    fn a_knot_list_places_the_profile_entries() {
        let extrusion = Extrusion::new(straight_axis(), circle())
            .with_scale(vec![
                Vec2::new(1.0, 1.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 0.0),
            ])
            .with_knots(vec![0.0, 0.8, 1.0]);

        assert_relative_eq!(extrusion.u_min(), 0.0);
        assert_relative_eq!(extrusion.u_max(), 1.0);
        // Constant up to 0.8, then a fast taper.
        assert_relative_eq!(extrusion.profile_at(0.4).0, Vec2::new(1.0, 1.0), epsilon = 1e-6);
        assert_relative_eq!(extrusion.profile_at(0.9).0, Vec2::new(0.5, 0.5), epsilon = 1e-6);
        assert_relative_eq!(extrusion.profile_at(1.0).0, Vec2::new(0.0, 0.0), epsilon = 1e-6);
    }

    #[test]
    fn the_profile_is_held_constant_outside_the_knot_range() {
        let extrusion = Extrusion::new(straight_axis(), circle())
            .with_scale(vec![Vec2::new(2.0, 2.0), Vec2::new(1.0, 1.0)])
            .with_knots(vec![0.25, 0.75]);
        assert_relative_eq!(extrusion.profile_at(0.0).0, Vec2::new(2.0, 2.0));
        assert_relative_eq!(extrusion.profile_at(1.0).0, Vec2::new(1.0, 1.0));
    }

    #[test]
    fn a_radius_profile_becomes_the_scale_list() {
        let extrusion = Extrusion::with_radius_profile(
            straight_axis(),
            circle(),
            &QuantisedFunction::ramp(1.0, 0.0),
            5,
        );
        assert_eq!(extrusion.scale.len(), 5);
        assert_relative_eq!(extrusion.scale[0], Vec2::new(1.0, 1.0));
        assert_relative_eq!(extrusion.scale[4], Vec2::new(0.0, 0.0));
        assert_relative_eq!(extrusion.profile_at(0.5).0, Vec2::new(0.5, 0.5), epsilon = 1e-6);
    }

    #[test]
    fn a_mismatched_profile_list_is_an_error() {
        let extrusion = Extrusion::new(straight_axis(), circle())
            .with_scale(vec![Vec2::new(1.0, 1.0), Vec2::new(0.5, 0.5)])
            .with_knots(vec![0.0, 0.5, 1.0]);
        assert!(matches!(extrusion.is_valid(30), Err(Error::InvalidIndex(_))));
    }

    #[test]
    fn a_zero_length_axis_is_degenerate() {
        let axis = Curve3D::from(Polyline::new(vec![Point3::origin(), Point3::origin()])).into_ref();
        let extrusion = Extrusion::new(axis, circle());
        assert!(matches!(
            extrusion.is_valid(30),
            Err(Error::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn a_one_point_cross_section_is_degenerate() {
        let section = Curve2D::from(Polyline2D::new(vec![Point2::new(1.0, 0.0)])).into_ref();
        let extrusion = Extrusion::new(straight_axis(), section);
        assert!(matches!(
            extrusion.is_valid(30),
            Err(Error::DegenerateGeometry(_))
        ));
    }
}
