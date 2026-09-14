//! Guides — the curves a turtle's forward moves follow.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/modelling/turtlepath.{h,cpp}` and
//! `Turtle::_applyGuide`/`Turtle::_ajustToGuide` in `turtle.cpp`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # What a guide does
//!
//! `setGuide(curve, length)` says "the next `length` of turtle travel follows
//! this curve". Each forward move then consumes its share of the curve's arc
//! length, and the turtle is turned by the angle the curve turned through over
//! that share — so the axis the turtle draws *is* the curve, sampled at
//! whatever step the L-system happens to use. A drooping vine or an arching
//! frond is one guide rather than a hand-tuned sequence of turns.
//!
//! The turtle is turned rather than teleported, which is what keeps the branch
//! frame — and therefore every organ hung off it — consistent with the path.
//!
//! Two kinds, as upstream: a 2D guide, which bends the turtle within one plane
//! of its own frame, and a 3D guide, which carries a reference frame along the
//! curve and reproduces its rotation in the turtle's frame.

use crate::error::{Error, Result};
use crate::math::{angle2, Frame, Point2, Point3, Real, Vec2, Vec3, EPSILON};
use crate::modelling::tropism::axis_rotation;
use crate::scenegraph::curve::{Curve2DRef, Curve3DRef, ParametricCurve};
use crate::scenegraph::function::QuantisedFunction;

/// The state every guide carries — upstream's `TurtlePath` base.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PathState {
    /// `__totalLength` — the turtle-space length the guide is stretched over.
    pub total_length: Real,
    /// `__actualLength` — the curve's own length.
    pub actual_length: Real,
    /// `__scale` — `total_length / actual_length`, the factor that converts a
    /// distance measured on the curve into one the turtle travels.
    pub scale: Real,
    /// `__arclengthParam` — normalised arc length to curve parameter.
    pub arc_length_param: QuantisedFunction,
    /// `__actualT` — how much of the guide has been consumed, in `[0, 1]`.
    pub actual_t: Real,
}

impl PathState {
    fn new(total_length: Real, actual_length: Real, arc_length_param: QuantisedFunction) -> Self {
        Self {
            total_length,
            actual_length,
            scale: total_length / actual_length,
            arc_length_param,
            actual_t: 0.0,
        }
    }
}

/// A guide over a 2D curve — upstream's `Turtle2DPath`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Guide2D {
    pub state: PathState,
    pub path: Curve2DRef,
    /// `__orientation` — whether the curve's first heading is `Y` (`true`) or
    /// `X` (`false`), which decides whether the turtle bends about its `up` or
    /// about its `left`.
    pub orientation: bool,
    /// `__ccw` — mirrors the sense of every deviation.
    pub ccw: bool,
    last_position: Point2,
    last_heading: Vec2,
}

/// A guide over a 3D curve — upstream's `Turtle3DPath`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Guide3D {
    pub state: PathState,
    pub path: Curve3DRef,
    last_position: Point3,
    /// The reference frame carried along the curve. Rotating it by the same
    /// amount as the turtle's frame is what keeps a roll about the heading
    /// meaningful while a 3D guide is active.
    last_heading: Vec3,
    last_up: Vec3,
    last_left: Vec3,
}

/// Either kind of guide — upstream's `TurtlePathPtr`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TurtlePath {
    Path2D(Box<Guide2D>),
    Path3D(Box<Guide3D>),
}

impl Guide2D {
    /// `Turtle2DPath(curve, totalLength, actualLength, orientation, ccw, …)`.
    ///
    /// `samples` is the density the curve's length and arc-length mapping are
    /// measured at, which upstream reads off the curve's own stride.
    pub fn new(
        path: Curve2DRef,
        total_length: Real,
        orientation: bool,
        ccw: bool,
        samples: u32,
    ) -> Result<Self> {
        let actual_length = path.length(samples)?;
        if actual_length <= EPSILON {
            return Err(Error::degenerate("a guide curve of zero length"));
        }
        let arc_length_param = path.arc_length_to_u_mapping(samples)?;
        let last_position = path.eval(path.first_knot())?;
        let last_heading = match (orientation, ccw) {
            (false, false) => Vec2::new(1.0, 0.0),
            (true, false) => Vec2::new(0.0, 1.0),
            (false, true) => Vec2::new(-1.0, 0.0),
            (true, true) => Vec2::new(0.0, -1.0),
        };
        Ok(Self {
            state: PathState::new(total_length, actual_length, arc_length_param),
            path,
            orientation,
            ccw,
            last_position,
            last_heading,
        })
    }

    /// `Turtle2DPath::setPosition(t)` — jumps to `t` turtle-space units along
    /// the guide, re-reading the position and heading there.
    pub fn set_position(&mut self, t: Real) -> Result<()> {
        self.state.actual_t = (t / self.state.total_length).clamp(0.0, 1.0);
        let u = self.state.arc_length_param.value(self.state.actual_t);
        self.last_position = self.path.eval(u)?;
        self.last_heading = self
            .path
            .tangent(u)?
            .try_normalize(EPSILON)
            .ok_or_else(|| Error::degenerate("a guide curve with no tangent"))?;
        Ok(())
    }
}

impl Guide3D {
    /// `Turtle3DPath(curve, totalLength, actualLength, …)`.
    pub fn new(path: Curve3DRef, total_length: Real, samples: u32) -> Result<Self> {
        let actual_length = path.length(samples)?;
        if actual_length <= EPSILON {
            return Err(Error::degenerate("a guide curve of zero length"));
        }
        let arc_length_param = path.arc_length_to_u_mapping(samples)?;
        let last_position = path.eval(path.first_knot())?;
        Ok(Self {
            state: PathState::new(total_length, actual_length, arc_length_param),
            path,
            last_position,
            last_heading: Vec3::new(0.0, 0.0, 1.0),
            last_up: Vec3::new(1.0, 0.0, 0.0),
            last_left: Vec3::new(0.0, -1.0, 0.0),
        })
    }

    /// `Turtle3DPath::setPosition(t)`.
    pub fn set_position(&mut self, t: Real) -> Result<()> {
        self.state.actual_t = (t / self.state.total_length).clamp(0.0, 1.0);
        let u = self.state.arc_length_param.value(self.state.actual_t);
        self.last_position = self.path.eval(u)?;
        self.last_heading = self
            .path
            .tangent(u)?
            .try_normalize(EPSILON)
            .ok_or_else(|| Error::degenerate("a guide curve with no tangent"))?;
        self.last_up = self
            .path
            .normal(u)?
            .try_normalize(EPSILON)
            .ok_or_else(|| Error::degenerate("a guide curve with no normal"))?;
        self.last_left = self
            .last_up
            .cross(&self.last_heading)
            .try_normalize(EPSILON)
            .ok_or_else(|| Error::degenerate("a guide curve with a collapsed frame"))?;
        Ok(())
    }

    /// `iRollL`'s effect on a 3D guide: the reference frame on the curve rolls
    /// with the turtle, so an intrinsic roll stays intrinsic.
    pub fn roll(&mut self, radians: Real) {
        if let Some(m) = axis_rotation(&self.last_heading, radians) {
            self.last_up = m * self.last_up;
            self.last_left = m * self.last_left;
        }
    }
}

/// What [`TurtlePath::apply`] decided about the guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideOutcome {
    /// The guide is still active.
    Active,
    /// The guide has been consumed past its end: the caller drops it, as
    /// upstream nulls `__params->guide`.
    Exhausted,
}

impl TurtlePath {
    /// `TurtlePath::is2D()`.
    pub fn is_2d(&self) -> bool {
        matches!(self, TurtlePath::Path2D(_))
    }

    /// How much of the guide has been consumed, in `[0, 1]`.
    pub fn actual_t(&self) -> Real {
        match self {
            TurtlePath::Path2D(g) => g.state.actual_t,
            TurtlePath::Path3D(g) => g.state.actual_t,
        }
    }

    /// `Turtle::setPositionOnGuide(t)`.
    pub fn set_position(&mut self, t: Real) -> Result<()> {
        match self {
            TurtlePath::Path2D(g) => g.set_position(t),
            TurtlePath::Path3D(g) => g.set_position(t),
        }
    }

    /// `Turtle::_applyGuide(l)` — advances along the guide by `*length` and
    /// turns `frame` to match, rewriting `*length` to the distance the turtle
    /// should actually travel.
    ///
    /// A `*length` of zero is upstream's `_ajustToGuide`: no advance, just a
    /// turn onto the local tangent, which is what a forward move does *after*
    /// it lands so the next organ is oriented along the curve.
    pub fn apply(&mut self, frame: &mut Frame, length: &mut Real) -> Result<GuideOutcome> {
        let local_adjustment = length.abs() < EPSILON;
        if self.actual_t() >= 1.0 && !local_adjustment {
            return Ok(GuideOutcome::Exhausted);
        }
        match self {
            TurtlePath::Path2D(guide) => apply_2d(guide, frame, length, local_adjustment)?,
            TurtlePath::Path3D(guide) => apply_3d(guide, frame, length, local_adjustment)?,
        }
        Ok(GuideOutcome::Active)
    }
}

/// The step the guide takes: where it lands on the curve and what that costs
/// the turtle. Shared by the 2D and 3D branches, which upstream writes twice.
struct Step<P, V> {
    tangent: V,
    next_position: P,
    next_t: Real,
}

/// The shared half of `_applyGuide`: how far along the curve this move
/// reaches, and the direction from where the guide was to where it lands.
///
/// Overshooting the end of the guide is not clamped away — the remainder is
/// carried past the last point along the end tangent, which is what makes a
/// too-long forward move leave the curve straight rather than pile up on its
/// last point.
fn step<C>(
    curve: &C,
    state: &PathState,
    last_position: C::Point,
    length: Real,
    local_adjustment: bool,
) -> Result<Step<C::Point, C::Tangent>>
where
    C: ParametricCurve,
    C::Point: std::ops::Sub<C::Point, Output = C::Tangent>
        + std::ops::Add<C::Tangent, Output = C::Point>,
    C::Tangent: NormalisableTangent,
{
    if local_adjustment {
        let u = state.arc_length_param.value(state.actual_t);
        return Ok(Step {
            tangent: curve.tangent(u)?,
            next_position: last_position,
            next_t: state.actual_t,
        });
    }

    let dt = length / state.total_length;
    let mut next = state.actual_t + dt;
    let remainder = next - 1.0;
    if remainder > EPSILON {
        next = 1.0;
    }
    let next_u = state.arc_length_param.value(next);
    let next_position = curve.eval(next_u)?;

    let mut target = next_position;
    if remainder > EPSILON {
        // Past the end: continue along the end tangent for what is left. The
        // offset is measured in the curve's own space, hence `actual_length`.
        let end_tangent = curve.tangent(next_u)?;
        let offset_length = remainder * state.actual_length;
        target = next_position + scaled_unit(end_tangent, offset_length)?;
    }

    Ok(Step {
        tangent: target - last_position,
        next_position,
        next_t: next,
    })
}

/// `v.normalize() * factor` on a tangent of either dimension.
fn scaled_unit<V>(v: V, factor: Real) -> Result<V>
where
    V: NormalisableTangent,
{
    v.unit()
        .map(|unit| unit.scaled(factor))
        .ok_or_else(|| Error::degenerate("a guide curve with no tangent"))
}

/// The operations `_applyGuide` performs on a tangent, in either dimension.
pub trait NormalisableTangent: Sized {
    fn unit(self) -> Option<Self>;
    fn norm_of(&self) -> Real;
    fn scaled(self, factor: Real) -> Self;
}

impl NormalisableTangent for Vec2 {
    fn unit(self) -> Option<Self> {
        self.try_normalize(EPSILON)
    }
    fn norm_of(&self) -> Real {
        self.norm()
    }
    fn scaled(self, factor: Real) -> Self {
        self * factor
    }
}

impl NormalisableTangent for Vec3 {
    fn unit(self) -> Option<Self> {
        self.try_normalize(EPSILON)
    }
    fn norm_of(&self) -> Real {
        self.norm()
    }
    fn scaled(self, factor: Real) -> Self {
        self * factor
    }
}

fn apply_2d(
    guide: &mut Guide2D,
    frame: &mut Frame,
    length: &mut Real,
    local_adjustment: bool,
) -> Result<()> {
    let path = guide.path.clone();
    let step = step(
        path.as_ref(),
        &guide.state,
        guide.last_position,
        *length,
        local_adjustment,
    )?;
    let mut tangent = step.tangent;

    if !local_adjustment {
        // The turtle travels the straight-line distance between the two points
        // on the curve, converted into turtle space.
        *length = tangent.norm_of() * guide.state.scale;
        tangent = tangent
            .unit()
            .ok_or_else(|| Error::degenerate("a guide step of zero length"))?;
        guide.last_position = step.next_position;
        guide.state.actual_t = step.next_t;
    }

    // How far the curve turned over this step, and the same turn applied to
    // the turtle.
    let mut deviation = angle2(&guide.last_heading, &tangent);
    if guide.ccw {
        deviation = -deviation;
    }
    if guide.orientation {
        if let Some(m) = axis_rotation(&frame.up, deviation) {
            frame.heading = m * frame.heading;
            frame.left = m * frame.left;
        }
    } else if let Some(m) = axis_rotation(&frame.left, -deviation) {
        frame.heading = m * frame.heading;
        frame.up = m * frame.up;
    }
    guide.last_heading = tangent;
    Ok(())
}

fn apply_3d(
    guide: &mut Guide3D,
    frame: &mut Frame,
    length: &mut Real,
    local_adjustment: bool,
) -> Result<()> {
    let path = guide.path.clone();
    let step = step(
        path.as_ref(),
        &guide.state,
        guide.last_position,
        *length,
        local_adjustment,
    )?;
    let mut tangent = step.tangent;

    if local_adjustment {
        tangent = tangent
            .unit()
            .ok_or_else(|| Error::degenerate("a guide curve with no tangent"))?;
    } else {
        *length = tangent.norm_of() * guide.state.scale;
        tangent = tangent
            .unit()
            .ok_or_else(|| Error::degenerate("a guide step of zero length"))?;
        guide.last_position = step.next_position;
        guide.state.actual_t = step.next_t;
    }

    // The new direction, read in the guide's frame and re-expressed in the
    // turtle's: this is what lets a guide bend a turtle that has been rolled
    // or turned since the guide was set.
    let projected = Vec3::new(
        tangent.dot(&guide.last_heading),
        tangent.dot(&guide.last_left),
        tangent.dot(&guide.last_up),
    );
    let projected = frame.orientation() * projected;
    let projected = projected
        .try_normalize(EPSILON)
        .ok_or_else(|| Error::degenerate("a guide direction that collapsed"))?;

    let turtle_axis = frame.heading.cross(&projected);
    let curve_axis = guide.last_heading.cross(&tangent);
    let sinus = turtle_axis.norm();
    if sinus > EPSILON {
        let turtle_axis = turtle_axis / sinus;
        let cosinus = frame.heading.dot(&projected);
        let ang = sinus.atan2(cosinus);
        if let Some(m) = axis_rotation(&turtle_axis, ang) {
            frame.heading = m * frame.heading;
            frame.left = m * frame.left;
            frame.up = m * frame.up;
        }
        // The same rotation on the curve's own frame, about the curve's axis.
        if let Some(curve_axis) = curve_axis.try_normalize(EPSILON) {
            if let Some(m) = axis_rotation(&curve_axis, ang) {
                guide.last_left = (m * guide.last_left).normalize();
                guide.last_up = (m * guide.last_up).normalize();
            }
        }
    }

    guide.last_heading = tangent;
    Ok(())
}

/// The step a guide takes when nothing is travelled — upstream's
/// `_ajustToGuide`.
pub fn adjust_to_guide(path: &mut TurtlePath, frame: &mut Frame) -> Result<GuideOutcome> {
    let mut zero = 0.0;
    path.apply(frame, &mut zero)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenegraph::curve::{Curve3D, Polyline2D};
    use crate::scenegraph::mesh::Polyline;
    use approx::assert_relative_eq;
    use std::f32::consts::FRAC_PI_2;

    /// A quarter circle of radius 1 in the XZ plane, as a dense polyline.
    ///
    /// It starts along `+Z` and ends along `+X`, which is the orientation a
    /// guide assumes: upstream's `Turtle3DPath` starts its reference frame at
    /// the turtle's own default heading rather than at the curve's first
    /// tangent, so a curve that starts elsewhere turns the turtle onto it in
    /// the first step.
    fn arc() -> Curve3DRef {
        let points = (0..=64)
            .map(|i| {
                let t = FRAC_PI_2 * i as Real / 64.0;
                Point3::new(1.0 - t.cos(), 0.0, t.sin())
            })
            .collect();
        Curve3D::from(Polyline::new(points)).into_ref()
    }

    #[test]
    fn a_3d_guide_bends_the_turtle_along_the_curve() {
        let curve = arc();
        let length = curve.length(64).unwrap();
        let mut guide = TurtlePath::Path3D(Box::new(Guide3D::new(curve, length, 64).unwrap()));

        let mut frame = Frame::default();
        let mut travelled = 0.0;
        for _ in 0..16 {
            let mut step = length / 16.0;
            assert_eq!(
                guide.apply(&mut frame, &mut step).unwrap(),
                GuideOutcome::Active
            );
            frame.position += frame.heading * step;
            travelled += step;
        }

        // The arc is a quarter turn from +Z to +X, and the turtle started
        // along +Z, so it ends heading +X, has travelled the guide's whole
        // length, and stands where the arc ends.
        //
        // The heading lags the arc's end tangent by half a step: each move
        // turns onto the *chord* of the piece of curve it covers, which is
        // the point of the guide — the segment drawn joins the two curve
        // points it spans. Sixteen steps over a quarter turn leaves ~2.8°.
        assert_relative_eq!(frame.heading, Vec3::x(), epsilon = 5e-2);
        assert_relative_eq!(travelled, length, epsilon = 1e-3);
        assert_relative_eq!(frame.position, Point3::new(1.0, 0.0, 1.0), epsilon = 2e-2);
    }

    #[test]
    fn a_guide_is_exhausted_once_consumed() {
        let curve = arc();
        let length = curve.length(64).unwrap();
        let mut guide = TurtlePath::Path3D(Box::new(Guide3D::new(curve, length, 64).unwrap()));
        let mut frame = Frame::default();

        let mut step = length * 2.0;
        assert_eq!(
            guide.apply(&mut frame, &mut step).unwrap(),
            GuideOutcome::Active
        );
        assert_eq!(guide.actual_t(), 1.0);

        let mut more = length;
        assert_eq!(
            guide.apply(&mut frame, &mut more).unwrap(),
            GuideOutcome::Exhausted
        );
    }

    #[test]
    fn set_position_moves_along_the_guide() {
        let curve = arc();
        let length = curve.length(64).unwrap();
        let mut guide = TurtlePath::Path3D(Box::new(Guide3D::new(curve, length, 64).unwrap()));
        guide.set_position(length / 2.0).unwrap();
        assert_relative_eq!(guide.actual_t(), 0.5, epsilon = 1e-6);
    }

    #[test]
    fn a_2d_guide_turns_about_the_left_axis() {
        // A straight 2D path turns nothing at all.
        let straight = Curve2DRef::new(crate::scenegraph::curve::Curve2D::from(Polyline2D::new(
            vec![Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)],
        )));
        let mut guide =
            TurtlePath::Path2D(Box::new(Guide2D::new(straight, 1.0, false, false, 8).unwrap()));
        let mut frame = Frame::default();
        let mut step = 0.5;
        guide.apply(&mut frame, &mut step).unwrap();
        assert_relative_eq!(frame.heading, Vec3::z(), epsilon = 1e-6);
        assert_relative_eq!(step, 0.5, epsilon = 1e-5);
    }

    #[test]
    fn a_zero_length_guide_is_rejected() {
        let degenerate = Curve3D::from(Polyline::new(vec![Point3::origin(), Point3::origin()]));
        assert!(Guide3D::new(degenerate.into_ref(), 1.0, 8).is_err());
    }
}
