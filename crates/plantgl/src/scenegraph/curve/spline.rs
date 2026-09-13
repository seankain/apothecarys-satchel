//! Bézier and NURBS evaluation.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/{beziercurve,
//! nurbscurve}.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189, whose
//! algorithms are in turn Piegl and Tiller, *The NURBS Book*, 2nd ed. — the
//! algorithm numbers upstream cites (A2.1 `findSpan`, A2.2 `basisFunctions`,
//! A2.3 `derivatesBasisFunctions`, A4.2 the rational derivative) are kept in
//! the comments here.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # Everything here computes in `f64`
//!
//! Upstream's `real_t` is `f32` unless `PGL_USE_DOUBLE` is set, and the port
//! follows it for stored geometry ([`crate::math::Real`]). Knot-span arithmetic
//! is the one place where that margin is genuinely thin: `basis_functions`
//! divides by `right[r + 1] + left[j - r]`, a difference of knots, and a
//! 200-control-point curve's knots are spaced ~5e-3 apart — four `f32` decimal
//! digits from a catastrophic cancellation. So knots are widened to `f64` on
//! the way in, every basis function and de Boor accumulation runs in `f64`, and
//! only the finished point is narrowed back to `f32`. `precision_holds_on_a_two_hundred_point_curve`
//! in `tests/analytic.rs` is the check on that claim.
//!
//! # Control points carry their weight, they are not homogeneous
//!
//! Upstream stores a control point as `Vector4(x, y, z, w)` where `x, y, z` are
//! *Cartesian* and `w` is the weight; `wtoxyz()` produces the homogeneous
//! `(wx, wy, wz, w)` the algorithms need and `project()` divides back out. The
//! port keeps that layout — [`Vec4`] for a 3D control point, [`Vec3`] for a 2D
//! one with `z` as the weight — so a control net transcribed from PlantGL means
//! the same thing here.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point2, Point3, Real, Vec2, Vec3, Vec4};

/// A homogeneous point in the precision the algorithms run at.
pub(crate) type Homogeneous = nalgebra::Vector4<f64>;

/// `BezierCurve::DEFAULT_STRIDE` — the number of segments a parametric curve
/// discretises to when it does not carry its own.
pub const DEFAULT_STRIDE: u32 = 30;

/// `NurbsCurve::DEFAULT_NURBS_DEGREE`.
pub const DEFAULT_NURBS_DEGREE: usize = 3;

/// `Curve::DEFAULT_WIDTH` — the wireframe width, carried for the renderer.
pub const DEFAULT_WIDTH: u32 = 1;

/// Below this a weight is zero and the point cannot be projected — upstream's
/// `GEOM_TOLERANCE` guard in `getPointAt`.
const WEIGHT_TOLERANCE: f64 = 1e-10;

/// The tolerance `isValid` compares knots at, upstream's `GEOM_TOLERANCE`.
const KNOT_TOLERANCE: Real = 1e-10;

// --- Knot vectors -----------------------------------------------------------

/// `NurbsCurve::defaultKnotList(nbCtrlPoints, degree)` — the clamped, uniform
/// knot vector: `degree + 1` zeros, evenly spaced interior knots, `degree + 1`
/// ones.
/// A degenerate control net produces a degenerate knot vector rather than a
/// panic; the curve's own `is_valid` is what rejects it, with a message about
/// the control points rather than about their knots.
pub fn default_knots(ctrl_count: usize, degree: usize) -> Vec<Real> {
    let degree = degree.max(1);
    let size = ctrl_count + degree + 1;
    let mut knots = vec![0.0; size];
    // The number of interior spans; zero for a Bézier-like net, where the loop
    // below does not run at all.
    let spans = size.saturating_sub(2 * degree + 1).max(1);
    for (j, knot) in knots
        .iter_mut()
        .enumerate()
        .take(size - degree - 1)
        .skip(degree + 1)
    {
        *knot = (j - degree) as Real / spans as Real;
    }
    for knot in knots.iter_mut().skip(size - degree - 1) {
        *knot = 1.0;
    }
    knots
}

/// The degree a knot vector implies for a given control net, as upstream's
/// builder infers it: `knots - control points - 1`.
pub fn implied_degree(knot_count: usize, ctrl_count: usize) -> Option<usize> {
    knot_count.checked_sub(ctrl_count + 1).filter(|d| *d >= 1)
}

/// `NurbsCurve::Builder::isValid`'s knot checks, as a `Result`.
///
/// Upstream warns and builds the object anyway; a knot vector that is too
/// short or not non-decreasing makes `findSpan` index out of range, so the port
/// refuses it instead. The three conditions are upstream's:
///
/// - the length is `control points + degree + 1`,
/// - the vector is clamped — the first and last `degree + 1` knots each repeat,
/// - and (ours, because upstream's binary search silently assumes it) the
///   sequence is non-decreasing and finite.
pub fn validate_knots(knots: &[Real], ctrl_count: usize, degree: usize) -> Result<()> {
    let expected = ctrl_count + degree + 1;
    if knots.len() != expected {
        return Err(Error::BadKnotVector(format!(
            "{} knots for {ctrl_count} control points of degree {degree}, expected {expected}",
            knots.len()
        )));
    }
    if let Some((i, knot)) = knots.iter().enumerate().find(|(_, k)| !k.is_finite()) {
        return Err(Error::BadKnotVector(format!("knot {i} is {knot}")));
    }
    if let Some((i, pair)) = knots.windows(2).enumerate().find(|(_, w)| w[1] < w[0]) {
        return Err(Error::BadKnotVector(format!(
            "knots must be non-decreasing, but knot {} is {} after {}",
            i + 1,
            pair[1],
            pair[0]
        )));
    }
    if knots[0] >= knots[knots.len() - 1] {
        return Err(Error::BadKnotVector(format!(
            "the knot range [{}, {}] is empty",
            knots[0],
            knots[knots.len() - 1]
        )));
    }
    let clamped_at = |start: usize| {
        let value = knots[start];
        knots[start..start + degree + 1]
            .iter()
            .all(|k| (k - value).abs() < KNOT_TOLERANCE)
    };
    if !clamped_at(0) || !clamped_at(knots.len() - degree - 1) {
        return Err(Error::BadKnotVector(
            "the vector must be clamped: the first and last knots each repeat degree + 1 times"
                .to_string(),
        ));
    }
    Ok(())
}

/// Widens a stored knot vector for the arithmetic below.
pub(crate) fn widen(knots: &[Real]) -> Vec<f64> {
    knots.iter().map(|k| *k as f64).collect()
}

// --- The NURBS Book algorithms ----------------------------------------------

/// A2.1 `findSpan` — the index of the knot span containing `u`.
///
/// Valid for clamped knot vectors only, which [`validate_knots`] enforces. The
/// return is always in `degree ..= n - 1`, so a caller can index
/// `ctrl[span - degree ..= span]` without a bounds check.
pub fn find_span(u: f64, degree: usize, knots: &[f64]) -> usize {
    let n = knots.len() - degree - 1;
    if u >= knots[n] {
        return n - 1;
    }
    if u <= knots[degree] {
        return degree;
    }

    let (mut low, mut high) = (degree, n);
    let mut mid = (low + high) / 2;
    while u < knots[mid] || u >= knots[mid + 1] {
        if u < knots[mid] {
            high = mid;
        } else {
            low = mid;
        }
        mid = (low + high) / 2;
    }
    mid
}

/// A2.2 `basisFunctions` — the `degree + 1` non-zero basis functions at `u`.
///
/// Upstream carries a guard for `span >= knots - degree - 1` "for clamped
/// vector only"; [`find_span`] cannot return such a span, so the branch is
/// unreachable and is not translated.
pub fn basis_functions(span: usize, u: f64, degree: usize, knots: &[f64]) -> Vec<f64> {
    let mut basis = vec![0.0; degree + 1];
    let mut left = vec![0.0; degree + 1];
    let mut right = vec![0.0; degree + 1];
    basis[0] = 1.0;

    for j in 1..=degree {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            // A clamped, non-decreasing vector makes this denominator the width
            // of a span that contains u, which is positive.
            let temp = basis[r] / (right[r + 1] + left[j - r]);
            basis[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        basis[j] = saved;
    }
    basis
}

/// A2.3 `derivatesBasisFunctions` — basis functions and their first `n`
/// derivatives, as `ders[k][j]` for derivative `k` of basis function `j`.
pub fn basis_derivatives(
    n: usize,
    u: f64,
    span: usize,
    degree: usize,
    knots: &[f64],
) -> Vec<Vec<f64>> {
    let mut ndu = vec![vec![0.0; degree + 1]; degree + 1];
    let mut left = vec![0.0; degree + 1];
    let mut right = vec![0.0; degree + 1];
    ndu[0][0] = 1.0;

    for j in 1..=degree {
        left[j] = u - knots[span + 1 - j];
        right[j] = knots[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            // Lower triangle: the knot differences. Upper: the basis functions.
            ndu[j][r] = right[r + 1] + left[j - r];
            let temp = ndu[r][j - 1] / ndu[j][r];
            ndu[r][j] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        ndu[j][j] = saved;
    }

    let mut ders = vec![vec![0.0; degree + 1]; n + 1];
    for j in 0..=degree {
        ders[0][j] = ndu[j][degree];
    }

    let mut a = vec![vec![0.0; degree + 1]; 2];
    for r in 0..=degree {
        let (mut s1, mut s2) = (0usize, 1usize);
        a[0][0] = 1.0;
        for k in 1..=n {
            let mut d = 0.0;
            let rk = r as isize - k as isize;
            let pk = degree as isize - k as isize;

            if r >= k {
                a[s2][0] = a[s1][0] / ndu[(pk + 1) as usize][rk as usize];
                d = a[s2][0] * ndu[rk as usize][pk as usize];
            }
            let j1 = if rk >= -1 { 1 } else { (-rk) as usize };
            let j2 = if (r as isize) - 1 <= pk {
                k - 1
            } else {
                degree - r
            };
            for j in j1..=j2 {
                let index = (rk + j as isize) as usize;
                a[s2][j] = (a[s1][j] - a[s1][j - 1]) / ndu[(pk + 1) as usize][index];
                d += a[s2][j] * ndu[index][pk as usize];
            }
            if (r as isize) <= pk {
                a[s2][k] = -a[s1][k - 1] / ndu[(pk + 1) as usize][r];
                d += a[s2][k] * ndu[r][pk as usize];
            }
            ders[k][r] = d;
            std::mem::swap(&mut s1, &mut s2);
        }
    }

    // Multiply through by the falling factorial p!/(p - k)!.
    let mut factor = degree;
    for (k, row) in ders.iter_mut().enumerate().take(n + 1).skip(1) {
        for value in row.iter_mut() {
            *value *= factor as f64;
        }
        factor = factor.saturating_mul(degree.saturating_sub(k));
    }
    ders
}

/// Pascal's triangle up to `d`, as `nurbspatch.cpp`'s `binomialCoef`.
fn binomials(d: usize) -> Vec<Vec<f64>> {
    let mut bin = vec![vec![0.0; d + 1]; d + 1];
    bin[0][0] = 1.0;
    for n in 0..d {
        bin[n + 1][0] = 1.0;
        for l in 1..=d {
            if n + 1 >= l {
                bin[n + 1][l] = bin[n][l] + bin[n][l - 1];
            }
        }
    }
    bin
}

/// de Casteljau over homogeneous control points — upstream's
/// `BezierCurve::getPointAt`, which is the same repeated linear interpolation.
pub(crate) fn de_casteljau(ctrl: &[Homogeneous], u: f64) -> Homogeneous {
    let mut q = ctrl.to_vec();
    let u1 = 1.0 - u;
    for k in 1..q.len() {
        for i in 0..(q.len() - k) {
            q[i] = q[i] * u1 + q[i + 1] * u;
        }
    }
    q[0]
}

/// The hodograph: the control net of the derivative of a Bézier curve.
pub(crate) fn hodograph(ctrl: &[Homogeneous]) -> Vec<Homogeneous> {
    let degree = ctrl.len() - 1;
    ctrl.windows(2)
        .map(|pair| (pair[1] - pair[0]) * degree as f64)
        .collect()
}

/// de Boor: the point of a NURBS curve at `u`, in homogeneous coordinates.
pub(crate) fn nurbs_point(
    ctrl: &[Homogeneous],
    degree: usize,
    knots: &[f64],
    u: f64,
) -> Homogeneous {
    let span = find_span(u, degree, knots);
    let basis = basis_functions(span, u, degree, knots);
    let mut point = Homogeneous::zeros();
    for (j, weight) in basis.iter().enumerate() {
        point += ctrl[span - degree + j] * *weight;
    }
    point
}

/// A3.2/A4.2: derivatives `0 ..= d` of the homogeneous NURBS curve.
pub(crate) fn nurbs_derivatives_h(
    ctrl: &[Homogeneous],
    degree: usize,
    knots: &[f64],
    u: f64,
    d: usize,
) -> Vec<Homogeneous> {
    let span = find_span(u, degree, knots);
    let du = d.min(degree);
    let basis = basis_derivatives(du, u, span, degree, knots);
    let mut ders = vec![Homogeneous::zeros(); d + 1];
    for (k, der) in ders.iter_mut().enumerate().take(du + 1) {
        for j in 0..=degree {
            *der += ctrl[span - degree + j] * basis[k][j];
        }
    }
    ders
}

/// A4.2: the derivatives of the rational curve from those of its homogeneous
/// lift — `C^(k) = (A^(k) - Σ binom(k, i) w^(i) C^(k-i)) / w`.
///
/// This is where the port and upstream part company for a *rational* Bézier.
/// Upstream's `BezierCurve::getTangentAt` differences the stored control points
/// and calls `project()` on the result, which divides by a difference of
/// weights rather than applying the quotient rule; for the weight-1 curves that
/// dominate plant modelling the difference has `w = 0` and upstream's answer is
/// exactly this one, but for a NURBS circle it is not a tangent at all. The
/// port applies the quotient rule everywhere, so a rational cross-section has a
/// usable tangent.
pub(crate) fn rational_derivatives(ders_h: &[Homogeneous]) -> Vec<nalgebra::Vector3<f64>> {
    let d = ders_h.len() - 1;
    let bin = binomials(d);
    let w0 = ders_h[0].w;
    let mut ders: Vec<nalgebra::Vector3<f64>> = vec![nalgebra::Vector3::zeros(); d + 1];
    for k in 0..=d {
        let mut v = ders_h[k].xyz();
        for i in 1..=k {
            v -= ders[k - i] * (bin[k][i] * ders_h[i].w);
        }
        ders[k] = if w0.abs() < WEIGHT_TOLERANCE {
            v
        } else {
            v / w0
        };
    }
    ders
}

/// `Vector4::project()` — divide by the weight, or pass the point through when
/// the weight is zero, exactly as upstream's guard does.
pub(crate) fn project(point: Homogeneous) -> nalgebra::Vector3<f64> {
    if point.w.abs() < WEIGHT_TOLERANCE {
        point.xyz()
    } else {
        point.xyz() / point.w
    }
}

/// `Vector4::wtoxyz()` — a stored control point `(x, y, z, w)` as the
/// homogeneous `(wx, wy, wz, w)`.
pub(crate) fn to_homogeneous_3d(point: &Vec4) -> Homogeneous {
    let w = point.w as f64;
    Homogeneous::new(
        point.x as f64 * w,
        point.y as f64 * w,
        point.z as f64 * w,
        w,
    )
}

/// The 2D equivalent: a stored `(x, y, w)` as `(wx, wy, 0, w)`.
pub(crate) fn to_homogeneous_2d(point: &Vec3) -> Homogeneous {
    let w = point.z as f64;
    Homogeneous::new(point.x as f64 * w, point.y as f64 * w, 0.0, w)
}

fn narrow_point3(v: nalgebra::Vector3<f64>) -> Point3 {
    Point3::new(v.x as Real, v.y as Real, v.z as Real)
}

fn narrow_point2(v: nalgebra::Vector3<f64>) -> Point2 {
    Point2::new(v.x as Real, v.y as Real)
}

fn narrow_vec3(v: nalgebra::Vector3<f64>) -> Vec3 {
    Vec3::new(v.x as Real, v.y as Real, v.z as Real)
}

fn narrow_vec2(v: nalgebra::Vector3<f64>) -> Vec2 {
    Vec2::new(v.x as Real, v.y as Real)
}

/// The shared validity check on a control net: enough points, all finite, and
/// no zero weight — upstream's `Builder::isValid` for every curve type.
fn validate_ctrl_points<T, F>(points: &[T], weight: F, what: &str) -> Result<()>
where
    F: Fn(&T) -> Real,
    T: std::fmt::Debug,
{
    if points.len() < 2 {
        return Err(Error::degenerate(format!(
            "{what} needs at least 2 control points, got {}",
            points.len()
        )));
    }
    for (i, point) in points.iter().enumerate() {
        let w = weight(point);
        if !w.is_finite() || w.abs() < KNOT_TOLERANCE {
            return Err(Error::degenerate(format!(
                "{what} control point {i} has weight {w}, which must be non-zero"
            )));
        }
    }
    Ok(())
}

// --- 3D curves --------------------------------------------------------------

/// Upstream's `BezierCurve` — a rational Bézier curve over `[0, 1]`.
///
/// Each control point is `(x, y, z, weight)`, not a homogeneous point; see the
/// module header.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BezierCurve {
    pub ctrl_points: Arc<Vec<Vec4>>,
    /// `Stride` — the segment count the discretizer uses. `None` defers to
    /// [`DiscretizeCtx`](crate::algo::discretize::DiscretizeCtx), as every
    /// other density in the port does.
    pub stride: Option<u32>,
    /// `Width`, carried for the wireframe renderer.
    pub width: u32,
}

impl BezierCurve {
    /// A curve through Cartesian control points, every weight 1 — the
    /// non-rational case, and the one plant models use.
    pub fn new(ctrl_points: Vec<Point3>) -> Self {
        Self::rational(
            ctrl_points
                .into_iter()
                .map(|p| Vec4::new(p.x, p.y, p.z, 1.0))
                .collect(),
        )
    }

    /// A curve over weighted control points `(x, y, z, weight)`.
    pub fn rational(ctrl_points: Vec<Vec4>) -> Self {
        Self {
            ctrl_points: Arc::new(ctrl_points),
            stride: None,
            width: DEFAULT_WIDTH,
        }
    }

    /// Pins the discretisation density, as upstream's `Stride` field does.
    pub fn with_stride(mut self, stride: u32) -> Self {
        self.stride = Some(stride);
        self
    }

    /// `getDegree()` — one less than the number of control points.
    pub fn degree(&self) -> usize {
        self.ctrl_points.len().saturating_sub(1)
    }

    pub fn is_valid(&self) -> Result<()> {
        validate_ctrl_points(&self.ctrl_points, |p| p.w, "a Bézier curve")
    }

    fn homogeneous(&self) -> Vec<Homogeneous> {
        self.ctrl_points.iter().map(to_homogeneous_3d).collect()
    }

    /// `getPointAt(u)` for `u` in `[0, 1]`.
    pub fn eval(&self, u: Real) -> Result<Point3> {
        self.is_valid()?;
        Ok(narrow_point3(project(de_casteljau(
            &self.homogeneous(),
            clamp_unit(u),
        ))))
    }

    /// `getTangentAt(u)` — the first derivative, by the quotient rule.
    pub fn tangent(&self, u: Real) -> Result<Vec3> {
        Ok(narrow_vec3(self.derivative(u, 1)?))
    }

    /// `getDerivativeAt(u, d)`.
    pub fn derivative(&self, u: Real, d: usize) -> Result<nalgebra::Vector3<f64>> {
        self.is_valid()?;
        Ok(bezier_derivatives(&self.homogeneous(), clamp_unit(u), d)[d])
    }

    /// `getNormalAt(u)` — the principal normal, zero where the curve is
    /// locally straight.
    pub fn normal(&self, u: Real) -> Result<Vec3> {
        self.is_valid()?;
        let ders = bezier_derivatives(&self.homogeneous(), clamp_unit(u), 2);
        Ok(narrow_vec3(principal_normal(ders[1], ders[2])))
    }

    /// The same curve one degree higher, control point for control point.
    ///
    /// Degree elevation is value-preserving: the elevated curve evaluates to
    /// the same point at every `u`. `tests/analytic.rs` asserts that, which is
    /// the cheapest available check that de Casteljau is right — a wrong
    /// evaluator almost never agrees with itself across two control nets.
    pub fn elevated(&self) -> Self {
        Self {
            ctrl_points: Arc::new(elevate(&self.ctrl_points)),
            stride: self.stride,
            width: self.width,
        }
    }
}

/// Upstream's `NurbsCurve` — a rational B-spline curve.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NurbsCurve {
    pub ctrl_points: Arc<Vec<Vec4>>,
    pub degree: usize,
    pub knots: Arc<Vec<Real>>,
    pub stride: Option<u32>,
    pub width: u32,
}

impl NurbsCurve {
    /// A curve with the default clamped uniform knot vector, as upstream's
    /// builder produces when no knot list is given.
    pub fn new(ctrl_points: Vec<Point3>, degree: usize) -> Self {
        Self::rational(
            ctrl_points
                .into_iter()
                .map(|p| Vec4::new(p.x, p.y, p.z, 1.0))
                .collect(),
            degree,
        )
    }

    /// As [`NurbsCurve::new`], over weighted control points.
    pub fn rational(ctrl_points: Vec<Vec4>, degree: usize) -> Self {
        let degree = degree.min(ctrl_points.len().saturating_sub(1)).max(1);
        let knots = default_knots(ctrl_points.len(), degree);
        Self {
            ctrl_points: Arc::new(ctrl_points),
            degree,
            knots: Arc::new(knots),
            stride: None,
            width: DEFAULT_WIDTH,
        }
    }

    /// A curve over an explicit knot vector, validated by [`validate_knots`].
    pub fn with_knots(ctrl_points: Vec<Vec4>, degree: usize, knots: Vec<Real>) -> Result<Self> {
        validate_ctrl_points(&ctrl_points, |p| p.w, "a NURBS curve")?;
        validate_knots(&knots, ctrl_points.len(), degree)?;
        Ok(Self {
            ctrl_points: Arc::new(ctrl_points),
            degree,
            knots: Arc::new(knots),
            stride: None,
            width: DEFAULT_WIDTH,
        })
    }

    pub fn with_stride(mut self, stride: u32) -> Self {
        self.stride = Some(stride);
        self
    }

    /// `getFirstKnot()`.
    pub fn first_knot(&self) -> Real {
        self.knots[0]
    }

    /// `getLastKnot()`.
    pub fn last_knot(&self) -> Real {
        self.knots[self.knots.len() - 1]
    }

    pub fn is_valid(&self) -> Result<()> {
        validate_ctrl_points(&self.ctrl_points, |p| p.w, "a NURBS curve")?;
        validate_knots(&self.knots, self.ctrl_points.len(), self.degree)
    }

    fn homogeneous(&self) -> Vec<Homogeneous> {
        self.ctrl_points.iter().map(to_homogeneous_3d).collect()
    }

    /// `getPointAt(u)`.
    pub fn eval(&self, u: Real) -> Result<Point3> {
        self.is_valid()?;
        let knots = widen(&self.knots);
        let u = clamp(u as f64, knots[0], knots[knots.len() - 1]);
        Ok(narrow_point3(project(nurbs_point(
            &self.homogeneous(),
            self.degree,
            &knots,
            u,
        ))))
    }

    /// `getTangentAt(u)` — `getDerivativeAt(u, 1)`.
    pub fn tangent(&self, u: Real) -> Result<Vec3> {
        Ok(narrow_vec3(self.derivative(u, 1)?))
    }

    /// `getNormalAt(u)` — the principal normal, zero where the curve is
    /// locally straight.
    pub fn normal(&self, u: Real) -> Result<Vec3> {
        Ok(narrow_vec3(principal_normal(
            self.derivative(u, 1)?,
            self.derivative(u, 2)?,
        )))
    }

    /// `getDerivativeAt(u, d)`. Beyond the curve's degree every derivative is
    /// zero, as upstream returns.
    pub fn derivative(&self, u: Real, d: usize) -> Result<nalgebra::Vector3<f64>> {
        self.is_valid()?;
        if d > self.degree {
            return Ok(nalgebra::Vector3::zeros());
        }
        let knots = widen(&self.knots);
        let u = clamp(u as f64, knots[0], knots[knots.len() - 1]);
        let ders_h = nurbs_derivatives_h(&self.homogeneous(), self.degree, &knots, u, d);
        Ok(rational_derivatives(&ders_h)[d])
    }
}

// --- 2D curves --------------------------------------------------------------

/// Upstream's `BezierCurve2D`. Control points are `(x, y, weight)`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BezierCurve2D {
    pub ctrl_points: Arc<Vec<Vec3>>,
    pub stride: Option<u32>,
    pub width: u32,
}

impl BezierCurve2D {
    pub fn new(ctrl_points: Vec<Point2>) -> Self {
        Self::rational(
            ctrl_points
                .into_iter()
                .map(|p| Vec3::new(p.x, p.y, 1.0))
                .collect(),
        )
    }

    pub fn rational(ctrl_points: Vec<Vec3>) -> Self {
        Self {
            ctrl_points: Arc::new(ctrl_points),
            stride: None,
            width: DEFAULT_WIDTH,
        }
    }

    pub fn with_stride(mut self, stride: u32) -> Self {
        self.stride = Some(stride);
        self
    }

    pub fn degree(&self) -> usize {
        self.ctrl_points.len().saturating_sub(1)
    }

    pub fn is_valid(&self) -> Result<()> {
        validate_ctrl_points(&self.ctrl_points, |p| p.z, "a 2D Bézier curve")
    }

    fn homogeneous(&self) -> Vec<Homogeneous> {
        self.ctrl_points.iter().map(to_homogeneous_2d).collect()
    }

    pub fn eval(&self, u: Real) -> Result<Point2> {
        self.is_valid()?;
        Ok(narrow_point2(project(de_casteljau(
            &self.homogeneous(),
            clamp_unit(u),
        ))))
    }

    pub fn tangent(&self, u: Real) -> Result<Vec2> {
        self.is_valid()?;
        Ok(narrow_vec2(
            bezier_derivatives(&self.homogeneous(), clamp_unit(u), 1)[1],
        ))
    }

    /// The same curve one degree higher; see [`BezierCurve::elevated`].
    pub fn elevated(&self) -> Self {
        Self {
            ctrl_points: Arc::new(elevate(&self.ctrl_points)),
            stride: self.stride,
            width: self.width,
        }
    }
}

/// Upstream's `NurbsCurve2D`. Control points are `(x, y, weight)`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NurbsCurve2D {
    pub ctrl_points: Arc<Vec<Vec3>>,
    pub degree: usize,
    pub knots: Arc<Vec<Real>>,
    pub stride: Option<u32>,
    pub width: u32,
}

impl NurbsCurve2D {
    pub fn new(ctrl_points: Vec<Point2>, degree: usize) -> Self {
        Self::rational(
            ctrl_points
                .into_iter()
                .map(|p| Vec3::new(p.x, p.y, 1.0))
                .collect(),
            degree,
        )
    }

    pub fn rational(ctrl_points: Vec<Vec3>, degree: usize) -> Self {
        let degree = degree.min(ctrl_points.len().saturating_sub(1)).max(1);
        let knots = default_knots(ctrl_points.len(), degree);
        Self {
            ctrl_points: Arc::new(ctrl_points),
            degree,
            knots: Arc::new(knots),
            stride: None,
            width: DEFAULT_WIDTH,
        }
    }

    pub fn with_knots(ctrl_points: Vec<Vec3>, degree: usize, knots: Vec<Real>) -> Result<Self> {
        validate_ctrl_points(&ctrl_points, |p| p.z, "a 2D NURBS curve")?;
        validate_knots(&knots, ctrl_points.len(), degree)?;
        Ok(Self {
            ctrl_points: Arc::new(ctrl_points),
            degree,
            knots: Arc::new(knots),
            stride: None,
            width: DEFAULT_WIDTH,
        })
    }

    pub fn with_stride(mut self, stride: u32) -> Self {
        self.stride = Some(stride);
        self
    }

    /// The exact circle of radius `r`: the standard rational quadratic, nine
    /// control points over four 90° arcs with corner weights `√2/2`.
    ///
    /// This is the `Profile`-style helper T8.6 asks for. A `Polyline2D::circle`
    /// cross-section is an *n*-gon that approaches a circle from the inside;
    /// this one **is** a circle, so a stem swept along it has the radius it was
    /// asked for at every parameter rather than only at the sample points.
    pub fn circle(radius: Real) -> Self {
        let corner = std::f32::consts::FRAC_1_SQRT_2;
        let r = radius;
        let ctrl_points = vec![
            Vec3::new(r, 0.0, 1.0),
            Vec3::new(r, r, corner),
            Vec3::new(0.0, r, 1.0),
            Vec3::new(-r, r, corner),
            Vec3::new(-r, 0.0, 1.0),
            Vec3::new(-r, -r, corner),
            Vec3::new(0.0, -r, 1.0),
            Vec3::new(r, -r, corner),
            Vec3::new(r, 0.0, 1.0),
        ];
        let knots = vec![
            0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0,
        ];
        Self {
            ctrl_points: Arc::new(ctrl_points),
            degree: 2,
            knots: Arc::new(knots),
            stride: None,
            width: DEFAULT_WIDTH,
        }
    }

    pub fn first_knot(&self) -> Real {
        self.knots[0]
    }

    pub fn last_knot(&self) -> Real {
        self.knots[self.knots.len() - 1]
    }

    pub fn is_valid(&self) -> Result<()> {
        validate_ctrl_points(&self.ctrl_points, |p| p.z, "a 2D NURBS curve")?;
        validate_knots(&self.knots, self.ctrl_points.len(), self.degree)
    }

    fn homogeneous(&self) -> Vec<Homogeneous> {
        self.ctrl_points.iter().map(to_homogeneous_2d).collect()
    }

    pub fn eval(&self, u: Real) -> Result<Point2> {
        self.is_valid()?;
        let knots = widen(&self.knots);
        let u = clamp(u as f64, knots[0], knots[knots.len() - 1]);
        Ok(narrow_point2(project(nurbs_point(
            &self.homogeneous(),
            self.degree,
            &knots,
            u,
        ))))
    }

    pub fn tangent(&self, u: Real) -> Result<Vec2> {
        self.is_valid()?;
        let knots = widen(&self.knots);
        let u = clamp(u as f64, knots[0], knots[knots.len() - 1]);
        if self.degree < 1 {
            return Ok(Vec2::zeros());
        }
        let ders_h = nurbs_derivatives_h(&self.homogeneous(), self.degree, &knots, u, 1);
        Ok(narrow_vec2(rational_derivatives(&ders_h)[1]))
    }
}

// --- Shared helpers ---------------------------------------------------------

/// Derivatives `0 ..= d` of a rational Bézier at `u`: evaluate the net, then
/// its hodograph, then the hodograph of that, and divide the weights out.
fn bezier_derivatives(ctrl: &[Homogeneous], u: f64, d: usize) -> Vec<nalgebra::Vector3<f64>> {
    let mut ders_h = Vec::with_capacity(d + 1);
    let mut net = ctrl.to_vec();
    for _ in 0..=d {
        if net.is_empty() {
            ders_h.push(Homogeneous::zeros());
            continue;
        }
        ders_h.push(de_casteljau(&net, u));
        net = if net.len() >= 2 {
            hodograph(&net)
        } else {
            Vec::new()
        };
    }
    rational_derivatives(&ders_h)
}

/// `(t × n) × t / |t|⁴` — the principal normal of a curve from its first two
/// derivatives, upstream's `getNormalAt`. Zero where the curve is locally
/// straight, which is what the caller tests for.
pub(crate) fn principal_normal(
    tangent: nalgebra::Vector3<f64>,
    second: nalgebra::Vector3<f64>,
) -> nalgebra::Vector3<f64> {
    let norm = tangent.norm();
    if norm < 1e-12 {
        return nalgebra::Vector3::zeros();
    }
    tangent.cross(&second).cross(&tangent) / norm.powi(4)
}

/// Degree elevation on control points that carry a weight: elevate the
/// homogeneous net, then divide the weight back out.
fn elevate<T: WeightedPoint>(ctrl: &[T]) -> Vec<T> {
    let homogeneous: Vec<Homogeneous> = ctrl.iter().map(T::to_homogeneous).collect();
    let n = homogeneous.len().saturating_sub(1);
    let mut elevated = Vec::with_capacity(n + 2);
    elevated.push(homogeneous[0]);
    for i in 1..=n {
        let alpha = i as f64 / (n + 1) as f64;
        elevated.push(homogeneous[i - 1] * alpha + homogeneous[i] * (1.0 - alpha));
    }
    elevated.push(homogeneous[n]);
    elevated.iter().map(|h| T::from_homogeneous(*h)).collect()
}

/// The two control-point layouts, so degree elevation is written once.
trait WeightedPoint: Copy {
    fn to_homogeneous(&self) -> Homogeneous;
    fn from_homogeneous(h: Homogeneous) -> Self;
}

impl WeightedPoint for Vec4 {
    fn to_homogeneous(&self) -> Homogeneous {
        to_homogeneous_3d(self)
    }

    fn from_homogeneous(h: Homogeneous) -> Self {
        let p = project(h);
        Vec4::new(p.x as Real, p.y as Real, p.z as Real, h.w as Real)
    }
}

impl WeightedPoint for Vec3 {
    fn to_homogeneous(&self) -> Homogeneous {
        to_homogeneous_2d(self)
    }

    fn from_homogeneous(h: Homogeneous) -> Self {
        let p = project(h);
        Vec3::new(p.x as Real, p.y as Real, h.w as Real)
    }
}

fn clamp(u: f64, low: f64, high: f64) -> f64 {
    u.clamp(low, high)
}

/// A Bézier's domain is `[0, 1]`; upstream asserts it and the port clamps,
/// because a sampling loop that lands on `1 + 1e-7` should give the end point
/// rather than an extrapolation.
fn clamp_unit(u: Real) -> f64 {
    (u as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn the_default_knot_vector_is_clamped_and_uniform() {
        let knots = default_knots(5, 3);
        assert_eq!(knots, vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0]);
        assert!(validate_knots(&knots, 5, 3).is_ok());
    }

    #[test]
    fn a_bezier_knot_vector_has_no_interior_knots() {
        assert_eq!(default_knots(4, 3), vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn a_wrong_length_knot_vector_is_rejected() {
        let error = validate_knots(&[0.0, 0.0, 1.0, 1.0], 5, 3).unwrap_err();
        assert!(matches!(error, Error::BadKnotVector(_)));
    }

    #[test]
    fn a_non_monotone_knot_vector_is_rejected() {
        let knots = vec![0.0, 0.0, 0.0, 0.6, 0.4, 1.0, 1.0, 1.0];
        assert!(matches!(
            validate_knots(&knots, 5, 2),
            Err(Error::BadKnotVector(_))
        ));
    }

    #[test]
    fn an_unclamped_knot_vector_is_rejected() {
        let knots = vec![0.0, 0.1, 0.2, 0.4, 0.6, 0.8, 0.9, 1.0];
        assert!(matches!(
            validate_knots(&knots, 5, 2),
            Err(Error::BadKnotVector(_))
        ));
    }

    #[test]
    fn find_span_stays_in_range() {
        let knots = widen(&default_knots(6, 3));
        for i in 0..=100 {
            let u = i as f64 / 100.0;
            let span = find_span(u, 3, &knots);
            assert!((3..knots.len() - 4).contains(&span), "span {span} for u {u}");
        }
    }

    #[test]
    fn basis_functions_are_a_partition_of_unity() {
        let knots = widen(&default_knots(7, 3));
        for i in 0..=50 {
            let u = i as f64 / 50.0;
            let span = find_span(u, 3, &knots);
            let sum: f64 = basis_functions(span, u, 3, &knots).iter().sum();
            assert!((sum - 1.0).abs() < 1e-12, "basis sum {sum} at u {u}");
        }
    }

    #[test]
    fn basis_derivatives_agree_with_finite_differences() {
        let knots = widen(&default_knots(6, 3));
        let h = 1e-6;
        for step in 1..20 {
            let u = step as f64 / 20.0;
            let span = find_span(u, 3, &knots);
            let ders = basis_derivatives(1, u, span, 3, &knots);
            let ahead = basis_functions(span, u + h, 3, &knots);
            let behind = basis_functions(span, u - h, 3, &knots);
            for j in 0..=3 {
                let numeric = (ahead[j] - behind[j]) / (2.0 * h);
                assert!(
                    (ders[1][j] - numeric).abs() < 1e-4,
                    "d/du N_{j} at {u}: analytic {} numeric {numeric}",
                    ders[1][j]
                );
            }
        }
    }

    #[test]
    fn a_bezier_interpolates_its_end_points() {
        let curve = BezierCurve::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ]);
        assert_relative_eq!(curve.eval(0.0).unwrap(), Point3::new(0.0, 0.0, 0.0));
        assert_relative_eq!(curve.eval(1.0).unwrap(), Point3::new(2.0, 0.0, 0.0));
        // The apex of a symmetric quadratic sits half way to the middle point.
        assert_relative_eq!(
            curve.eval(0.5).unwrap(),
            Point3::new(1.0, 1.0, 0.0),
            epsilon = 1e-6
        );
    }

    #[test]
    fn a_bezier_tangent_matches_the_hodograph_at_the_ends() {
        let curve = BezierCurve::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 2.0, 0.0),
            Point3::new(2.0, 0.0, 0.0),
        ]);
        // For a degree-2 curve, C'(0) = 2 (P1 - P0) and C'(1) = 2 (P2 - P1).
        assert_relative_eq!(
            curve.tangent(0.0).unwrap(),
            Vec3::new(2.0, 4.0, 0.0),
            epsilon = 1e-5
        );
        assert_relative_eq!(
            curve.tangent(1.0).unwrap(),
            Vec3::new(2.0, -4.0, 0.0),
            epsilon = 1e-5
        );
    }

    #[test]
    fn a_nurbs_line_is_a_line() {
        let curve = NurbsCurve::new(
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(3.0, 0.0, 0.0),
            ],
            3,
        );
        for i in 0..=10 {
            let u = i as Real / 10.0;
            let point = curve.eval(u).unwrap();
            assert_relative_eq!(point.y, 0.0, epsilon = 1e-6);
            assert_relative_eq!(point.x, 3.0 * u, epsilon = 1e-5);
        }
    }

    #[test]
    fn a_nurbs_circle_has_unit_radius_everywhere() {
        let circle = NurbsCurve2D::circle(1.0);
        for i in 0..=256 {
            let u = i as Real / 256.0;
            let point = circle.eval(u).unwrap();
            assert!(
                (point.coords.norm() - 1.0).abs() < 1e-5,
                "radius {} at u {u}",
                point.coords.norm()
            );
        }
    }

    #[test]
    fn a_nurbs_circle_tangent_is_perpendicular_to_its_radius() {
        let circle = NurbsCurve2D::circle(2.0);
        for i in 0..64 {
            let u = i as Real / 64.0;
            let point = circle.eval(u).unwrap();
            let tangent = circle.tangent(u).unwrap().normalize();
            assert!(
                point.coords.normalize().dot(&tangent).abs() < 1e-4,
                "tangent is not perpendicular at u {u}"
            );
        }
    }

    #[test]
    fn degree_elevation_preserves_every_value() {
        let curve = BezierCurve::new(vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 3.0, -1.0),
            Point3::new(2.0, -1.0, 2.0),
            Point3::new(4.0, 0.0, 0.0),
        ]);
        let elevated = curve.elevated();
        assert_eq!(elevated.degree(), curve.degree() + 1);
        for i in 0..=64 {
            let u = i as Real / 64.0;
            assert_relative_eq!(
                curve.eval(u).unwrap(),
                elevated.eval(u).unwrap(),
                epsilon = 1e-5
            );
        }
    }

    #[test]
    fn a_zero_weight_control_point_is_degenerate() {
        let curve = BezierCurve::rational(vec![
            Vec4::new(0.0, 0.0, 0.0, 1.0),
            Vec4::new(1.0, 0.0, 0.0, 0.0),
        ]);
        assert!(matches!(curve.eval(0.5), Err(Error::DegenerateGeometry(_))));
    }

    #[test]
    fn one_control_point_is_not_a_curve() {
        let curve = BezierCurve::new(vec![Point3::origin()]);
        assert!(matches!(curve.eval(0.0), Err(Error::DegenerateGeometry(_))));
    }

    /// An empty control net is a caller's mistake, and the answer to it is an
    /// error from `is_valid` — not an arithmetic overflow while the default
    /// knot vector is being sized.
    #[test]
    fn an_empty_control_net_is_rejected_not_a_panic() {
        for degree in [1, 3, 7] {
            let _ = default_knots(0, degree);
            let curve = NurbsCurve::rational(Vec::new(), degree);
            assert!(matches!(
                curve.eval(0.5),
                Err(Error::DegenerateGeometry(_))
            ));
        }
    }
}
