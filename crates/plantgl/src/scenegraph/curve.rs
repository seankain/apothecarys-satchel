//! Curves and patches.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/{curve,polyline,
//! beziercurve,nurbscurve,bezierpatch,nurbspatch}.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Phase B (#18) translates [`Polyline2D`] — the explicit member of upstream's
//! `Curve2D` hierarchy — because [`Revolution`] and [`Swung`] sweep a 2D
//! profile and cannot be discretised without one. Bézier and NURBS evaluation
//! is Phase C (#19); those variants are still placeholders and
//! [`Curve2D::sample`] reports them as [`Error::Unsupported`].
//!
//! [`Revolution`]: crate::scenegraph::primitive::Revolution
//! [`Swung`]: crate::scenegraph::primitive::Swung

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point2, Real};

/// Shared 2D curve handle, standing in for upstream's `Curve2DPtr`.
pub type Curve2DRef = Arc<Curve2D>;

macro_rules! curve_stub {
    ($name:ident, $upstream:literal) => {
        #[doc = concat!("Stub for upstream's `", $upstream, "`. Ported in Phase C (#19).")]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[non_exhaustive]
        pub struct $name;
    };
}

curve_stub!(BezierCurve, "BezierCurve");
curve_stub!(NurbsCurve, "NurbsCurve");
curve_stub!(BezierPatch, "BezierPatch");
curve_stub!(NurbsPatch, "NurbsPatch");
curve_stub!(BezierCurve2D, "BezierCurve2D");
curve_stub!(NurbsCurve2D, "NurbsCurve2D");

/// Upstream's `Polyline2D` — an explicit 2D polyline.
///
/// In a [`Revolution`] or [`Swung`] profile the `x` of each point is the
/// radius from the axis of revolution and the `y` is the height along it.
///
/// [`Revolution`]: crate::scenegraph::primitive::Revolution
/// [`Swung`]: crate::scenegraph::primitive::Swung
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Polyline2D {
    pub points: Arc<Vec<Point2>>,
    /// `Polyline2D::DEFAULT_WIDTH`, carried for the wireframe renderer.
    pub width: u32,
}

impl Polyline2D {
    /// `Polyline2D::DEFAULT_WIDTH`.
    pub const DEFAULT_WIDTH: u32 = 1;
    /// `Polyline2D::DEFAULT_ANGLE` — π, the arc a half circle spans.
    pub const DEFAULT_ANGLE: Real = std::f32::consts::PI;

    pub fn new(points: Vec<Point2>) -> Self {
        Self {
            points: Arc::new(points),
            width: Self::DEFAULT_WIDTH,
        }
    }

    /// `Polyline2D::Circle(radius, slices)` — a closed circle whose first and
    /// last point coincide exactly, so a swept profile closes without a seam.
    pub fn circle(radius: Real, slices: u8) -> Self {
        let slices = slices.max(3) as usize;
        let angle_delta = std::f32::consts::TAU / slices as Real;
        let mut points = Vec::with_capacity(slices + 1);
        points.push(Point2::new(radius, 0.0));
        for i in 1..slices {
            let angle = angle_delta * i as Real;
            points.push(Point2::new(radius * angle.cos(), radius * angle.sin()));
        }
        // Upstream repeats the first point rather than recomputing cos(2π),
        // which would leave a float's width of gap at the seam.
        points.push(Point2::new(radius, 0.0));
        Self::new(points)
    }

    /// `Polyline2D::ArcOfCircle(radius, starting_angle, angle_range, slices)`.
    pub fn arc_of_circle(
        radius: Real,
        starting_angle: Real,
        angle_range: Real,
        slices: u8,
    ) -> Self {
        let slices = slices.max(1) as usize;
        let angle_delta = angle_range / slices as Real;
        let points = (0..=slices)
            .map(|i| {
                let angle = starting_angle + angle_delta * i as Real;
                Point2::new(radius * angle.cos(), radius * angle.sin())
            })
            .collect();
        Self::new(points)
    }

    /// `isValid()` — a polyline needs at least two points.
    pub fn is_valid(&self) -> Result<()> {
        if self.points.len() < 2 {
            return Err(Error::degenerate("a 2D polyline needs at least 2 points"));
        }
        Ok(())
    }
}

/// Upstream's `Curve2D` hierarchy — the profile types a surface of revolution
/// can be swept from.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Curve2D {
    Polyline2D(Polyline2D),
    /// Phase C (#19).
    BezierCurve2D(BezierCurve2D),
    /// Phase C (#19).
    NurbsCurve2D(NurbsCurve2D),
}

impl Curve2D {
    /// Wraps in an [`Arc`], the form primitives hold.
    pub fn into_ref(self) -> Curve2DRef {
        Arc::new(self)
    }

    /// The upstream type name, for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Curve2D::Polyline2D(_) => "Polyline2D",
            Curve2D::BezierCurve2D(_) => "BezierCurve2D",
            Curve2D::NurbsCurve2D(_) => "NurbsCurve2D",
        }
    }

    /// The curve's points — upstream's `Discretizer::process(Curve2D*)`,
    /// which reduces any 2D curve to a polyline before it is swept.
    ///
    /// `samples` is the number of segments a parametric curve is evaluated
    /// at; a [`Polyline2D`] is already explicit and ignores it, exactly as
    /// upstream's discretizer does.
    pub fn sample(&self, samples: u32) -> Result<Vec<Point2>> {
        let _ = samples;
        match self {
            Curve2D::Polyline2D(p) => {
                p.is_valid()?;
                Ok(p.points.as_ref().clone())
            }
            other => Err(Error::unsupported(format!(
                "{} is not ported yet — Phase C (#19)",
                other.type_name()
            ))),
        }
    }

    /// The number of segments a curve reports as its natural sampling
    /// density — upstream's `Curve::getStride()`.
    pub fn stride(&self) -> Option<u32> {
        match self {
            Curve2D::Polyline2D(p) => Some(p.points.len().saturating_sub(1) as u32),
            _ => None,
        }
    }
}

impl From<Polyline2D> for Curve2D {
    fn from(p: Polyline2D) -> Self {
        Curve2D::Polyline2D(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn circle_closes_exactly_on_its_first_point() {
        let circle = Polyline2D::circle(2.0, 8);
        assert_eq!(circle.points.len(), 9);
        assert_eq!(circle.points[0], circle.points[8]);
        for p in circle.points.iter() {
            assert_relative_eq!(p.coords.norm(), 2.0, epsilon = 1e-5);
        }
    }

    #[test]
    fn an_arc_spans_its_range() {
        let arc = Polyline2D::arc_of_circle(1.0, 0.0, std::f32::consts::FRAC_PI_2, 4);
        assert_eq!(arc.points.len(), 5);
        assert_relative_eq!(arc.points[0], Point2::new(1.0, 0.0), epsilon = 1e-6);
        assert_relative_eq!(arc.points[4], Point2::new(0.0, 1.0), epsilon = 1e-6);
    }

    #[test]
    fn sampling_a_polyline_returns_its_own_points() {
        let curve = Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
        ]));
        assert_eq!(curve.sample(32).unwrap().len(), 2);
        assert_eq!(curve.stride(), Some(1));
    }

    #[test]
    fn sampling_a_phase_c_curve_is_unsupported() {
        let curve = Curve2D::NurbsCurve2D(NurbsCurve2D);
        assert!(matches!(curve.sample(32), Err(Error::Unsupported(_))));
        assert_eq!(curve.stride(), None);
    }

    #[test]
    fn a_one_point_polyline_is_degenerate() {
        let curve = Curve2D::from(Polyline2D::new(vec![Point2::new(1.0, 0.0)]));
        assert!(matches!(curve.sample(4), Err(Error::DegenerateGeometry(_))));
    }
}
