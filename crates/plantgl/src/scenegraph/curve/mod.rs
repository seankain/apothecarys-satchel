//! Curves and patches.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/{curve,lineicmodel,
//! polyline,beziercurve,nurbscurve,bezierpatch,nurbspatch}.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # Traits for the operations, enums for the dispatch
//!
//! Upstream's hierarchy is `Curve2D` and `LineicModel` as abstract bases with
//! `getPointAt`, `getTangentAt`, `getFirstKnot`, `getLastKnot`, `getStride` and
//! `getLength` as pure virtuals. The port keeps that *operation set* as
//! [`ParametricCurve`], but the set of curve types is closed — the same
//! argument [`crate::scenegraph::geometry`] makes for `Geometry` — so the
//! implementors are the [`Curve2D`] and [`Curve3D`] enums rather than a
//! `dyn` hierarchy: exhaustive at compile time, no vtable, and a profile can be
//! matched on when a caller needs to know what it actually is.

pub mod patch;
pub mod spline;

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point2, Point3, Real, Vec2, Vec3, EPSILON};
use crate::scenegraph::function::QuantisedFunction;
use crate::scenegraph::mesh::Polyline;

pub use patch::{BezierPatch, CtrlPointMatrix, NurbsPatch};
pub use spline::{
    BezierCurve, BezierCurve2D, NurbsCurve, NurbsCurve2D, DEFAULT_NURBS_DEGREE, DEFAULT_STRIDE,
};

/// Shared 2D curve handle, standing in for upstream's `Curve2DPtr`.
pub type Curve2DRef = Arc<Curve2D>;

/// Shared 3D curve handle, standing in for upstream's `LineicModelPtr`.
pub type Curve3DRef = Arc<Curve3D>;

/// A point a curve can be evaluated to. Only the distance is needed, for the
/// arc-length machinery [`ParametricCurve`] provides.
pub trait CurvePoint: Copy {
    fn distance_to(self, other: Self) -> Real;
}

impl CurvePoint for Point2 {
    fn distance_to(self, other: Self) -> Real {
        (other - self).norm()
    }
}

impl CurvePoint for Point3 {
    fn distance_to(self, other: Self) -> Real {
        (other - self).norm()
    }
}

/// The operations upstream's `Curve2D` and `LineicModel` declare.
///
/// `samples` throughout is the *fallback* density, in segments: a curve that
/// carries its own [`ParametricCurve::stride`] uses that instead, exactly as
/// the parametric primitives resolve their `slices` against
/// [`DiscretizeCtx`](crate::algo::discretize::DiscretizeCtx).
pub trait ParametricCurve {
    type Point: CurvePoint;
    type Tangent;

    /// `getFirstKnot()`.
    fn first_knot(&self) -> Real;

    /// `getLastKnot()`.
    fn last_knot(&self) -> Real;

    /// `getStride()`, or `None` for a curve that has no opinion.
    fn stride(&self) -> Option<u32>;

    /// `getPointAt(u)`.
    fn eval(&self, u: Real) -> Result<Self::Point>;

    /// `getTangentAt(u)`.
    fn tangent(&self, u: Real) -> Result<Self::Tangent>;

    /// The segment count this curve actually discretises at.
    fn resolved_stride(&self, samples: u32) -> u32 {
        self.stride().unwrap_or(samples).max(1)
    }

    /// The parameters the curve is sampled at, `resolved_stride + 1` of them
    /// spanning `[first_knot, last_knot]`.
    ///
    /// The last one is the last knot exactly rather than the accumulation of
    /// `stride` additions, which is upstream's own arrangement in
    /// `Discretizer::process(NurbsCurve*)` — an `f32` step summed thirty times
    /// does not land on 1.
    fn parameters(&self, samples: u32) -> Vec<Real> {
        let stride = self.resolved_stride(samples);
        let first = self.first_knot();
        let last = self.last_knot();
        let step = (last - first) / stride as Real;
        let mut parameters: Vec<Real> = (0..stride).map(|i| first + step * i as Real).collect();
        parameters.push(last);
        parameters
    }

    /// `Discretizer::process` for this curve: the polyline it reduces to.
    fn discretize(&self, samples: u32) -> Result<Vec<Self::Point>> {
        self.parameters(samples)
            .into_iter()
            .map(|u| self.eval(u))
            .collect()
    }

    /// `getLength()` — the length of the discretisation, upstream's own
    /// definition of a curve's length.
    ///
    /// Upstream also takes a `[begin, end]` sub-range; nothing in the port
    /// needs one, and its implementation adds `getFirstKnot()` to an offset
    /// that already includes it, so only the full-range case is translated.
    fn length(&self, samples: u32) -> Result<Real> {
        let points = self.discretize(samples)?;
        Ok(points
            .windows(2)
            .map(|pair| pair[0].distance_to(pair[1]))
            .sum())
    }

    /// Whether the curve returns to where it started — what makes a swept
    /// cross-section a tube rather than a sheet.
    fn is_closed(&self) -> bool {
        match (self.eval(self.first_knot()), self.eval(self.last_knot())) {
            (Ok(first), Ok(last)) => first.distance_to(last) <= EPSILON,
            _ => false,
        }
    }

    /// `getUToArcLengthMapping()` — parameter to normalised arc length.
    ///
    /// The mapping a swept surface needs for a texture coordinate that runs at
    /// a constant rate over the *surface* rather than over the parameter, which
    /// is what stops bark bunching where a curve is sampled densely.
    fn u_to_arc_length_mapping(&self, samples: u32) -> Result<QuantisedFunction> {
        let points = self.discretize(samples)?;
        let mut lengths = Vec::with_capacity(points.len());
        let mut total = 0.0;
        lengths.push(0.0);
        for pair in points.windows(2) {
            total += pair[0].distance_to(pair[1]);
            lengths.push(total);
        }
        if total <= EPSILON {
            return Err(Error::degenerate(
                "a curve of zero length has no arc-length parameterisation",
            ));
        }
        for length in &mut lengths {
            *length /= total;
        }
        QuantisedFunction::from_samples(lengths, self.first_knot(), self.last_knot())
    }

    /// `getArcLengthToUMapping()` — normalised arc length to parameter, the
    /// inverse of [`ParametricCurve::u_to_arc_length_mapping`].
    ///
    /// This is what a turtle guide runs on: "advance 0.3 of the way along this
    /// curve *by length*" is a question about arc length, and the curve only
    /// answers questions about its parameter.
    ///
    /// Upstream builds the `(arc length, u)` pairs at its stride and hands
    /// them to `QuantisedFunction`, which resamples them onto an evenly spaced
    /// grid of `5 * stride` points. This crate's [`QuantisedFunction`] is
    /// evenly spaced by construction, so the resampling is done here instead
    /// — the same linear interpolation, in the same places.
    fn arc_length_to_u_mapping(&self, samples: u32) -> Result<QuantisedFunction> {
        let parameters = self.parameters(samples);
        let points = self.discretize(samples)?;

        // The cumulative length at each parameter, skipping the samples that
        // did not advance — upstream skips those too, so a repeated point
        // cannot produce two `u` for one arc length.
        let mut table: Vec<(Real, Real)> = vec![(0.0, parameters[0])];
        let mut total = 0.0;
        for i in 1..points.len() {
            let step = points[i - 1].distance_to(points[i]);
            if step <= 0.0 {
                continue;
            }
            total += step;
            table.push((total, parameters[i]));
        }
        if total <= EPSILON || table.len() < 2 {
            return Err(Error::degenerate(
                "a curve of zero length has no arc-length parameterisation",
            ));
        }
        for entry in &mut table {
            entry.0 /= total;
        }
        // The last sample is the last knot exactly, whatever rounding did to
        // the accumulated length.
        if let Some(last) = table.last_mut() {
            *last = (1.0, self.last_knot());
        }

        let count = (QuantisedFunction::SAMPLES_PER_SEGMENT * self.resolved_stride(samples)) as usize
            + 1;
        let mut values = Vec::with_capacity(count);
        let mut cursor = 0;
        for i in 0..count {
            let s = i as Real / (count - 1) as Real;
            while cursor + 2 < table.len() && table[cursor + 1].0 < s {
                cursor += 1;
            }
            let (s0, u0) = table[cursor];
            let (s1, u1) = table[cursor + 1];
            let span = s1 - s0;
            let t = if span > 0.0 {
                ((s - s0) / span).clamp(0.0, 1.0)
            } else {
                0.0
            };
            values.push(u0 + (u1 - u0) * t);
        }
        QuantisedFunction::from_samples(values, 0.0, 1.0)
    }
}

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
    ///
    /// This is the default cross-section the turtle sweeps: upstream's
    /// `Turtle::setDefaultCrossSection` is `Polyline2D::Circle(1, slices)`.
    /// It is an *n*-gon inscribed in the circle; [`NurbsCurve2D::circle`] is
    /// the exact one.
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

    /// `getPointAt(u)`: the parameter of an explicit polyline is the point
    /// index, so `u = 2.5` is half way along its third segment.
    pub fn eval(&self, u: Real) -> Result<Point2> {
        self.is_valid()?;
        Ok(interpolate(&self.points, u, |a, b, t| a + (b - a) * t))
    }

    /// `getTangentAt(u)` — the segment direction, and at an interior vertex the
    /// length-weighted mean of the two segments meeting there, as upstream.
    pub fn tangent(&self, u: Real) -> Result<Vec2> {
        self.is_valid()?;
        Ok(polyline_tangent(&self.points, u, |a, b| b - a))
    }
}

/// Upstream's `Curve2D` hierarchy — the profile types a surface of revolution
/// is swept from and an [`Extrusion`](crate::scenegraph::primitive::Extrusion)
/// sweeps along its axis.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Curve2D {
    Polyline2D(Polyline2D),
    BezierCurve2D(BezierCurve2D),
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

    /// `isValid()`.
    pub fn is_valid(&self) -> Result<()> {
        match self {
            Curve2D::Polyline2D(c) => c.is_valid(),
            Curve2D::BezierCurve2D(c) => c.is_valid(),
            Curve2D::NurbsCurve2D(c) => c.is_valid(),
        }
    }
}

impl ParametricCurve for Curve2D {
    type Point = Point2;
    type Tangent = Vec2;

    fn first_knot(&self) -> Real {
        match self {
            Curve2D::Polyline2D(_) | Curve2D::BezierCurve2D(_) => 0.0,
            Curve2D::NurbsCurve2D(c) => c.first_knot(),
        }
    }

    fn last_knot(&self) -> Real {
        match self {
            // An explicit polyline is parameterised by its point index.
            Curve2D::Polyline2D(c) => c.points.len().saturating_sub(1) as Real,
            Curve2D::BezierCurve2D(_) => 1.0,
            Curve2D::NurbsCurve2D(c) => c.last_knot(),
        }
    }

    fn stride(&self) -> Option<u32> {
        match self {
            Curve2D::Polyline2D(c) => Some(c.points.len().saturating_sub(1) as u32),
            Curve2D::BezierCurve2D(c) => c.stride,
            Curve2D::NurbsCurve2D(c) => c.stride,
        }
    }

    fn eval(&self, u: Real) -> Result<Point2> {
        match self {
            Curve2D::Polyline2D(c) => c.eval(u),
            Curve2D::BezierCurve2D(c) => c.eval(u),
            Curve2D::NurbsCurve2D(c) => c.eval(u),
        }
    }

    fn tangent(&self, u: Real) -> Result<Vec2> {
        match self {
            Curve2D::Polyline2D(c) => c.tangent(u),
            Curve2D::BezierCurve2D(c) => c.tangent(u),
            Curve2D::NurbsCurve2D(c) => c.tangent(u),
        }
    }

    /// An explicit polyline discretises to its own points, whatever the
    /// requested density — upstream's `Discretizer::process(Polyline2D*)`
    /// copies the point list rather than resampling it.
    fn discretize(&self, samples: u32) -> Result<Vec<Point2>> {
        if let Curve2D::Polyline2D(polyline) = self {
            polyline.is_valid()?;
            return Ok(polyline.points.as_ref().clone());
        }
        self.parameters(samples)
            .into_iter()
            .map(|u| self.eval(u))
            .collect()
    }
}

impl From<Polyline2D> for Curve2D {
    fn from(c: Polyline2D) -> Self {
        Curve2D::Polyline2D(c)
    }
}

impl From<BezierCurve2D> for Curve2D {
    fn from(c: BezierCurve2D) -> Self {
        Curve2D::BezierCurve2D(c)
    }
}

impl From<NurbsCurve2D> for Curve2D {
    fn from(c: NurbsCurve2D) -> Self {
        Curve2D::NurbsCurve2D(c)
    }
}

/// Upstream's `LineicModel` hierarchy — the 3D curves, and what an
/// [`Extrusion`](crate::scenegraph::primitive::Extrusion) can take as an axis.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Curve3D {
    Polyline(Polyline),
    BezierCurve(BezierCurve),
    NurbsCurve(NurbsCurve),
}

impl Curve3D {
    pub fn into_ref(self) -> Curve3DRef {
        Arc::new(self)
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Curve3D::Polyline(_) => "Polyline",
            Curve3D::BezierCurve(_) => "BezierCurve",
            Curve3D::NurbsCurve(_) => "NurbsCurve",
        }
    }

    pub fn is_valid(&self) -> Result<()> {
        match self {
            Curve3D::Polyline(c) => c.is_valid(),
            Curve3D::BezierCurve(c) => c.is_valid(),
            Curve3D::NurbsCurve(c) => c.is_valid(),
        }
    }

    /// `getNormalAt(u)` — the principal normal where the curve has curvature,
    /// and for a polyline the world-axis perpendicular upstream substitutes.
    pub fn normal(&self, u: Real) -> Result<Vec3> {
        match self {
            Curve3D::Polyline(c) => c.normal(u),
            Curve3D::BezierCurve(c) => c.normal(u),
            Curve3D::NurbsCurve(c) => c.normal(u),
        }
    }

    /// The frame a sweep along this curve starts from — upstream's
    /// `Extrusion::getInitialNormalValue`, as a whole frame.
    ///
    /// The cross-section's `x` axis maps to the frame's `left` and its `y` to
    /// `up`, which is upstream's `Matrix3(normal, binormal, velocity)` with
    /// `binormal = velocity × normal`. Where the curve is locally straight it
    /// has no principal normal, and upstream falls back to crossing the tangent
    /// with whichever world axis it leans on least; that fallback is translated
    /// term for term, because it decides the phase of every cross-section and a
    /// different choice would rotate the whole sweep.
    pub fn initial_frame(&self) -> Result<crate::math::Frame> {
        let u = self.first_knot();
        let position = self.eval(u)?;
        let tangent = self.tangent(u)?;
        let mut normal = self.normal(u)?;
        if normal.norm_squared() <= EPSILON {
            let t = tangent;
            let reference = if t.x < t.y {
                if t.z < t.x {
                    Vec3::z()
                } else {
                    Vec3::x()
                }
            } else if t.z < t.y {
                Vec3::z()
            } else {
                Vec3::y()
            };
            normal = t.cross(&reference);
        }
        let mut frame = crate::math::Frame::new(position, tangent, normal, Vec3::zeros());
        if !frame.orthonormalize() {
            return Err(Error::degenerate(format!(
                "the {} has no usable frame at its start: its tangent and normal are degenerate",
                self.type_name()
            )));
        }
        Ok(frame)
    }
}

impl ParametricCurve for Curve3D {
    type Point = Point3;
    type Tangent = Vec3;

    fn first_knot(&self) -> Real {
        match self {
            Curve3D::Polyline(_) | Curve3D::BezierCurve(_) => 0.0,
            Curve3D::NurbsCurve(c) => c.first_knot(),
        }
    }

    fn last_knot(&self) -> Real {
        match self {
            Curve3D::Polyline(c) => c.points.len().saturating_sub(1) as Real,
            Curve3D::BezierCurve(_) => 1.0,
            Curve3D::NurbsCurve(c) => c.last_knot(),
        }
    }

    fn stride(&self) -> Option<u32> {
        match self {
            Curve3D::Polyline(c) => Some(c.points.len().saturating_sub(1) as u32),
            Curve3D::BezierCurve(c) => c.stride,
            Curve3D::NurbsCurve(c) => c.stride,
        }
    }

    fn eval(&self, u: Real) -> Result<Point3> {
        match self {
            Curve3D::Polyline(c) => c.eval(u),
            Curve3D::BezierCurve(c) => c.eval(u),
            Curve3D::NurbsCurve(c) => c.eval(u),
        }
    }

    fn tangent(&self, u: Real) -> Result<Vec3> {
        match self {
            Curve3D::Polyline(c) => c.tangent(u),
            Curve3D::BezierCurve(c) => c.tangent(u),
            Curve3D::NurbsCurve(c) => c.tangent(u),
        }
    }

    fn discretize(&self, samples: u32) -> Result<Vec<Point3>> {
        if let Curve3D::Polyline(polyline) = self {
            polyline.is_valid()?;
            return Ok(polyline.points.as_ref().clone());
        }
        self.parameters(samples)
            .into_iter()
            .map(|u| self.eval(u))
            .collect()
    }
}

impl From<Polyline> for Curve3D {
    fn from(c: Polyline) -> Self {
        Curve3D::Polyline(c)
    }
}

impl From<BezierCurve> for Curve3D {
    fn from(c: BezierCurve) -> Self {
        Curve3D::BezierCurve(c)
    }
}

impl From<NurbsCurve> for Curve3D {
    fn from(c: NurbsCurve) -> Self {
        Curve3D::NurbsCurve(c)
    }
}

// --- The explicit-polyline parameterisation, shared by 2D and 3D ------------

/// `Polyline::getPointAt(u)`: `u` indexes the point list, and a fractional `u`
/// interpolates the segment it falls in.
pub(crate) fn interpolate<P, F>(points: &[P], u: Real, lerp: F) -> P
where
    P: Copy,
    F: Fn(P, P, Real) -> P,
{
    let last = points.len() - 1;
    let u = u.clamp(0.0, last as Real);
    let index = u.floor();
    let i = index as usize;
    if i >= last {
        return points[last];
    }
    let t = u - index;
    if t.abs() < EPSILON {
        return points[i];
    }
    lerp(points[i], points[i + 1], t)
}

/// `Polyline::getTangentAt(u)`.
pub(crate) fn polyline_tangent<P, V, F>(points: &[P], u: Real, delta: F) -> V
where
    P: Copy,
    V: Copy + std::ops::Add<Output = V> + std::ops::Mul<Real, Output = V> + Norm,
    F: Fn(P, P) -> V,
{
    let last = points.len() - 1;
    if u <= 0.0 {
        return delta(points[0], points[1]);
    }
    if u >= last as Real {
        return delta(points[last - 1], points[last]);
    }
    let index = u.floor();
    let i = index as usize;
    if (u - index).abs() >= EPSILON {
        return delta(points[i], points[i + 1]);
    }
    // At a vertex upstream averages the two adjacent unit directions and scales
    // by their mean length, so a long segment does not dominate the direction
    // but still sets the magnitude.
    let before = delta(points[i - 1], points[i]);
    let after = delta(points[i], points[i + 1]);
    let (la, lb) = (before.norm(), after.norm());
    let unit = |v: V, l: Real| if l > 0.0 { v * (1.0 / l) } else { v };
    (unit(before, la) + unit(after, lb)) * ((la + lb) / 2.0)
}

/// The one thing [`polyline_tangent`] needs of a vector type.
pub(crate) trait Norm {
    fn norm(&self) -> Real;
}

impl Norm for Vec2 {
    fn norm(&self) -> Real {
        nalgebra::Vector2::norm(self)
    }
}

impl Norm for Vec3 {
    fn norm(&self) -> Real {
        nalgebra::Vector3::norm(self)
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
        assert!(Curve2D::from(circle).is_closed());
    }

    #[test]
    fn an_arc_spans_its_range() {
        let arc = Polyline2D::arc_of_circle(1.0, 0.0, std::f32::consts::FRAC_PI_2, 4);
        assert_eq!(arc.points.len(), 5);
        assert_relative_eq!(arc.points[0], Point2::new(1.0, 0.0), epsilon = 1e-6);
        assert_relative_eq!(arc.points[4], Point2::new(0.0, 1.0), epsilon = 1e-6);
        assert!(!Curve2D::from(arc).is_closed());
    }

    #[test]
    fn discretising_a_polyline_returns_its_own_points() {
        let curve = Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
        ]));
        assert_eq!(curve.discretize(32).unwrap().len(), 2);
        assert_eq!(curve.stride(), Some(1));
    }

    #[test]
    fn a_one_point_polyline_is_degenerate() {
        let curve = Curve2D::from(Polyline2D::new(vec![Point2::new(1.0, 0.0)]));
        assert!(matches!(
            curve.discretize(4),
            Err(Error::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn a_polyline_is_parameterised_by_its_point_index() {
        let line = Polyline2D::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0),
        ]);
        assert_relative_eq!(line.eval(0.0).unwrap(), Point2::new(0.0, 0.0));
        assert_relative_eq!(line.eval(0.5).unwrap(), Point2::new(1.0, 0.0));
        assert_relative_eq!(line.eval(1.0).unwrap(), Point2::new(2.0, 0.0));
        assert_relative_eq!(line.eval(1.5).unwrap(), Point2::new(2.0, 1.0));
        assert_relative_eq!(line.eval(2.0).unwrap(), Point2::new(2.0, 2.0));
    }

    #[test]
    fn a_polyline_tangent_follows_its_segments() {
        let line = Polyline2D::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(2.0, 2.0),
        ]);
        assert_relative_eq!(line.tangent(0.5).unwrap(), Vec2::new(2.0, 0.0));
        assert_relative_eq!(line.tangent(1.5).unwrap(), Vec2::new(0.0, 2.0));
        // At the corner, the mean of the two unit directions scaled by their
        // mean length.
        assert_relative_eq!(
            line.tangent(1.0).unwrap(),
            Vec2::new(2.0, 2.0),
            epsilon = 1e-6
        );
    }

    #[test]
    fn a_nurbs_curve_reports_its_own_knot_range() {
        let curve = Curve2D::from(NurbsCurve2D::circle(1.0));
        assert_relative_eq!(curve.first_knot(), 0.0);
        assert_relative_eq!(curve.last_knot(), 1.0);
        assert!(curve.is_closed());
    }

    #[test]
    fn discretising_a_parametric_curve_ends_on_the_last_knot() {
        let curve = Curve2D::from(BezierCurve2D::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(2.0, 0.0),
        ]));
        let points = curve.discretize(8).unwrap();
        assert_eq!(points.len(), 9);
        assert_relative_eq!(points[0], Point2::new(0.0, 0.0));
        assert_relative_eq!(points[8], Point2::new(2.0, 0.0), epsilon = 1e-6);
    }

    /// Acceptance (T8.6): the arc length of a straight NURBS line is its chord.
    #[test]
    fn the_length_of_a_straight_nurbs_line_is_its_chord() {
        let curve = Curve3D::from(NurbsCurve::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 2.0, 2.0),
                Point3::new(2.0, 4.0, 4.0),
                Point3::new(3.0, 6.0, 6.0),
            ],
            3,
        ));
        let chord = (Point3::new(3.0, 6.0, 6.0) - Point3::origin()).norm();
        assert_relative_eq!(curve.length(64).unwrap(), chord, epsilon = 1e-4);
    }

    #[test]
    fn arc_length_mapping_is_monotone_and_spans_zero_to_one() {
        let curve = Curve3D::from(NurbsCurve::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 3.0, 0.0),
                Point3::new(3.0, 3.0, 2.0),
                Point3::new(4.0, 0.0, 2.0),
            ],
            3,
        ));
        let mapping = curve.u_to_arc_length_mapping(32).unwrap();
        assert_relative_eq!(mapping.value(0.0), 0.0, epsilon = 1e-6);
        assert_relative_eq!(mapping.value(1.0), 1.0, epsilon = 1e-6);
        assert!(mapping.is_increasing(false));
    }

    #[test]
    fn a_zero_length_curve_has_no_arc_length_mapping() {
        let curve = Curve3D::from(Polyline::new(vec![Point3::origin(), Point3::origin()]));
        assert!(matches!(
            curve.u_to_arc_length_mapping(8),
            Err(Error::DegenerateGeometry(_))
        ));
    }
}
