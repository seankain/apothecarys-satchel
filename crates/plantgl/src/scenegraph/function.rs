//! Sampled 1D functions.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/function/function.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.

use crate::error::{Error, Result};
use crate::math::{Real, EPSILON};
use crate::scenegraph::curve::{Curve2D, ParametricCurve};

/// Upstream's `QuantisedFunction` — a function of one variable stored as
/// evenly spaced samples and read back by linear interpolation.
///
/// Two jobs in this crate. It is the radius profile an
/// [`Extrusion`](crate::scenegraph::primitive::Extrusion) tapers by — a stem
/// that narrows from 1.0 to 0.0 along its axis is one of these — and it is the
/// parameter-to-arc-length mapping a swept surface's texture coordinates run
/// on ([`ParametricCurve::u_to_arc_length_mapping`]).
///
/// # The domain is clamped
///
/// Reading outside `[first_x, last_x]` returns the nearest end sample rather
/// than extrapolating. Upstream has a `__clamped` flag whose sense is inverted
/// — `clamped == false` is what makes it clamp, and `true` lets the read run
/// on and report an error — and nothing in the port wants the unclamped
/// behaviour, so the field is not carried.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QuantisedFunction {
    values: Vec<Real>,
    first_x: Real,
    last_x: Real,
}

impl QuantisedFunction {
    /// The default number of samples upstream quantises a curve at, relative to
    /// its stride: `QuantisedFunction(points, 5 * stride)` in
    /// `LineicModel::getUToArcLengthMapping`.
    pub const SAMPLES_PER_SEGMENT: u32 = 5;

    /// Builds from samples taken at evenly spaced `x` across `[first_x, last_x]`.
    pub fn from_samples(values: Vec<Real>, first_x: Real, last_x: Real) -> Result<Self> {
        if values.len() < 2 {
            return Err(Error::degenerate(
                "a quantised function needs at least 2 samples",
            ));
        }
        if !(first_x.is_finite() && last_x.is_finite()) || last_x - first_x <= EPSILON {
            return Err(Error::degenerate(format!(
                "a quantised function needs an increasing domain, got [{first_x}, {last_x}]"
            )));
        }
        if let Some((i, value)) = values.iter().enumerate().find(|(_, v)| !v.is_finite()) {
            return Err(Error::degenerate(format!("sample {i} is {value}")));
        }
        Ok(Self {
            values,
            first_x,
            last_x,
        })
    }

    /// `QuantisedFunction::build(curve, sampling)` — samples a 2D curve read as
    /// a graph `y = f(x)`.
    ///
    /// The curve must be a function of `x`: strictly increasing in `x` along
    /// its parameter, which upstream's `check()` tests and which the port
    /// reports as [`Error::DegenerateGeometry`] rather than warning and
    /// returning a silently wrong table. Each sample is found by bisecting the
    /// parameter for the wanted `x`, as upstream's `_computeValue` does.
    pub fn from_curve(curve: &Curve2D, samples: u32) -> Result<Self> {
        let samples = samples.max(2) as usize;
        let control = curve.discretize(samples as u32)?;
        for pair in control.windows(2) {
            if pair[1].x <= pair[0].x {
                return Err(Error::degenerate(
                    "a quantised function's curve must be strictly increasing in x",
                ));
            }
        }
        let first_x = curve.eval(curve.first_knot())?.x;
        let last_x = curve.eval(curve.last_knot())?.x;
        let extent = last_x - first_x;
        let values = (0..samples)
            .map(|i| {
                let x = first_x + extent * i as Real / (samples - 1) as Real;
                solve_for_x(curve, x)
            })
            .collect::<Result<Vec<Real>>>()?;
        Self::from_samples(values, first_x, last_x)
    }

    /// A constant function over `[0, 1]` — the identity profile, which scales
    /// nothing.
    pub fn constant(value: Real) -> Self {
        Self {
            values: vec![value, value],
            first_x: 0.0,
            last_x: 1.0,
        }
    }

    /// A function that runs linearly from `start` to `end` over `[0, 1]` — the
    /// radius profile of a cone.
    pub fn ramp(start: Real, end: Real) -> Self {
        Self {
            values: vec![start, end],
            first_x: 0.0,
            last_x: 1.0,
        }
    }

    pub fn first_x(&self) -> Real {
        self.first_x
    }

    pub fn last_x(&self) -> Real {
        self.last_x
    }

    pub fn values(&self) -> &[Real] {
        &self.values
    }

    /// `getValue(x)` — linear interpolation between the two nearest samples,
    /// with `x` clamped into the domain.
    pub fn value(&self, x: Real) -> Real {
        let x = x.clamp(self.first_x, self.last_x);
        let last = self.values.len() - 1;
        let index = last as Real * (x - self.first_x) / (self.last_x - self.first_x);
        let floor = index.floor();
        let i = (floor as usize).min(last);
        if i == last {
            return self.values[last];
        }
        let t = index - floor;
        if t.abs() < EPSILON {
            return self.values[i];
        }
        self.values[i] * (1.0 - t) + self.values[i + 1] * t
    }

    /// `isIncreasing(strictly)`.
    pub fn is_increasing(&self, strictly: bool) -> bool {
        self.values
            .windows(2)
            .all(|w| if strictly { w[0] < w[1] } else { w[0] <= w[1] })
    }

    /// `isDecreasing(strictly)`.
    pub fn is_decreasing(&self, strictly: bool) -> bool {
        self.values
            .windows(2)
            .all(|w| if strictly { w[0] > w[1] } else { w[0] >= w[1] })
    }

    /// `isMonotonous(strictly)`.
    pub fn is_monotonic(&self, strictly: bool) -> bool {
        self.is_increasing(strictly) || self.is_decreasing(strictly)
    }
}

/// `_computeValue` — bisect the curve's parameter until its `x` reaches the
/// wanted one, then take that point's `y`.
fn solve_for_x(curve: &Curve2D, x: Real) -> Result<Real> {
    let (mut low, mut high) = (curve.first_knot(), curve.last_knot());
    let first = curve.eval(low)?;
    if (x - first.x).abs() < EPSILON {
        return Ok(first.y);
    }
    let last = curve.eval(high)?;
    if (x - last.x).abs() < EPSILON {
        return Ok(last.y);
    }
    // The curve is strictly increasing in x, so 64 halvings of the parameter
    // range reach f32's resolution whatever the domain; upstream loops on the
    // error alone and can spin forever on a curve that does not converge.
    let mut point = first;
    for _ in 0..64 {
        let mid = (low + high) / 2.0;
        point = curve.eval(mid)?;
        if (point.x - x).abs() <= EPSILON {
            break;
        }
        if point.x > x {
            high = mid;
        } else {
            low = mid;
        }
    }
    Ok(point.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Point2;
    use crate::scenegraph::curve::Polyline2D;
    use approx::assert_relative_eq;

    #[test]
    fn a_ramp_interpolates_linearly() {
        let ramp = QuantisedFunction::ramp(1.0, 0.0);
        assert_relative_eq!(ramp.value(0.0), 1.0);
        assert_relative_eq!(ramp.value(0.25), 0.75, epsilon = 1e-6);
        assert_relative_eq!(ramp.value(1.0), 0.0);
    }

    #[test]
    fn the_domain_is_clamped_at_both_ends() {
        let ramp = QuantisedFunction::ramp(2.0, 5.0);
        assert_relative_eq!(ramp.value(-10.0), 2.0);
        assert_relative_eq!(ramp.value(10.0), 5.0);
    }

    #[test]
    fn samples_are_read_back_exactly() {
        let f = QuantisedFunction::from_samples(vec![0.0, 1.0, 4.0, 9.0], 0.0, 3.0).unwrap();
        for (i, expected) in [0.0, 1.0, 4.0, 9.0].iter().enumerate() {
            assert_relative_eq!(f.value(i as Real), *expected, epsilon = 1e-6);
        }
        // And between them, the chord.
        assert_relative_eq!(f.value(2.5), 6.5, epsilon = 1e-6);
    }

    #[test]
    fn monotonicity_is_reported() {
        let up = QuantisedFunction::from_samples(vec![0.0, 1.0, 1.0, 2.0], 0.0, 1.0).unwrap();
        assert!(up.is_increasing(false));
        assert!(!up.is_increasing(true));
        assert!(up.is_monotonic(false));

        let wobble = QuantisedFunction::from_samples(vec![0.0, 2.0, 1.0], 0.0, 1.0).unwrap();
        assert!(!wobble.is_monotonic(false));
    }

    #[test]
    fn a_function_needs_two_samples_and_a_real_domain() {
        assert!(QuantisedFunction::from_samples(vec![1.0], 0.0, 1.0).is_err());
        assert!(QuantisedFunction::from_samples(vec![1.0, 2.0], 1.0, 1.0).is_err());
        assert!(QuantisedFunction::from_samples(vec![1.0, Real::NAN], 0.0, 1.0).is_err());
    }

    #[test]
    fn a_curve_becomes_the_function_it_draws() {
        // y = x² sampled as a polyline, read back as a function of x.
        let curve = Curve2D::from(Polyline2D::new(
            (0..=10)
                .map(|i| {
                    let x = i as Real / 10.0;
                    Point2::new(x, x * x)
                })
                .collect(),
        ));
        let f = QuantisedFunction::from_curve(&curve, 21).unwrap();
        assert_relative_eq!(f.first_x(), 0.0, epsilon = 1e-6);
        assert_relative_eq!(f.last_x(), 1.0, epsilon = 1e-6);
        for i in 0..=10 {
            let x = i as Real / 10.0;
            assert!(
                (f.value(x) - x * x).abs() < 2e-2,
                "f({x}) = {} against {}",
                f.value(x),
                x * x
            );
        }
    }

    #[test]
    fn a_curve_that_doubles_back_is_not_a_function() {
        let curve = Curve2D::from(Polyline2D::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(0.5, 2.0),
        ]));
        assert!(matches!(
            QuantisedFunction::from_curve(&curve, 8),
            Err(Error::DegenerateGeometry(_))
        ));
    }
}
