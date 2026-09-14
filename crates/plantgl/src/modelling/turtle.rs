//! The 3D turtle.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/modelling/turtle.{h,cpp}` and
//! `pglturtle.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # What this is
//!
//! The cpfg/L-studio command set, in the grouping upstream uses: move and
//! draw; turn, pitch and roll; push and pop; width, colour and scale; the
//! standalone primitives; generalized cylinders; polygons; cross-sections;
//! guides; tropism. An L-system driver walks its derived string and calls
//! these; what comes out depends only on the [`TurtleDrawer`] it was given.
//!
//! # Naming
//!
//! Upstream overloads on argument count and distinguishes `f` from `F` by
//! case, neither of which Rust allows. The rule here: the cpfg letter becomes
//! a word — `f` stays `f` (move without drawing), `F` becomes
//! [`Turtle::forward`], `nF` becomes [`Turtle::n_forward`] — and an omitted
//! argument becomes a `_default` suffix or an [`Option`]. Every method names
//! its upstream counterpart in its documentation, so a cpfg program can be
//! translated by reading down the list.
//!
//! # Where this departs from upstream
//!
//! - **`pop` on an empty stack is an error**, not a warning. Upstream's
//!   `Turtle::pop` prints and carries on with a state that is not the one the
//!   matching `push` saved; a driver that wants that behaviour can ignore the
//!   `Err`, and one that does not gets told.
//! - **A drawing command that cannot build its geometry is an error**, for
//!   the same reason: a plant with a silently missing organ is worse than a
//!   plant that failed to build.
//! - **The frame is re-orthonormalised every
//!   [`Turtle::ORTHONORMALISE_EVERY`] rotations.** Upstream never does, and a
//!   deep derivation — tens of thousands of rotations down one axis — walks
//!   its basis out of orthonormality in `f32`. Gram-Schmidt every eighth
//!   rotation costs nothing measurable and bounds the drift.
//! - **Reflections multiply the *angle*, not the rotation matrix.** See
//!   [`Turtle::down`].
//! - `frame`/`arrow`/`vector` (debug visualisation), `label`,
//!   `setScreenCoordinatesEnabled`, `registerPushPopHandler` and upstream's
//!   static error handlers are not ported.

use std::sync::Arc;

use crate::algo::discretize::DiscretizeCtx;
use crate::error::{Error, Result};
use crate::math::{to_radians, Frame, Mat3, Point3, Real, Vec2, Vec3, EPSILON};
use crate::modelling::drawer::{DrawCtx, IdPair, SurfaceLibrary, TurtleDrawer};
use crate::modelling::param::{TurtleDefaults, TurtleState};
use crate::modelling::path::{Guide2D, Guide3D, GuideOutcome, TurtlePath};
use crate::modelling::tropism::{axis_rotation, tend_to, Reflection};
use crate::scenegraph::appearance::{
    Appearance, AppearanceRef, Color3, Color4, Material, Texture2D, Texture2DTransformation,
};
use crate::scenegraph::curve::{Curve2D, Curve2DRef, Curve3DRef, Polyline2D};
use crate::scenegraph::function::QuantisedFunction;
use crate::scenegraph::geometry::GeometryRef;
use crate::scenegraph::scene::NOID;

/// A turtle drawing into `D`.
///
/// ```
/// use plantgl::modelling::{MeasureDrawer, Turtle};
///
/// // A stem: three segments, narrowing as it goes.
/// let mut turtle = Turtle::upright(MeasureDrawer::new());
/// turtle.set_width(0.05).unwrap();
/// turtle.n_forward_tapered(1.0, 1.0 / 3.0, 0.02, None).unwrap();
///
/// let measures = turtle.drawer().measures();
/// assert_eq!(measures.segment_count, 3);
/// // Surface area is the harvest-yield input, available without a mesh.
/// assert!(measures.surface_area > 0.0);
/// ```
#[derive(Debug, Clone)]
pub struct Turtle<D> {
    state: TurtleState,
    stack: Vec<TurtleState>,
    defaults: TurtleDefaults,
    drawer: D,
    materials: Vec<AppearanceRef>,
    surfaces: SurfaceLibrary,
    id: u32,
    parent_id: u32,
    rotations: u32,
    /// The density guides and cross-sections are sampled at when the curve
    /// carries no stride of its own.
    samples: u32,
}

impl<D: TurtleDrawer> Turtle<D> {
    /// How many rotations may pass before the frame is re-orthonormalised.
    pub const ORTHONORMALISE_EVERY: u32 = 8;

    /// A turtle in upstream's starting state: at the origin, heading `+Z`,
    /// with upstream's default colour list and its one default surface.
    pub fn new(drawer: D) -> Self {
        let mut turtle = Self {
            state: TurtleState::default(),
            stack: Vec::new(),
            defaults: TurtleDefaults::default(),
            drawer,
            materials: default_materials(),
            surfaces: SurfaceLibrary::with_default_leaf(),
            id: NOID,
            parent_id: NOID,
            rotations: 0,
            samples: DiscretizeCtx::DEFAULT_CURVE_SAMPLES,
        };
        turtle.set_default_cross_section(turtle.state.section_resolution);
        turtle
    }

    /// A turtle heading `+Y`, the direction the game's plants grow. See
    /// [`TurtleState::upright`] for why this is not upstream's default.
    pub fn upright(drawer: D) -> Self {
        let mut turtle = Self::new(drawer);
        turtle.state.frame = TurtleState::upright().frame;
        turtle
    }

    /// `start()` — resets everything, turtle and drawer alike.
    pub fn start(&mut self) {
        self.reset();
    }

    /// `stop()` — closes an open generalized cylinder, drawing what it has.
    pub fn stop(&mut self) -> Result<()> {
        if self.state.is_generalized_cylinder_on() && self.state.points().len() > 1 {
            self.draw_generalized_cylinder()?;
        }
        self.state.remove_points();
        Ok(())
    }

    /// `reset()`.
    pub fn reset(&mut self) {
        self.reset_values();
        self.drawer.reset();
    }

    /// `resetValues()` — the turtle's own state, leaving the drawer's output
    /// alone.
    pub fn reset_values(&mut self) {
        let upright = self.state.frame == TurtleState::upright().frame;
        self.state.reset();
        if upright {
            self.state.frame = TurtleState::upright().frame;
        }
        self.stack.clear();
        self.rotations = 0;
        self.set_default_cross_section(self.state.section_resolution);
    }

    // ---- accessors ---------------------------------------------------

    pub fn drawer(&self) -> &D {
        &self.drawer
    }

    pub fn drawer_mut(&mut self) -> &mut D {
        &mut self.drawer
    }

    /// Consumes the turtle for its drawer — how a scene or a mesh is
    /// collected once the derivation is interpreted.
    pub fn into_drawer(self) -> D {
        self.drawer
    }

    pub fn state(&self) -> &TurtleState {
        &self.state
    }

    pub fn defaults(&self) -> &TurtleDefaults {
        &self.defaults
    }

    pub fn defaults_mut(&mut self) -> &mut TurtleDefaults {
        &mut self.defaults
    }

    /// The colour list `setColor` indexes — upstream's `PglTurtle::__appList`.
    pub fn materials(&self) -> &[AppearanceRef] {
        &self.materials
    }

    pub fn materials_mut(&mut self) -> &mut Vec<AppearanceRef> {
        &mut self.materials
    }

    /// The templates `surface(name, scale)` instances.
    pub fn surfaces(&self) -> &SurfaceLibrary {
        &self.surfaces
    }

    pub fn surfaces_mut(&mut self) -> &mut SurfaceLibrary {
        &mut self.surfaces
    }

    pub fn position(&self) -> Point3 {
        self.state.frame.position
    }

    pub fn heading(&self) -> Vec3 {
        self.state.frame.heading
    }

    /// The `left` axis of the frame. Named for the axis, not for the command
    /// [`Turtle::left`], which turns about `up`.
    pub fn left_vector(&self) -> Vec3 {
        self.state.frame.left
    }

    /// The `up` axis of the frame.
    pub fn up_vector(&self) -> Vec3 {
        self.state.frame.up
    }

    pub fn scale_of(&self) -> Vec3 {
        self.state.scale
    }

    pub fn width(&self) -> Real {
        self.state.width
    }

    pub fn color(&self) -> i32 {
        self.state.draw.color
    }

    /// `getId()`.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// `emptyStack()`.
    pub fn stack_is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    /// `isValid()` — the frame is still orthonormal.
    pub fn is_valid(&self) -> bool {
        self.state.is_valid()
    }

    /// `isGCEnabled()`.
    pub fn is_gc_enabled(&self) -> bool {
        self.state.is_generalized_cylinder_on()
    }

    /// `isPolygonEnabled()`.
    pub fn is_polygon_enabled(&self) -> bool {
        self.state.is_polygon_on()
    }

    // ---- stack -------------------------------------------------------

    /// `push()` — saves the state, and starts a fresh sweep from the last
    /// point if a generalized cylinder is open.
    pub fn push(&mut self) {
        self.state.last_id = self.parent_id;
        self.stack.push(self.state.clone());
        if self.state.is_generalized_cylinder_on() {
            self.state.keep_last_point();
            self.state.custom_parent_id = self.state.custom_id;
            self.state.custom_id = self.id;
            self.parent_id = self.id;
        }
    }

    /// `pop()` — restores the state, drawing an open generalized cylinder
    /// first, as upstream does: a branch's sweep ends where the branch does.
    ///
    /// Returns [`Error::EmptyStack`] where upstream warns and continues.
    pub fn pop(&mut self) -> Result<()> {
        if self.state.is_generalized_cylinder_on() && self.state.points().len() > 1 {
            self.draw_generalized_cylinder()?;
        }
        match self.stack.pop() {
            Some(previous) => {
                self.state = previous;
                self.parent_id = self.state.last_id;
                Ok(())
            }
            None => Err(Error::EmptyStack),
        }
    }

    // ---- movement ----------------------------------------------------

    /// `f()` — move the default step without drawing.
    pub fn f_default(&mut self) -> Result<()> {
        self.f(self.defaults.step)
    }

    /// `f(length)` — move without drawing.
    pub fn f(&mut self, length: Real) -> Result<()> {
        let mut length = length;
        if length > 0.0 && self.state.guide.is_some() {
            self.apply_guide(&mut length)?;
        }
        if self.state.elasticity > EPSILON {
            self.apply_tropism();
        }
        self.state.forward(length);
        if length > 0.0 && self.state.guide.is_some() {
            self.adjust_to_guide()?;
        }
        Ok(())
    }

    /// `F()` — draw the default step.
    pub fn forward_default(&mut self) -> Result<()> {
        self.forward(self.defaults.step)
    }

    /// `F(length)` — draw a segment of the current width.
    pub fn forward(&mut self, length: Real) -> Result<()> {
        self.forward_tapered_opt(length, None)
    }

    /// `F(length, topradius)` — draw a segment that ends at `top_radius`, and
    /// leave the width there.
    pub fn forward_tapered(&mut self, length: Real, top_radius: Real) -> Result<()> {
        self.forward_tapered_opt(length, Some(top_radius))
    }

    /// The two `F`s in one. Upstream signals "no top radius" with a negative
    /// sentinel; here it is [`None`].
    fn forward_tapered_opt(&mut self, length: Real, top_radius: Option<Real>) -> Result<()> {
        if length < -EPSILON {
            // Upstream draws a backwards segment by turning round, drawing it
            // forwards, and turning back.
            self.turn_around();
            self.forward_tapered_opt(-length, top_radius)?;
            self.turn_around();
            return Ok(());
        }
        if length <= 0.0 {
            return Ok(());
        }

        let mut length = length;
        if self.state.guide.is_some() {
            self.apply_guide(&mut length)?;
        }
        if self.state.elasticity > EPSILON {
            self.apply_tropism();
        }

        if length > EPSILON {
            if !self.state.is_generalized_cylinder_on() {
                self.draw_segment(length, top_radius)?;
            }
            self.state.forward(length);
            self.state.draw.axial_length += length;
        }
        if self.state.guide.is_some() {
            self.adjust_to_guide()?;
        }
        if length > EPSILON
            && (self.state.is_generalized_cylinder_on() || self.state.is_polygon_on())
        {
            self.state.push_position();
        }
        if let Some(top_radius) = top_radius {
            self.set_width(top_radius)?;
        }
        Ok(())
    }

    /// `nF(length, dl)` — `length` of travel in steps of `dl`, so a guided or
    /// tropic axis is sampled finely enough to bend.
    pub fn n_forward(&mut self, length: Real, dl: Real) -> Result<()> {
        if dl <= EPSILON {
            return Err(Error::degenerate("nF needs a positive step"));
        }
        let steps = (length / dl) as u32;
        let remainder = length - steps as Real * dl;
        for _ in 0..steps {
            self.forward(dl)?;
        }
        if remainder > EPSILON {
            self.forward(remainder)?;
        }
        Ok(())
    }

    /// `nF(length, dl, radius, radiusvariation)` — the same, narrowing to
    /// `radius`, or following `radius_variation` scaled by it.
    ///
    /// This is how a tapered axis is drawn without the caller computing a
    /// width per segment: the profile is read at the fraction of the axis each
    /// step lands on.
    pub fn n_forward_tapered(
        &mut self,
        length: Real,
        dl: Real,
        radius: Real,
        radius_variation: Option<&QuantisedFunction>,
    ) -> Result<()> {
        if dl <= EPSILON {
            return Err(Error::degenerate("nF needs a positive step"));
        }
        let steps = (length / dl) as u32;
        let remainder = length - steps as Real * dl;

        match radius_variation {
            Some(profile) => {
                let (first_x, last_x) = (profile.first_x(), profile.last_x());
                let du = (dl / length) * (last_x - first_x);
                let mut u = first_x;
                self.set_width(profile.value(u) * radius)?;
                for _ in 0..steps {
                    u += du;
                    self.forward_tapered(dl, profile.value(u) * radius)?;
                }
                if remainder > EPSILON {
                    self.forward_tapered(remainder, radius * profile.value(last_x))?;
                }
            }
            None => {
                let start = self.width();
                let dw = (radius - start) / length;
                let mut width = start;
                for _ in 0..steps {
                    width += dw * dl;
                    self.forward_tapered(dl, width)?;
                }
                if remainder > EPSILON {
                    self.forward_tapered(remainder, radius)?;
                }
            }
        }
        Ok(())
    }

    /// `move(pos)` — teleport, drawing nothing.
    pub fn move_to(&mut self, position: Point3) {
        self.state.frame.position = position;
    }

    /// `shift(v)`.
    pub fn shift(&mut self, offset: Vec3) {
        self.state.frame.position += offset;
    }

    /// `lineTo(v, topradius)` — draw to `v`, leaving the orientation alone.
    pub fn line_to(&mut self, target: Point3, top_radius: Option<Real>) -> Result<()> {
        let orientation = self.state.frame;
        self.o_line_to(target, top_radius)?;
        self.restore_orientation(&orientation);
        Ok(())
    }

    /// `lineRel(v, topradius)`.
    pub fn line_rel(&mut self, offset: Vec3, top_radius: Option<Real>) -> Result<()> {
        let orientation = self.state.frame;
        self.o_line_rel(offset, top_radius)?;
        self.restore_orientation(&orientation);
        Ok(())
    }

    /// `oLineTo(v, topradius)` — draw to `v` and end up pointing that way.
    pub fn o_line_to(&mut self, target: Point3, top_radius: Option<Real>) -> Result<()> {
        let direction = target - self.state.frame.position;
        self.draw_towards(direction, top_radius)
    }

    /// `oLineRel(v, topradius)`.
    pub fn o_line_rel(&mut self, offset: Vec3, top_radius: Option<Real>) -> Result<()> {
        self.draw_towards(offset, top_radius)
    }

    /// `pinpoint(v)` — turn to face `v` without moving.
    pub fn pinpoint(&mut self, target: Point3) {
        self.pinpoint_rel(target - self.state.frame.position);
    }

    /// `pinpointRel(v)`.
    pub fn pinpoint_rel(&mut self, direction: Vec3) {
        if let Some(unit) = direction.try_normalize(EPSILON) {
            self.tend_towards(&unit, 1.0);
        }
    }

    fn draw_towards(&mut self, direction: Vec3, top_radius: Option<Real>) -> Result<()> {
        let length = direction.norm();
        if length <= EPSILON {
            return Ok(());
        }
        self.tend_towards(&(direction / length), 1.0);
        self.forward_tapered_opt(length, top_radius)
    }

    fn restore_orientation(&mut self, saved: &Frame) {
        self.state.frame.heading = saved.heading;
        self.state.frame.left = saved.left;
        self.state.frame.up = saved.up;
    }

    // ---- rotation ----------------------------------------------------

    /// `left()`.
    pub fn left_default(&mut self) {
        self.left(self.defaults.angle_increment);
    }

    /// `left(angle)` — turn about `up`, in degrees.
    pub fn left(&mut self, angle: Real) {
        let radians = to_radians(angle) * Reflection::Left.component(&self.state.reflection);
        let up = self.state.frame.up;
        if let Some(m) = axis_rotation(&up, radians) {
            self.state.frame.heading = m * self.state.frame.heading;
            self.state.frame.left = m * self.state.frame.left;
            self.note_rotation();
        }
    }

    /// `right(angle)`.
    pub fn right(&mut self, angle: Real) {
        self.left(-angle);
    }

    /// `right()`.
    pub fn right_default(&mut self) {
        self.left(-self.defaults.angle_increment);
    }

    /// `down(angle)` — pitch about `left`, in degrees.
    ///
    /// # The reflection is applied to the angle
    ///
    /// Upstream writes `Matrix3::axisRotation(left, ra) * reflection.z()`,
    /// which multiplies the rotation *matrix* by ±1 rather than the angle. A
    /// negated rotation matrix has determinant −1: it is not a rotation, and
    /// applying it to `heading` and `up` but not `left` leaves a frame that is
    /// no longer right-handed — every later `heading × left = up` invariant is
    /// then false. `left()`, two methods up, multiplies the *angle* by
    /// `reflection.y()`, which is what a mirrored turn means. The port does
    /// the same here and in [`Turtle::roll_left`], so the three families agree
    /// and the frame stays orthonormal.
    pub fn down(&mut self, angle: Real) {
        let radians = to_radians(angle) * Reflection::Up.component(&self.state.reflection);
        let left = self.state.frame.left;
        if let Some(m) = axis_rotation(&left, radians) {
            self.state.frame.heading = m * self.state.frame.heading;
            self.state.frame.up = m * self.state.frame.up;
            self.note_rotation();
        }
    }

    /// `down()`.
    pub fn down_default(&mut self) {
        self.down(self.defaults.angle_increment);
    }

    /// `up(angle)` — pitch up, in degrees.
    pub fn up(&mut self, angle: Real) {
        self.down(-angle);
    }

    /// `up()`.
    pub fn up_default(&mut self) {
        self.down(-self.defaults.angle_increment);
    }

    /// `rollL(angle)` — roll about `heading`, in degrees. See
    /// [`Turtle::down`] on the reflection.
    pub fn roll_left(&mut self, angle: Real) {
        let radians = to_radians(angle) * Reflection::Heading.component(&self.state.reflection);
        let heading = self.state.frame.heading;
        if let Some(m) = axis_rotation(&heading, radians) {
            self.state.frame.up = m * self.state.frame.up;
            self.state.frame.left = m * self.state.frame.left;
            self.note_rotation();
        }
    }

    /// `rollL()`.
    pub fn roll_left_default(&mut self) {
        self.roll_left(self.defaults.angle_increment);
    }

    /// `rollR(angle)`.
    pub fn roll_right(&mut self, angle: Real) {
        self.roll_left(-angle);
    }

    /// `rollR()`.
    pub fn roll_right_default(&mut self) {
        self.roll_left(-self.defaults.angle_increment);
    }

    /// `iRollL(angle)` — an *intrinsic* roll: the guide's reference frame
    /// rolls with the turtle, so the roll survives the guide's next turn.
    ///
    /// Without a 3D guide this is the same as [`Turtle::roll_left`]; upstream
    /// warns in that case, the port does not, because the result is correct
    /// either way.
    pub fn i_roll_left(&mut self, angle: Real) {
        let radians = to_radians(angle) * Reflection::Heading.component(&self.state.reflection);
        let heading = self.state.frame.heading;
        if let Some(m) = axis_rotation(&heading, radians) {
            self.state.frame.up = m * self.state.frame.up;
            self.state.frame.left = m * self.state.frame.left;
            self.note_rotation();
        }
        if let Some(TurtlePath::Path3D(guide)) = self.state.guide.as_mut() {
            guide.roll(radians);
        }
    }

    /// `iRollR(angle)`.
    pub fn i_roll_right(&mut self, angle: Real) {
        self.i_roll_left(-angle);
    }

    /// `turnAround()` — `left(180)`.
    pub fn turn_around(&mut self) {
        self.left(180.0);
    }

    /// `rollToVert(alpha, top)` — roll so `up` points as near `top` as the
    /// heading allows. `alpha` of 1 does it fully; less interpolates.
    ///
    /// This is how a leaf is kept face-up however the stem it hangs off has
    /// twisted.
    pub fn roll_to_vert(&mut self, alpha: Real, top: Vec3) {
        if alpha <= 0.0 {
            return;
        }
        if alpha >= 1.0 {
            let left = top.cross(&self.state.frame.heading);
            self.state.frame.left = match left.try_normalize(EPSILON) {
                Some(left) => left,
                // Heading parallel to `top`: any `left` will do, and upstream
                // picks its default one.
                None => Vec3::new(0.0, -1.0, 0.0),
            };
            self.state.frame.up = self.state.frame.heading.cross(&self.state.frame.left);
            self.note_rotation();
            return;
        }
        let Some(target_left) = top.cross(&self.state.frame.heading).try_normalize(EPSILON) else {
            return;
        };
        let rotation = crate::math::angle3_about(
            &self.state.frame.left,
            &target_left,
            &self.state.frame.heading,
        );
        self.roll_right(crate::math::to_degrees(rotation) * alpha);
    }

    /// `rollToHorizontal(alpha, top)` — turn the heading towards the plane
    /// normal to `top`.
    pub fn roll_to_horizontal(&mut self, alpha: Real, top: Vec3) {
        if alpha <= 0.0 {
            return;
        }
        let alpha = alpha.min(1.0);
        let out_of_plane = self.state.frame.heading.dot(&top);
        if out_of_plane.abs() <= EPSILON {
            return;
        }
        let target = self.state.frame.heading - top * out_of_plane;
        let target = target
            .try_normalize(EPSILON)
            .unwrap_or_else(|| Vec3::new(1.0, 0.0, 0.0));
        let Some(axis) = self.state.frame.heading.cross(&target).try_normalize(EPSILON) else {
            return;
        };
        let angle = crate::math::angle3_about(&self.state.frame.heading, &target, &axis);
        if let Some(m) = axis_rotation(&axis, angle * alpha) {
            self.transform(&m);
        }
    }

    /// `setHead(h, u)` — point the turtle along `heading` with `up` as near
    /// the given `up` as orthogonality allows.
    pub fn set_head(&mut self, heading: Vec3, up: Vec3) -> Result<()> {
        let heading = heading
            .try_normalize(EPSILON)
            .ok_or_else(|| Error::degenerate("setHead was given a null heading"))?;
        let up = up
            .try_normalize(EPSILON)
            .ok_or_else(|| Error::degenerate("setHead was given a null up vector"))?;
        let left = up
            .cross(&heading)
            .try_normalize(EPSILON)
            .ok_or_else(|| Error::degenerate("setHead was given colinear heading and up"))?;
        // Upstream re-orthogonalises a skewed `up` rather than refusing it.
        let up = heading.cross(&left);
        self.state.frame.heading = heading;
        self.state.frame.left = left;
        self.state.frame.up = up;
        self.rotations = 0;
        Ok(())
    }

    /// `eulerAngles(azimuth, elevation, roll)` — orient absolutely, in
    /// degrees.
    pub fn euler_angles(&mut self, azimuth: Real, elevation: Real, roll: Real) {
        let m = crate::math::euler_rotation_zyx(
            to_radians(azimuth),
            -to_radians(elevation),
            to_radians(roll),
        );
        let heading = m * Vec3::new(1.0, 0.0, 0.0);
        let up = m * Vec3::new(0.0, 0.0, 1.0);
        self.state.frame.heading = heading;
        self.state.frame.up = up;
        self.state.frame.left = up.cross(&heading);
        self.rotations = 0;
    }

    /// `transform(matrix)` — rotate the whole frame.
    pub fn transform(&mut self, m: &Mat3) {
        self.state.transform(m);
        self.note_rotation();
    }

    /// `leftReflection()` — mirror the `left`/`right` family.
    pub fn left_reflection(&mut self) {
        Reflection::Left.flip(&mut self.state.reflection);
    }

    /// `upReflection()` — mirror the `up`/`down` family.
    pub fn up_reflection(&mut self) {
        Reflection::Up.flip(&mut self.state.reflection);
    }

    /// `headingReflection()` — mirror the roll family.
    pub fn heading_reflection(&mut self) {
        Reflection::Heading.flip(&mut self.state.reflection);
    }

    // ---- scale, width, colour ----------------------------------------

    /// `scale(s)`.
    pub fn scale(&mut self, scale: Vec3) {
        self.state.scale = scale;
    }

    /// `scale(s)` with one factor.
    pub fn scale_uniform(&mut self, scale: Real) {
        self.scale(Vec3::new(scale, scale, scale));
    }

    /// `multScale(s)`.
    pub fn mult_scale(&mut self, factor: Vec3) {
        let current = self.state.scale;
        self.scale(Vec3::new(
            current.x * factor.x,
            current.y * factor.y,
            current.z * factor.z,
        ));
    }

    /// `multScale()` — by the default multiplier.
    pub fn mult_scale_default(&mut self) {
        let m = self.defaults.scale_multiplier;
        self.mult_scale(Vec3::new(m, m, m));
    }

    /// `divScale(s)`.
    pub fn div_scale(&mut self, factor: Vec3) {
        let current = self.state.scale;
        self.scale(Vec3::new(
            current.x / factor.x,
            current.y / factor.y,
            current.z / factor.z,
        ));
    }

    /// `divScale()` — by the default multiplier.
    pub fn div_scale_default(&mut self) {
        let m = self.defaults.scale_multiplier;
        self.div_scale(Vec3::new(m, m, m));
    }

    /// `setWidth(v)`. A negative width is an error where upstream warns.
    pub fn set_width(&mut self, width: Real) -> Result<()> {
        if width <= -EPSILON {
            return Err(Error::degenerate(format!("a width of {width} is negative")));
        }
        self.state.width = width;
        if self.state.is_generalized_cylinder_on() {
            self.state.push_radius();
        }
        Ok(())
    }

    /// `incWidth()`.
    pub fn inc_width(&mut self) -> Result<()> {
        self.set_width(self.state.width + self.defaults.width_increment)
    }

    /// `decWidth()`.
    pub fn dec_width(&mut self) -> Result<()> {
        self.set_width(self.state.width - self.defaults.width_increment)
    }

    /// `setColor(v)` — select an entry of the colour list.
    pub fn set_color(&mut self, color: i32) -> Result<()> {
        if color < 0 || color as usize >= self.materials.len() {
            return Err(Error::invalid_index(format!(
                "colour {color} is outside the turtle's list of {}",
                self.materials.len()
            )));
        }
        self.state.draw.color = color;
        self.state.draw.custom_material = None;
        self.state.draw.axial_length = 0.0;
        if self.state.is_gc_or_polygon_on_init() {
            self.state.initial.color = color;
        }
        Ok(())
    }

    /// `incColor()`.
    pub fn inc_color(&mut self) -> Result<()> {
        self.set_color(self.state.draw.color + self.defaults.color_increment)
    }

    /// `decColor()`.
    pub fn dec_color(&mut self) -> Result<()> {
        self.set_color(self.state.draw.color - self.defaults.color_increment)
    }

    /// `interpolateColors(a, b, alpha)` — draw in a colour part way between
    /// two entries of the list.
    pub fn interpolate_colors(&mut self, first: i32, second: i32, alpha: Real) -> Result<()> {
        let material = |turtle: &Self, index: i32| -> Result<Material> {
            let appearance = usize::try_from(index)
                .ok()
                .and_then(|i| turtle.materials.get(i))
                .ok_or_else(|| {
                    Error::invalid_index(format!("colour {index} is outside the turtle's list"))
                })?;
            match appearance.as_ref() {
                Appearance::Material(m) => Ok(m.clone()),
                Appearance::Texture2D(_) => Err(Error::unsupported(
                    "only materials can be interpolated, not textures",
                )),
            }
        };
        let blended = Material::interpolate(&material(self, first)?, &material(self, second)?, alpha);
        self.set_custom_appearance(Some(Arc::new(Appearance::Material(blended))));
        Ok(())
    }

    /// `setCustomAppearance(app)` — draw with this appearance, whatever the
    /// colour index says. `None` is `removeCustomAppearance()`.
    pub fn set_custom_appearance(&mut self, appearance: Option<AppearanceRef>) {
        self.state.draw.custom_material = appearance.clone();
        self.state.draw.axial_length = 0.0;
        if self.state.is_gc_or_polygon_on_init() {
            match appearance {
                Some(appearance) => self.state.initial.custom_material = Some(appearance),
                None => self.state.initial.color = self.state.draw.color,
            }
        }
    }

    // ---- textures ----------------------------------------------------

    /// `setTextureScale(s)`.
    pub fn set_texture_scale(&mut self, scale: Vec2) {
        self.state.draw.texture.scale = scale;
        if self.state.is_gc_or_polygon_on_init() {
            self.state.initial.texture.scale = scale;
        }
    }

    /// `setTextureUScale(u)`.
    pub fn set_texture_u_scale(&mut self, u: Real) {
        let mut scale = self.state.draw.texture.scale;
        scale.x = u;
        self.set_texture_scale(scale);
    }

    /// `setTextureVScale(v)`.
    pub fn set_texture_v_scale(&mut self, v: Real) {
        let mut scale = self.state.draw.texture.scale;
        scale.y = v;
        self.set_texture_scale(scale);
    }

    /// `setTextureTranslation(t)`.
    pub fn set_texture_translation(&mut self, translation: Vec2) {
        self.state.draw.texture.translation = translation;
        if self.state.is_gc_or_polygon_on_init() {
            self.state.initial.texture.translation = translation;
        }
    }

    /// `setTextureRotation(angle, center)` — the angle in degrees, as
    /// upstream stores it.
    pub fn set_texture_rotation(&mut self, angle: Real, center: Vec2) {
        self.state.draw.texture.rotation_angle = angle;
        self.state.draw.texture.rotation_center = center;
        if self.state.is_gc_or_polygon_on_init() {
            self.state.initial.texture.rotation_angle = angle;
            self.state.initial.texture.rotation_center = center;
        }
    }

    /// `setTextureTransformation(scaling, translation, angle, rotcenter)`.
    pub fn set_texture_transformation(
        &mut self,
        scaling: Vec2,
        translation: Vec2,
        angle: Real,
        rotation_center: Vec2,
    ) {
        self.set_texture_scale(scaling);
        self.set_texture_translation(translation);
        self.set_texture_rotation(angle, rotation_center);
    }

    /// `setTextureBaseColor(v)`.
    pub fn set_texture_base_color(&mut self, color: Color4) {
        self.state.draw.texture.base_color = color;
    }

    /// `setTextureBaseColor(index)` — the diffuse colour of a list entry.
    pub fn set_texture_base_color_from(&mut self, index: i32) -> Result<()> {
        let appearance = usize::try_from(index)
            .ok()
            .and_then(|i| self.materials.get(i))
            .ok_or_else(|| {
                Error::invalid_index(format!("colour {index} is outside the turtle's list"))
            })?;
        match appearance.as_ref() {
            Appearance::Material(m) => {
                let color = m.diffuse_color();
                self.set_texture_base_color(Color4::from_color3(color, 0));
                Ok(())
            }
            Appearance::Texture2D(_) => Err(Error::unsupported(
                "a texture has no diffuse colour to take a base colour from",
            )),
        }
    }

    /// `interpolateTextureBaseColors(a, b, alpha)`.
    pub fn interpolate_texture_base_colors(
        &mut self,
        first: i32,
        second: i32,
        alpha: Real,
    ) -> Result<()> {
        let material = |turtle: &Self, index: i32| -> Result<Material> {
            let appearance = usize::try_from(index)
                .ok()
                .and_then(|i| turtle.materials.get(i))
                .ok_or_else(|| {
                    Error::invalid_index(format!("colour {index} is outside the turtle's list"))
                })?;
            match appearance.as_ref() {
                Appearance::Material(m) => Ok(m.clone()),
                Appearance::Texture2D(_) => Err(Error::unsupported(
                    "only materials can be interpolated, not textures",
                )),
            }
        };
        let (a, b) = (material(self, first)?, material(self, second)?);
        let color = Color3::interpolate(a.diffuse_color(), b.diffuse_color(), alpha);
        let transparency = a.transparency * (1.0 - alpha) + b.transparency * alpha;
        self.set_texture_base_color(Color4::from_color3(
            color,
            (transparency * 255.0).clamp(0.0, 255.0) as u8,
        ));
        Ok(())
    }

    // ---- ids ---------------------------------------------------------

    /// `setId(i)`.
    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    /// `setNoId()`.
    pub fn set_no_id(&mut self) {
        self.id = NOID;
    }

    /// `incId(i)`.
    pub fn inc_id(&mut self, by: u32) -> Result<()> {
        if self.id > u32::MAX - by {
            return Err(Error::invalid_index("the turtle's id would overflow"));
        }
        self.id += by;
        Ok(())
    }

    /// `decId(i)`.
    pub fn dec_id(&mut self, by: u32) -> Result<()> {
        if self.id < by {
            return Err(Error::invalid_index("the turtle's id would go negative"));
        }
        self.id -= by;
        Ok(())
    }

    // ---- primitives --------------------------------------------------

    /// `sphere()` — of the current width.
    pub fn sphere_default(&mut self) -> Result<()> {
        self.sphere(self.state.width)
    }

    /// `sphere(radius)`.
    pub fn sphere(&mut self, radius: Real) -> Result<()> {
        if radius < -EPSILON {
            return Err(Error::degenerate("a sphere of negative radius"));
        }
        if radius <= EPSILON {
            return Ok(());
        }
        let (frame, ctx_ids, appearance) = self.draw_inputs();
        let resolution = self.state.section_resolution;
        let ctx = self.ctx(&frame, ctx_ids, appearance);
        self.drawer.sphere(&ctx, radius, resolution)
    }

    /// `circle()` — of the current width.
    pub fn circle_default(&mut self) -> Result<()> {
        self.circle(self.state.width)
    }

    /// `circle(radius)` — a disc across the heading.
    pub fn circle(&mut self, radius: Real) -> Result<()> {
        if radius < -EPSILON {
            return Err(Error::degenerate("a circle of negative radius"));
        }
        if radius <= EPSILON {
            return Ok(());
        }
        let (frame, ctx_ids, appearance) = self.draw_inputs();
        let resolution = self.state.section_resolution;
        let ctx = self.ctx(&frame, ctx_ids, appearance);
        self.drawer.circle(&ctx, radius, resolution)
    }

    /// `box(length, topradius)` — a rectangular segment; the turtle advances
    /// along it as it would for an `F`.
    pub fn box3(&mut self, length: Real, top_radius: Option<Real>) -> Result<()> {
        self.draw_prism(length, top_radius, true)
    }

    /// `box()`.
    pub fn box3_default(&mut self) -> Result<()> {
        self.box3(self.defaults.step, None)
    }

    /// `quad(length, topradius)` — a flat ribbon; the turtle advances along it.
    pub fn quad(&mut self, length: Real, top_radius: Option<Real>) -> Result<()> {
        self.draw_prism(length, top_radius, false)
    }

    /// `quad()`.
    pub fn quad_default(&mut self) -> Result<()> {
        self.quad(self.defaults.step, None)
    }

    /// `surface(name, scale)` — place a template from the surface library.
    ///
    /// This is how leaves, petals and fruit that are modelled rather than
    /// swept reach the scene: one geometry, registered once, instanced
    /// wherever the L-system says, oriented by the turtle's frame.
    pub fn surface(&mut self, name: &str, scale: Real) -> Result<()> {
        if scale < -EPSILON {
            return Err(Error::degenerate("a surface of negative scale"));
        }
        let geometry: GeometryRef = self
            .surfaces
            .get(name)
            .cloned()
            .ok_or_else(|| Error::UnknownSurface(name.to_string()))?;
        let (frame, ctx_ids, appearance) = self.draw_inputs();
        let ctx = self.ctx(&frame, ctx_ids, appearance);
        self.drawer.custom_geometry(&ctx, geometry.as_ref(), scale)
    }

    // ---- polygons ----------------------------------------------------

    /// `startPolygon()`.
    pub fn start_polygon(&mut self) {
        self.state.set_polygon(true);
        self.state.custom_id = self.pop_id();
        self.state.custom_parent_id = self.parent_id;
    }

    /// `polygonPoint()` — record the turtle's position as a vertex.
    pub fn polygon_point(&mut self) {
        self.state.push_position();
    }

    /// `stopPolygon(concave_test)` — close the outline and draw it.
    ///
    /// With `concave_test` the outline is ear-clipped rather than fanned,
    /// which is what a notched or lobed leaf needs.
    pub fn stop_polygon(&mut self, concave_test: bool) -> Result<()> {
        if self.state.points().len() > 2 {
            let points = self.state.points().to_vec();
            let (frame, ctx_ids, appearance) = self.draw_inputs_initial();
            let ctx = self.ctx(&frame, ctx_ids, appearance);
            self.drawer.polygon(&ctx, &points, concave_test)?;
        }
        self.state.set_polygon(false);
        self.state.custom_id = NOID;
        self.state.custom_parent_id = NOID;
        Ok(())
    }

    // ---- generalized cylinders ---------------------------------------

    /// `startGC()` — start accumulating an axis to sweep in one piece.
    ///
    /// Between `startGC` and `stopGC` an `F` draws nothing on its own: it
    /// records its end point, its `left` and its width, and the whole run is
    /// swept as one [`Extrusion`](crate::scenegraph::primitive::Extrusion) at
    /// the end. That is what makes a branch one continuous, mitred, tapering
    /// surface rather than a stack of cans.
    pub fn start_gc(&mut self) {
        self.state.set_generalized_cylinder(true);
        self.state.custom_id = self.pop_id();
        self.state.custom_parent_id = self.parent_id;
        self.state.push_position();
    }

    /// `stopGC()`.
    pub fn stop_gc(&mut self) -> Result<()> {
        if self.state.points().len() > 1 {
            self.draw_generalized_cylinder()?;
        }
        self.state.set_generalized_cylinder(false);
        self.state.custom_id = NOID;
        self.state.custom_parent_id = NOID;
        Ok(())
    }

    // ---- cross-sections ----------------------------------------------

    /// `setCrossSection(curve, ccw)` — sweep this profile instead of a circle.
    ///
    /// A square stem for a mint, a ridged one for a grass: the profile is a
    /// per-species parameter rather than a mesh.
    pub fn set_cross_section(&mut self, curve: Curve2DRef, ccw: bool) {
        self.state.set_cross_section(curve, ccw, false);
    }

    /// `setDefaultCrossSection(slices)` — back to a circle of `slices` sides.
    pub fn set_default_cross_section(&mut self, slices: u32) {
        let circle = Curve2D::from(Polyline2D::circle(
            1.0,
            crate::modelling::geometry::slices(slices),
        ))
        .into_ref();
        self.state.set_cross_section(circle, true, true);
    }

    /// `setSectionResolution(resolution)` — how many facets a drawn tube has.
    /// The single biggest lever on a plant's triangle count.
    pub fn set_section_resolution(&mut self, resolution: u32) {
        self.state.section_resolution = resolution;
        if self.state.draw.default_section {
            self.set_default_cross_section(resolution);
        }
    }

    /// `getSectionResolution()`.
    pub fn section_resolution(&self) -> u32 {
        self.state.section_resolution
    }

    // ---- guides and tropism ------------------------------------------

    /// `setGuide(path, length)` for a 3D curve — the next `length` of travel
    /// follows it.
    pub fn set_guide(&mut self, path: Curve3DRef, length: Real) -> Result<()> {
        let guide = Guide3D::new(path, length, self.samples)?;
        self.state.guide = Some(TurtlePath::Path3D(Box::new(guide)));
        Ok(())
    }

    /// `setGuide(path, length, yorientation, ccw)` for a 2D curve.
    pub fn set_guide_2d(
        &mut self,
        path: Curve2DRef,
        length: Real,
        y_orientation: bool,
        ccw: bool,
    ) -> Result<()> {
        let guide = Guide2D::new(path, length, y_orientation, ccw, self.samples)?;
        self.state.guide = Some(TurtlePath::Path2D(Box::new(guide)));
        Ok(())
    }

    /// `clearGuide()`.
    pub fn clear_guide(&mut self) {
        self.state.guide = None;
    }

    /// `setPositionOnGuide(t)`.
    pub fn set_position_on_guide(&mut self, t: Real) -> Result<()> {
        match self.state.guide.as_mut() {
            Some(guide) => guide.set_position(t),
            None => Err(Error::unsupported("no guide is set to position along")),
        }
    }

    /// `sweep(path, section, length, dl, radius, radiusvariation)` — the whole
    /// gesture in one call: follow `path`, sweeping `section`, in steps of
    /// `dl`, narrowing as `radius_variation` says.
    pub fn sweep(
        &mut self,
        path: Curve3DRef,
        section: Curve2DRef,
        length: Real,
        dl: Real,
        radius: Real,
        radius_variation: Option<&QuantisedFunction>,
    ) -> Result<()> {
        self.set_guide(path, length)?;
        self.set_cross_section(section, false);
        self.n_forward_tapered(length, dl, radius, radius_variation)
    }

    /// `setTropism(v)` — the direction forward moves bend towards.
    pub fn set_tropism(&mut self, tropism: Vec3) {
        self.state.tropism = tropism.try_normalize(EPSILON).unwrap_or(tropism);
    }

    /// `getTropism()`.
    pub fn tropism(&self) -> Vec3 {
        self.state.tropism
    }

    /// `setElasticity(v)` — how strongly each forward move bends towards the
    /// tropism. Zero switches tropism off.
    pub fn set_elasticity(&mut self, elasticity: Real) {
        self.state.elasticity = elasticity;
    }

    /// `getElasticity()`.
    pub fn elasticity(&self) -> Real {
        self.state.elasticity
    }

    // ---- internals ---------------------------------------------------

    /// `Turtle::_applyTropism`.
    fn apply_tropism(&mut self) {
        let (tropism, elasticity) = (self.state.tropism, self.state.elasticity);
        self.tend_towards(&tropism, elasticity);
    }

    /// `Turtle::_tendTo`, applied to this turtle's frame.
    fn tend_towards(&mut self, target: &Vec3, strength: Real) {
        let frame = self.state.frame;
        if let Some(m) = tend_to(&frame.heading, &frame.up, target, strength) {
            self.transform(&m);
        }
    }

    fn apply_guide(&mut self, length: &mut Real) -> Result<()> {
        let Some(mut guide) = self.state.guide.take() else {
            return Ok(());
        };
        let outcome = guide.apply(&mut self.state.frame, length)?;
        if outcome == GuideOutcome::Active {
            self.state.guide = Some(guide);
        }
        self.note_rotation();
        Ok(())
    }

    /// `Turtle::_ajustToGuide` — a guide step of zero length, which turns the
    /// turtle onto the curve's local tangent without moving it.
    fn adjust_to_guide(&mut self) -> Result<()> {
        let mut zero = 0.0;
        self.apply_guide(&mut zero)
    }

    /// Counts a rotation and re-orthonormalises the frame every
    /// [`Turtle::ORTHONORMALISE_EVERY`] of them.
    fn note_rotation(&mut self) {
        self.rotations += 1;
        if self.rotations >= Self::ORTHONORMALISE_EVERY {
            self.rotations = 0;
            self.state.frame.orthonormalize();
        }
    }

    /// `Turtle::popId`.
    fn pop_id(&mut self) -> u32 {
        let id = self.id;
        self.parent_id = self.id;
        id
    }

    /// `Turtle::getIdPair`.
    fn id_pair(&mut self) -> IdPair {
        if self.state.custom_id != NOID {
            IdPair::new(self.state.custom_id, self.state.custom_parent_id)
        } else {
            let parent_id = self.parent_id;
            IdPair::new(self.pop_id(), parent_id)
        }
    }

    /// The frame, ids and appearance a drawing command is issued with. The
    /// frame is returned by value so the borrow of `self` ends before the
    /// drawer is called.
    fn draw_inputs(&mut self) -> (Frame, IdPair, Option<AppearanceRef>) {
        let appearance = self.current_material();
        (self.state.frame, self.id_pair(), appearance)
    }

    /// The same, for a shape that was opened earlier and is only now being
    /// drawn — a generalized cylinder or a polygon, which carry the
    /// appearance they were opened with.
    fn draw_inputs_initial(&mut self) -> (Frame, IdPair, Option<AppearanceRef>) {
        let appearance = self.current_initial_material();
        (self.state.frame, self.id_pair(), appearance)
    }

    fn ctx<'a>(
        &self,
        frame: &'a Frame,
        ids: IdPair,
        appearance: Option<AppearanceRef>,
    ) -> DrawCtx<'a> {
        DrawCtx {
            ids,
            appearance,
            frame,
            scaling: self.state.scale,
        }
    }

    /// `PglTurtle::getCurrentMaterial`.
    pub fn current_material(&self) -> Option<AppearanceRef> {
        let draw = &self.state.draw;
        self.resolve_material(
            draw.custom_material.as_ref(),
            draw.color,
            &draw.texture,
            draw.axial_length,
        )
    }

    /// `PglTurtle::getCurrentInitialMaterial`.
    pub fn current_initial_material(&self) -> Option<AppearanceRef> {
        let initial = &self.state.initial;
        self.resolve_material(
            initial.custom_material.as_ref(),
            initial.color,
            &initial.texture,
            0.0,
        )
    }

    fn resolve_material(
        &self,
        custom: Option<&AppearanceRef>,
        color: i32,
        texture: &crate::modelling::param::TextureState,
        axial_length: Real,
    ) -> Option<AppearanceRef> {
        if let Some(custom) = custom {
            return Some(custom.clone());
        }
        let slot = usize::try_from(color).ok().and_then(|i| self.materials.get(i));
        let Some(appearance) = slot else {
            // Upstream invents a `Color_n` material for an index past the end
            // rather than drawing without an appearance at all.
            return Some(Arc::new(Appearance::Material(Material::new(format!(
                "Color_{color}"
            )))));
        };
        let Appearance::Texture2D(image) = appearance.as_ref() else {
            return Some(appearance.clone());
        };
        // A texture follows the axis: the shift keeps bark running
        // continuously up a stem drawn as many segments.
        let shift = Vec2::new(0.0, axial_length * texture.scale.y);
        if shift.norm() <= EPSILON && texture.is_identity() {
            return Some(appearance.clone());
        }
        Some(Arc::new(Appearance::Texture2D(Texture2D {
            name: image.name.clone(),
            image: image.image.clone(),
            transformation: Some(Texture2DTransformation {
                scale: texture.scale,
                translation: texture.translation + shift,
                rotation_center: texture.rotation_center,
                rotation_angle: to_radians(texture.rotation_angle),
            }),
            base_color: texture.base_color,
        })))
    }

    /// One `F`'s worth of geometry: a cylinder, a frustum, or a sweep of the
    /// current cross-section.
    fn draw_segment(&mut self, length: Real, top_radius: Option<Real>) -> Result<()> {
        let width = self.state.width;
        let resolution = self.state.section_resolution;
        let default_section = self.state.draw.default_section;
        let cross_section = self.state.draw.cross_section.clone();
        let ccw = self.state.draw.cross_section_ccw;
        let (frame, ids, appearance) = self.draw_inputs();
        let ctx = self.ctx(&frame, ids, appearance);

        if default_section && width > -EPSILON {
            return match top_radius {
                None => self.drawer.cylinder(&ctx, length, width, resolution),
                Some(top) => self
                    .drawer
                    .frustum(&ctx, length, width, top, resolution),
            };
        }
        let cross_section = cross_section
            .ok_or_else(|| Error::degenerate("a sweep with no cross-section set"))?;
        self.drawer.small_sweep(
            &ctx,
            length,
            width,
            top_radius.unwrap_or(width),
            cross_section,
            ccw,
            resolution,
        )
    }

    /// A `box` or a `quad`: both draw, both advance, both take the guide and
    /// the tropism into account first.
    fn draw_prism(&mut self, length: Real, top_radius: Option<Real>, boxed: bool) -> Result<()> {
        if length < -EPSILON {
            self.turn_around();
            self.draw_prism(-length, top_radius, boxed)?;
            self.turn_around();
            return Ok(());
        }
        if length <= 0.0 {
            return Ok(());
        }

        let mut length = length;
        if self.state.guide.is_some() {
            self.apply_guide(&mut length)?;
        }
        if self.state.elasticity > EPSILON {
            self.apply_tropism();
        }

        let width = self.state.width;
        let top = top_radius.unwrap_or(width);
        let (frame, ids, appearance) = self.draw_inputs();
        let ctx = self.ctx(&frame, ids, appearance);
        if boxed {
            self.drawer.box3(&ctx, length, width, top)?;
        } else {
            self.drawer.quad(&ctx, length, width, top)?;
        }

        self.state.forward(length);
        self.state.draw.axial_length += length;
        if self.state.guide.is_some() {
            self.adjust_to_guide()?;
        }
        if let Some(top_radius) = top_radius {
            self.set_width(top_radius)?;
        }
        Ok(())
    }

    /// Hands the accumulated axis to the drawer, with the cross-section and
    /// appearance the generalized cylinder was opened with.
    fn draw_generalized_cylinder(&mut self) -> Result<()> {
        let points = self.state.points().to_vec();
        let lefts = self.state.lefts().to_vec();
        let radii = self.state.radii().to_vec();
        let resolution = self.state.section_resolution;
        let ccw = self.state.initial.cross_section_ccw;
        let cross_section = match self.state.initial.cross_section.clone() {
            Some(section) => section,
            None => Curve2D::from(Polyline2D::circle(
                1.0,
                crate::modelling::geometry::slices(resolution),
            ))
            .into_ref(),
        };
        let (frame, ids, appearance) = self.draw_inputs_initial();
        let ctx = self.ctx(&frame, ids, appearance);
        self.drawer.generalized_cylinder(
            &ctx,
            &points,
            &lefts,
            &radii,
            cross_section,
            ccw,
            resolution,
        )
    }
}

/// The colour list upstream installs in `PglTurtle::defaultValue()`.
///
/// Seven entries, and the indices are part of the interface: an L-system that
/// says `setColor(2)` means green.
pub fn default_materials() -> Vec<AppearanceRef> {
    let named = |name: &str, ambient: Color3, diffuse: Real| {
        Arc::new(Appearance::Material(Material {
            name: Some(name.to_string()),
            ambient,
            diffuse,
            ..Material::default()
        }))
    };
    vec![
        Arc::new(Appearance::Material(Material::new("Color_0"))),
        named("Color_1", Color3::new(65, 45, 15), 3.0), // brown
        named("Color_2", Color3::new(30, 60, 10), 3.0), // green
        named("Color_3", Color3::new(60, 0, 0), 3.0),   // red
        named("Color_4", Color3::new(60, 60, 15), 3.0), // yellow
        named("Color_5", Color3::new(0, 0, 60), 3.0),   // blue
        named("Color_6", Color3::new(60, 0, 60), 3.0),  // purple
    ]
}
