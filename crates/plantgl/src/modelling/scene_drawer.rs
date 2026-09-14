//! The drawer that builds a scene graph.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/modelling/pglturtledrawer.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189 — upstream's `PglTurtleDrawer`,
//! which is the only drawer it ships.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! What comes out is a [`Scene`] of parametric [`Shape`]s: a stem is still a
//! `Cylinder` and a leaf still a `BezierPatch`, so the result can be inspected,
//! re-tessellated at another density for an LOD tier, exported, or edited.
//! [`MeshDrawer`](crate::modelling::MeshDrawer) is the fast path that skips
//! all of that.

use crate::error::Result;
use crate::math::{Point3, Real, Vec3};
use crate::modelling::drawer::{DrawCtx, TurtleDrawer};
use crate::modelling::geometry;
use crate::scenegraph::curve::Curve2DRef;
use crate::scenegraph::geometry::Geometry;
use crate::scenegraph::scene::{Scene, Shape};

/// A [`TurtleDrawer`] that accumulates a [`Scene`].
#[derive(Debug, Clone, Default)]
pub struct SceneDrawer {
    scene: Scene,
    /// The density a sweep's cross-section is sampled at when the curve has no
    /// stride of its own.
    samples: u32,
}

impl SceneDrawer {
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
            samples: crate::algo::discretize::DiscretizeCtx::DEFAULT_CURVE_SAMPLES,
        }
    }

    /// The scene drawn so far.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Takes the scene, leaving the drawer empty.
    pub fn take_scene(&mut self) -> Scene {
        std::mem::take(&mut self.scene)
    }

    /// Consumes the drawer for its scene.
    pub fn into_scene(self) -> Scene {
        self.scene
    }

    /// `PglTurtleDrawer::_addToScene` — one shape, carrying the ids and the
    /// appearance the turtle had when it drew.
    fn add(&mut self, ctx: &DrawCtx, geometry: Geometry) {
        let mut shape = Shape::new(geometry.into_ref())
            .with_id(ctx.ids.id)
            .with_parent_id(ctx.ids.parent_id);
        shape.appearance = ctx.appearance.clone();
        self.scene.push(shape);
    }

    fn add_option(&mut self, ctx: &DrawCtx, geometry: Option<Geometry>) {
        if let Some(geometry) = geometry {
            self.add(ctx, geometry);
        }
    }
}

impl TurtleDrawer for SceneDrawer {
    fn reset(&mut self) {
        self.scene = Scene::new();
    }

    fn cylinder(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        radius: Real,
        section_resolution: u32,
    ) -> Result<()> {
        let geometry = geometry::cylinder(ctx, length, radius, section_resolution);
        self.add_option(ctx, geometry);
        Ok(())
    }

    fn frustum(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        base_radius: Real,
        top_radius: Real,
        section_resolution: u32,
    ) -> Result<()> {
        let geometry = geometry::frustum(ctx, length, base_radius, top_radius, section_resolution);
        self.add_option(ctx, geometry);
        Ok(())
    }

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
        let _ = section_resolution;
        let geometry =
            geometry::generalized_cylinder(points, lefts, radii, cross_section, ccw, self.samples)?;
        self.add_option(ctx, geometry);
        Ok(())
    }

    fn sphere(&mut self, ctx: &DrawCtx, radius: Real, section_resolution: u32) -> Result<()> {
        let geometry = geometry::sphere(ctx, radius, section_resolution);
        self.add_option(ctx, geometry);
        Ok(())
    }

    fn circle(&mut self, ctx: &DrawCtx, radius: Real, section_resolution: u32) -> Result<()> {
        let geometry = geometry::circle(ctx, radius, section_resolution);
        self.add_option(ctx, geometry);
        Ok(())
    }

    fn box3(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
    ) -> Result<()> {
        let geometry = geometry::box3(ctx, length, bottom_radius, top_radius);
        self.add_option(ctx, geometry);
        Ok(())
    }

    fn quad(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
    ) -> Result<()> {
        let geometry = geometry::quad(ctx, length, bottom_radius, top_radius);
        self.add(ctx, geometry);
        Ok(())
    }

    fn polygon(&mut self, ctx: &DrawCtx, points: &[Point3], concave_test: bool) -> Result<()> {
        let geometry = geometry::polygon(points, concave_test)?;
        self.add_option(ctx, geometry);
        Ok(())
    }

    fn custom_geometry(&mut self, ctx: &DrawCtx, geometry: &Geometry, scale: Real) -> Result<()> {
        let geometry = geometry::custom_geometry(ctx, geometry, scale);
        self.add_option(ctx, geometry);
        Ok(())
    }
}
