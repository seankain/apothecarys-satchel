//! What a turtle draws into — the drawer interface and its shared inputs.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/modelling/turtledrawer.h`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! # Why the split exists
//!
//! Upstream separates `Turtle` — which knows the command set, the frame and
//! the stack — from `TurtleDrawer`, which knows what a command *produces*. One
//! turtle program can then emit a scene graph, a merged mesh, or nothing but
//! numbers. The port keeps the split and gains from it: these are plain trait
//! impls, where an FFI binding to upstream could not have offered them at all
//! (implementing a C++ abstract class from Rust puts Rust frames on a C++
//! unwind path).
//!
//! Every method defaults to drawing nothing, as upstream's do — a drawer
//! implements the commands it cares about. Unlike upstream they return
//! [`Result`], so a degenerate shape is reported rather than silently skipped.

use std::collections::BTreeMap;

use crate::error::Result;
use crate::math::{Frame, Point3, Real, Vec3};
use crate::scenegraph::appearance::AppearanceRef;
use crate::scenegraph::curve::Curve2DRef;
use crate::scenegraph::geometry::{Geometry, GeometryRef};
use crate::scenegraph::scene::NOID;

/// Upstream's `id_pair` — the shape id and the id of the shape it grew from.
///
/// The parent id is what lets a harvested organ be traced back to the axis it
/// hung off, which is the whole reason the turtle tracks ids at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IdPair {
    pub id: u32,
    pub parent_id: u32,
}

impl IdPair {
    pub fn new(id: u32, parent_id: u32) -> Self {
        Self { id, parent_id }
    }

    /// Both ids unset.
    pub fn none() -> Self {
        Self::new(NOID, NOID)
    }
}

impl Default for IdPair {
    fn default() -> Self {
        Self::none()
    }
}

/// What every drawer call is told about the turtle — upstream's `FrameInfo`
/// plus the ids and appearance it passes alongside.
///
/// `screenprojection` is not carried: `setScreenCoordinatesEnabled` draws a
/// heads-up overlay in upstream's viewer, and this crate has no viewer.
#[derive(Debug, Clone)]
pub struct DrawCtx<'a> {
    pub ids: IdPair,
    pub appearance: Option<AppearanceRef>,
    pub frame: &'a Frame,
    /// `TurtleParam::scale` — the per-axis scale a drawn shape carries.
    pub scaling: Vec3,
}

/// What a turtle command produces — upstream's `TurtleDrawer`.
///
/// The three implementations in this crate are
/// [`SceneDrawer`](crate::modelling::SceneDrawer),
/// [`MeshDrawer`](crate::modelling::MeshDrawer) and
/// [`MeasureDrawer`](crate::modelling::MeasureDrawer).
///
/// Deliberately not ported: `frame`, `arrow` and `vector` (debug
/// visualisation), `label` (text needs a font stack this crate does not have)
/// and the screen-projection wrapper.
pub trait TurtleDrawer {
    /// `reset()` — drop everything drawn so far.
    fn reset(&mut self) {}

    /// An `F` of constant width.
    fn cylinder(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        radius: Real,
        section_resolution: u32,
    ) -> Result<()> {
        let _ = (ctx, length, radius, section_resolution);
        Ok(())
    }

    /// An `F` that narrows or widens along its length.
    fn frustum(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        base_radius: Real,
        top_radius: Real,
        section_resolution: u32,
    ) -> Result<()> {
        let _ = (ctx, length, base_radius, top_radius, section_resolution);
        Ok(())
    }

    /// A whole accumulated axis, swept in one piece — `startGC` … `stopGC`.
    #[allow(clippy::too_many_arguments)]
    fn generalized_cylinder(
        &mut self,
        ctx: &DrawCtx,
        points: &[Point3],
        lefts: &[Vec3],
        radii: &[Real],
        cross_section: Curve2DRef,
        ccw: bool,
        section_resolution: u32,
    ) -> Result<()> {
        let _ = (
            ctx,
            points,
            lefts,
            radii,
            cross_section,
            ccw,
            section_resolution,
        );
        Ok(())
    }

    /// One `F` drawn as a sweep, because a cross-section has been set.
    ///
    /// Upstream's default builds a two-ring generalized cylinder and forwards,
    /// and so does this: a drawer that handles sweeps handles small ones.
    #[allow(clippy::too_many_arguments)]
    fn small_sweep(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
        cross_section: Curve2DRef,
        ccw: bool,
        section_resolution: u32,
    ) -> Result<()> {
        let points = [
            ctx.frame.position,
            ctx.frame.position + ctx.frame.heading * length * ctx.scaling.z,
        ];
        let lefts = [ctx.frame.left, ctx.frame.left];
        let radii = [bottom_radius, top_radius];
        self.generalized_cylinder(
            ctx,
            &points,
            &lefts,
            &radii,
            cross_section,
            ccw,
            section_resolution,
        )
    }

    fn sphere(&mut self, ctx: &DrawCtx, radius: Real, section_resolution: u32) -> Result<()> {
        let _ = (ctx, radius, section_resolution);
        Ok(())
    }

    fn circle(&mut self, ctx: &DrawCtx, radius: Real, section_resolution: u32) -> Result<()> {
        let _ = (ctx, radius, section_resolution);
        Ok(())
    }

    fn box3(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
    ) -> Result<()> {
        let _ = (ctx, length, bottom_radius, top_radius);
        Ok(())
    }

    fn quad(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
    ) -> Result<()> {
        let _ = (ctx, length, bottom_radius, top_radius);
        Ok(())
    }

    /// The accumulated outline — `startPolygon` … `stopPolygon`.
    fn polygon(&mut self, ctx: &DrawCtx, points: &[Point3], concave_test: bool) -> Result<()> {
        let _ = (ctx, points, concave_test);
        Ok(())
    }

    /// A template from the surface library, placed and scaled — how a leaf or
    /// a petal reaches the scene.
    fn custom_geometry(&mut self, ctx: &DrawCtx, geometry: &Geometry, scale: Real) -> Result<()> {
        let _ = (ctx, geometry, scale);
        Ok(())
    }
}

/// The named surfaces `surface(name, scale)` instances — upstream's
/// `PglTurtle::__surfList`.
///
/// This is how organs that are not tubes get into a plant: a leaf, a petal or
/// a fruit is modelled once as a [`Geometry`] — a patch, a triangle set, a
/// revolution — registered under a name, and then placed by the L-system
/// wherever the production says, oriented by the turtle's frame and scaled by
/// its argument.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurfaceLibrary {
    surfaces: BTreeMap<String, GeometryRef>,
}

impl SurfaceLibrary {
    pub fn new() -> Self {
        Self::default()
    }

    /// The default library upstream installs in `PglTurtle::defaultValue()`:
    /// one surface, `"l"`, a six-triangle leaf blade one unit long, pointed at
    /// both ends and half a unit across at its widest.
    pub fn with_default_leaf() -> Self {
        let mut library = Self::new();
        library.insert("l", default_leaf());
        library
    }

    /// `setSurface(name, geometry)`.
    pub fn insert(&mut self, name: impl Into<String>, geometry: GeometryRef) -> &mut Self {
        self.surfaces.insert(name.into(), geometry);
        self
    }

    /// `getSurface(name)`.
    pub fn get(&self, name: &str) -> Option<&GeometryRef> {
        self.surfaces.get(name)
    }

    /// `removeSurface(name)`.
    pub fn remove(&mut self, name: &str) -> Option<GeometryRef> {
        self.surfaces.remove(name)
    }

    /// `clearSurfaceList()`.
    pub fn clear(&mut self) {
        self.surfaces.clear();
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.surfaces.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }
}

/// Upstream's default `"l"` surface, vertex for vertex.
fn default_leaf() -> GeometryRef {
    use crate::scenegraph::mesh::TriangleSet;
    let third = 1.0 / 3.0;
    let points = vec![
        Point3::new(0.0, 0.0, 0.5),
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(0.0, -0.25, third),
        Point3::new(0.0, -0.25, 2.0 * third),
        Point3::new(0.0, 0.0, 1.0),
        Point3::new(0.0, 0.25, 2.0 * third),
        Point3::new(0.0, 0.25, third),
    ];
    let indices = vec![
        [0, 2, 1],
        [0, 3, 2],
        [0, 4, 3],
        [0, 5, 4],
        [0, 6, 5],
        [0, 1, 6],
    ];
    Geometry::from(TriangleSet::new(points, indices)).into_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::{discretize::discretize, measure::surface_area};

    #[test]
    fn the_default_library_carries_upstreams_leaf() {
        let library = SurfaceLibrary::with_default_leaf();
        assert_eq!(library.len(), 1);
        let leaf = library.get("l").expect("the default leaf");
        let area = surface_area(&discretize(leaf).unwrap()).unwrap();
        // Two half-blades a quarter wide over a unit of length: six triangles
        // fanned from the point a third of the way up, summing to 1/3.
        assert!((area - 1.0 / 3.0).abs() < 1e-5, "{area}");
        assert!(library.get("missing").is_none());
    }

    #[test]
    fn surfaces_can_be_added_and_removed() {
        let mut library = SurfaceLibrary::new();
        assert!(library.is_empty());
        library.insert("petal", default_leaf());
        assert_eq!(library.names().collect::<Vec<_>>(), ["petal"]);
        assert!(library.remove("petal").is_some());
        assert!(library.is_empty());
    }

    #[test]
    fn an_id_pair_defaults_to_no_id() {
        assert_eq!(IdPair::default(), IdPair::new(NOID, NOID));
    }
}
