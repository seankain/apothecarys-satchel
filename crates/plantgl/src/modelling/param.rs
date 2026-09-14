//! Turtle state and the defaults a bare command falls back to.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/modelling/turtleparam.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Upstream's `TurtleParam` carries the `position`/`heading`/`left`/`up`
//! quadruple itself; here that is a [`Frame`], which was translated from this
//! very file in Phase A and already knows how to re-orthonormalise itself.
//! Everything else — the scale, the reflection triple, the width, the colour
//! index, the texture transformation, the tropism, the guide, and the point
//! lists a generalized cylinder or a polygon accumulates — is translated field
//! for field.

use crate::math::{Frame, Mat3, Point3, Real, Vec2, Vec3};
use crate::modelling::path::TurtlePath;
use crate::scenegraph::appearance::{AppearanceRef, Color4};
use crate::scenegraph::curve::Curve2DRef;
use crate::scenegraph::primitive::DEFAULT_SLICES;
use crate::scenegraph::scene::NOID;

/// The values a command with no argument uses — upstream's `Turtle` members
/// `default_step`, `angle_increment`, `width_increment`, `color_increment` and
/// `scale_multiplier`, plus the section resolution a fresh state starts at.
///
/// Upstream stores the first five on the turtle and the sixth on the
/// parameters; they are grouped here because they are all "what does a bare
/// command mean", and because an L-system driver wants to set them in one
/// place. Upstream's setters take the absolute value of what they are given —
/// a negative default step is a caller error, not a direction — which
/// [`TurtleDefaults::set_step`] and its siblings reproduce.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TurtleDefaults {
    /// `default_step` — the length of a bare `f`/`F`.
    pub step: Real,
    /// `angle_increment` — the angle of a bare `left`/`up`/`rollL`, in degrees.
    pub angle_increment: Real,
    /// `width_increment` — what `incWidth`/`decWidth` move the width by.
    pub width_increment: Real,
    /// `color_increment` — what `incColor`/`decColor` move the colour index by.
    pub color_increment: i32,
    /// `scale_multiplier` — what a bare `multScale`/`divScale` scales by.
    pub scale_multiplier: Real,
    /// `TurtleParam::sectionResolution` — the facet count of a drawn tube.
    pub section_resolution: u32,
}

impl Default for TurtleDefaults {
    fn default() -> Self {
        Self {
            step: 1.0,
            angle_increment: 60.0,
            width_increment: 1.0,
            color_increment: 1,
            scale_multiplier: 0.5,
            section_resolution: DEFAULT_SLICES as u32,
        }
    }
}

impl TurtleDefaults {
    /// `setDefaultStep` — stores `|value|`.
    pub fn set_step(&mut self, value: Real) {
        self.step = value.abs();
    }

    /// `setAngleIncrement` — stores `|value|`.
    pub fn set_angle_increment(&mut self, value: Real) {
        self.angle_increment = value.abs();
    }

    /// `setWidthIncrement` — stores `|value|`.
    pub fn set_width_increment(&mut self, value: Real) {
        self.width_increment = value.abs();
    }

    /// `setColorIncrement` — stores `|value|`.
    pub fn set_color_increment(&mut self, value: i32) {
        self.color_increment = value.abs();
    }

    /// `setScaleMultiplier` — stores `|value|`.
    pub fn set_scale_multiplier(&mut self, value: Real) {
        self.scale_multiplier = value.abs();
    }
}

/// The texture transformation the turtle carries — upstream's
/// `texCoordScale`, `texCoordTranslation`, `texCoordRotCenter`,
/// `texCoordRotAngle` and `texBaseColor`, which
/// `PglTurtle::getCurrentMaterial` folds into a `Texture2DTransformation` when
/// the current appearance is a texture.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextureState {
    pub scale: Vec2,
    pub translation: Vec2,
    pub rotation_center: Vec2,
    /// In degrees, as upstream stores it; it multiplies by `GEOM_RAD` only
    /// when it builds the transformation.
    pub rotation_angle: Real,
    pub base_color: Color4,
}

impl Default for TextureState {
    fn default() -> Self {
        Self {
            scale: Vec2::new(1.0, 1.0),
            translation: Vec2::zeros(),
            rotation_center: Vec2::new(0.5, 0.5),
            rotation_angle: 0.0,
            base_color: Color4::new(255, 255, 255, 0),
        }
    }
}

impl TextureState {
    /// Whether anything here would change a texture's coordinates.
    pub fn is_identity(&self) -> bool {
        *self == Self::default()
    }
}

/// The appearance-facing half of the state — upstream's
/// `TurtleDrawParameter`.
///
/// A generalized cylinder or a polygon is drawn with one appearance, decided
/// when it opened; upstream snapshots this struct into `TurtleParam::initial`
/// at `startGC`/`startPolygon` for exactly that reason, and the snapshot is
/// what the drawer is handed when the shape finally closes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DrawParams {
    /// The index into the turtle's colour list. Upstream's default is 1.
    pub color: i32,
    /// An appearance set directly, which wins over `color`.
    pub custom_material: Option<AppearanceRef>,
    /// The profile a drawn tube is swept from.
    pub cross_section: Option<Curve2DRef>,
    /// `crossSectionCCW` — the winding of that profile.
    pub cross_section_ccw: bool,
    /// Whether `cross_section` is the circle the turtle installed for itself,
    /// rather than one a caller set. Upstream draws a `Cylinder` in that case
    /// and a sweep otherwise.
    pub default_section: bool,
    pub texture: TextureState,
    /// How far the turtle has drawn along the current axis, which upstream
    /// uses to shift a texture so it runs continuously up a stem.
    pub axial_length: Real,
}

impl Default for DrawParams {
    fn default() -> Self {
        Self {
            color: 1,
            custom_material: None,
            cross_section: None,
            cross_section_ccw: true,
            default_section: true,
            texture: TextureState::default(),
            axial_length: 0.0,
        }
    }
}

impl DrawParams {
    /// `TurtleDrawParameter::reset()`.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The complete turtle state — upstream's `TurtleParam`.
///
/// One of these is cloned onto a stack by `push` and restored by `pop`, so
/// everything a branch may change locally lives here and nothing that must
/// survive a branch does.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TurtleState {
    /// Position and the `heading`/`left`/`up` basis.
    pub frame: Frame,
    /// Per-axis scale, applied to drawn geometry and to the step length.
    pub scale: Vec3,
    /// `reflection` — `±1` per axis, flipping the sense of `left`
    /// (`reflection.y`), `down` (`reflection.z`) and `rollL`
    /// (`reflection.x`). A mirrored plant is one sign change, not a second
    /// L-system.
    pub reflection: Vec3,
    pub width: Real,
    /// `sectionResolution` — facets around a drawn tube.
    pub section_resolution: u32,
    /// The direction a forward move is bent towards, per unit elasticity.
    pub tropism: Vec3,
    /// How strongly a forward move bends towards `tropism`. Zero disables it.
    pub elasticity: Real,
    /// The curve a forward move follows, if one is set.
    pub guide: Option<TurtlePath>,
    /// The current appearance and cross-section.
    pub draw: DrawParams,
    /// The snapshot taken when a generalized cylinder or polygon opened.
    pub initial: DrawParams,
    /// `customId` — the id a multi-segment shape is emitted under.
    pub custom_id: u32,
    /// `customParentId`.
    pub custom_parent_id: u32,
    /// `lastId` — the parent id to restore on `pop`.
    pub last_id: u32,

    /// The axis a generalized cylinder or polygon is accumulating.
    points: Vec<Point3>,
    /// The `left` vector at each of those points; a GC needs it to keep the
    /// cross-section's phase.
    lefts: Vec<Vec3>,
    /// The width at each of those points.
    radii: Vec<Real>,
    polygon: bool,
    generalized_cylinder: bool,
}

impl Default for TurtleState {
    fn default() -> Self {
        Self {
            frame: Frame::default(),
            scale: Vec3::new(1.0, 1.0, 1.0),
            reflection: Vec3::new(1.0, 1.0, 1.0),
            width: 0.1,
            section_resolution: DEFAULT_SLICES as u32,
            tropism: Vec3::new(0.0, 0.0, 1.0),
            elasticity: 0.0,
            guide: None,
            draw: DrawParams::default(),
            initial: DrawParams::default(),
            custom_id: NOID,
            custom_parent_id: NOID,
            last_id: NOID,
            points: Vec::new(),
            lefts: Vec::new(),
            radii: Vec::new(),
            polygon: false,
            generalized_cylinder: false,
        }
    }
}

impl TurtleState {
    /// A state whose heading is `+Y`, as the game's plants grow.
    ///
    /// **Not upstream.** PlantGL's turtle starts heading `+Z` with `left = -Y`
    /// and `up = +X`, which is what [`TurtleState::default`] reproduces and
    /// what the differential harness compares against. The engine's world is
    /// Y-up, and `crates/botany`'s own turtle — the one #21 replaces — starts
    /// heading `+Y`, `left = -X`, `up = +Z`. That frame is right-handed and
    /// orthonormal, so it is a legitimate starting state rather than a
    /// different turtle: this constructor is the one the game path uses, and
    /// the acceptance test that `F(1)` draws one unit along `+Y` is written
    /// against it.
    pub fn upright() -> Self {
        Self {
            frame: Frame::new(
                Point3::origin(),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            ..Self::default()
        }
    }

    /// `TurtleParam::reset()` — back to the initial values, keeping nothing.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// `TurtleParam::transform(const Matrix3&)` — rotates the three axes.
    pub fn transform(&mut self, m: &Mat3) {
        self.frame.heading = m * self.frame.heading;
        self.frame.left = m * self.frame.left;
        self.frame.up = m * self.frame.up;
    }

    /// `TurtleParam::isValid()` — the basis is orthonormal.
    pub fn is_valid(&self) -> bool {
        self.frame.is_orthonormal(crate::math::EPSILON)
    }

    /// `TurtleParam::forward(l)` — advances along the heading, scaled by the
    /// state's `scale.z` as every upstream forward move is.
    pub fn forward(&mut self, length: Real) {
        self.frame.position += self.frame.heading * length * self.scale.z;
    }

    /// `isPolygonOn()`.
    pub fn is_polygon_on(&self) -> bool {
        self.polygon
    }

    /// `isGeneralizedCylinderOn()`.
    pub fn is_generalized_cylinder_on(&self) -> bool {
        self.generalized_cylinder
    }

    /// `isGeneralizedCylinderOnInit()` — open, but nothing drawn into it yet.
    pub fn is_generalized_cylinder_on_init(&self) -> bool {
        self.generalized_cylinder && self.points.len() <= 1
    }

    /// `isGCorPolygonOnInit()`.
    pub fn is_gc_or_polygon_on_init(&self) -> bool {
        (self.generalized_cylinder || self.polygon) && self.points.len() <= 1
    }

    /// The accumulated axis.
    pub fn points(&self) -> &[Point3] {
        &self.points
    }

    /// The `left` vector at each accumulated point.
    pub fn lefts(&self) -> &[Vec3] {
        &self.lefts
    }

    /// The width at each accumulated point.
    pub fn radii(&self) -> &[Real] {
        &self.radii
    }

    /// `polygon(bool)` — opens or closes polygon mode, clearing the points and
    /// snapshotting the appearance the way upstream does.
    pub fn set_polygon(&mut self, on: bool) {
        self.polygon = on;
        self.remove_points();
        if on {
            self.initial.color = self.draw.color;
            self.initial.custom_material = self.draw.custom_material.clone();
        } else {
            self.initial.reset();
        }
    }

    /// `generalizedCylinder(bool)`. Unlike polygon mode this snapshots the
    /// *whole* draw parameter block, cross-section included — a GC is swept
    /// from the profile it was opened with.
    pub fn set_generalized_cylinder(&mut self, on: bool) {
        self.generalized_cylinder = on;
        self.remove_points();
        if on {
            self.initial = self.draw.clone();
        } else {
            self.initial.reset();
        }
    }

    /// `pushPosition()` — records the current position, and for a generalized
    /// cylinder the `left` and width that go with it.
    pub fn push_position(&mut self) {
        self.points.push(self.frame.position);
        if self.generalized_cylinder {
            self.lefts.push(self.frame.left);
            self.radii.push(self.width);
        }
    }

    /// `pushRadius()` — rewrites the width of the last recorded point, so a
    /// `setWidth` inside a GC retroactively applies to the ring just placed.
    ///
    /// Upstream writes `*(radiusList.end()-1)` unconditionally, which is
    /// undefined behaviour on the empty list a GC has before its first point;
    /// the port leaves an empty list alone.
    pub fn push_radius(&mut self) {
        if let Some(last) = self.radii.last_mut() {
            *last = self.width;
        }
    }

    /// `keepLastPoint()` — what `push` does to an open GC, so the branch
    /// starts a fresh sweep from the point the trunk had reached.
    pub fn keep_last_point(&mut self) {
        if let Some(last) = self.points.last().copied() {
            self.points.clear();
            self.points.push(last);
        }
        if let Some(last) = self.lefts.last().copied() {
            self.lefts.clear();
            self.lefts.push(last);
        }
        if let Some(last) = self.radii.last().copied() {
            self.radii.clear();
            self.radii.push(last);
        }
    }

    /// `removePoints()`.
    pub fn remove_points(&mut self) {
        self.points.clear();
        self.lefts.clear();
        self.radii.clear();
    }

    /// `TurtleParam::setCrossSection(curve, ccw, defaultSection)` — also
    /// updates the snapshot while a GC is still empty, so setting the profile
    /// immediately after `startGC` affects that GC.
    pub fn set_cross_section(&mut self, curve: Curve2DRef, ccw: bool, default_section: bool) {
        self.draw.cross_section = Some(curve.clone());
        self.draw.cross_section_ccw = ccw;
        self.draw.default_section = default_section;
        if self.is_generalized_cylinder_on_init() {
            self.initial.cross_section = Some(curve);
            self.initial.cross_section_ccw = ccw;
            self.initial.default_section = default_section;
        }
    }

    /// `getTransformationMatrix()` — the frame with this state's scale.
    pub fn transformation_matrix(&self) -> crate::math::Mat4 {
        self.frame.to_matrix4_scaled(&self.scale)
    }

    /// `getOrientationMatrix()`.
    pub fn orientation_matrix(&self) -> Mat3 {
        self.frame.orientation()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenegraph::curve::{Curve2D, Polyline2D};

    #[test]
    fn the_default_state_is_upstreams() {
        let state = TurtleState::default();
        assert_eq!(state.frame.heading, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(state.frame.left, Vec3::new(0.0, -1.0, 0.0));
        assert_eq!(state.frame.up, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(state.width, 0.1);
        assert_eq!(state.draw.color, 1);
        assert_eq!(state.tropism, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(state.section_resolution, 8);
        assert!(state.is_valid());
    }

    #[test]
    fn the_upright_state_is_right_handed() {
        let state = TurtleState::upright();
        assert!(state.is_valid());
        assert_eq!(state.frame.heading, Vec3::y());
    }

    #[test]
    fn defaults_store_magnitudes() {
        let mut defaults = TurtleDefaults::default();
        defaults.set_step(-3.0);
        defaults.set_angle_increment(-45.0);
        defaults.set_width_increment(-0.5);
        defaults.set_color_increment(-2);
        defaults.set_scale_multiplier(-0.25);
        assert_eq!(defaults.step, 3.0);
        assert_eq!(defaults.angle_increment, 45.0);
        assert_eq!(defaults.width_increment, 0.5);
        assert_eq!(defaults.color_increment, 2);
        assert_eq!(defaults.scale_multiplier, 0.25);
    }

    #[test]
    fn a_generalized_cylinder_records_left_and_radius() {
        let mut state = TurtleState::default();
        state.set_generalized_cylinder(true);
        state.push_position();
        state.forward(1.0);
        state.push_position();
        assert_eq!(state.points().len(), 2);
        assert_eq!(state.lefts().len(), 2);
        assert_eq!(state.radii(), [0.1, 0.1]);

        state.width = 0.05;
        state.push_radius();
        assert_eq!(state.radii(), [0.1, 0.05]);

        state.keep_last_point();
        assert_eq!(state.points().len(), 1);
        assert_eq!(state.radii(), [0.05]);
    }

    #[test]
    fn a_polygon_records_positions_only() {
        let mut state = TurtleState::default();
        state.set_polygon(true);
        state.push_position();
        state.forward(1.0);
        state.push_position();
        assert_eq!(state.points().len(), 2);
        assert!(state.lefts().is_empty());
        assert!(state.radii().is_empty());
    }

    #[test]
    fn push_radius_on_an_empty_list_is_a_no_op() {
        // Upstream dereferences `radiusList.end()-1` here.
        let mut state = TurtleState::default();
        state.set_generalized_cylinder(true);
        state.width = 0.5;
        state.push_radius();
        assert!(state.radii().is_empty());
    }

    #[test]
    fn a_cross_section_set_while_a_gc_is_empty_reaches_the_snapshot() {
        let section = Curve2D::from(Polyline2D::circle(1.0, 6)).into_ref();
        let mut state = TurtleState::default();
        state.set_generalized_cylinder(true);
        state.set_cross_section(section.clone(), false, false);
        assert_eq!(state.initial.cross_section, Some(section.clone()));

        // Once the GC has two points it is no longer "on init", and the
        // snapshot stops following the live value.
        state.push_position();
        state.forward(1.0);
        state.push_position();
        let other = Curve2D::from(Polyline2D::circle(2.0, 6)).into_ref();
        state.set_cross_section(other.clone(), true, false);
        assert_eq!(state.initial.cross_section, Some(section));
        assert_eq!(state.draw.cross_section, Some(other));
    }

    #[test]
    fn forward_applies_the_z_scale() {
        let mut state = TurtleState {
            scale: Vec3::new(1.0, 1.0, 2.0),
            ..TurtleState::default()
        };
        state.forward(1.5);
        assert_eq!(state.frame.position, Point3::new(0.0, 0.0, 3.0));
    }
}
