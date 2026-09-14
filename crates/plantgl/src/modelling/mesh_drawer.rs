//! The drawer that builds triangles directly — the game's fast path.
//!
//! Original to this port; it has no upstream counterpart, because upstream
//! ships only the scene-graph drawer. It is nonetheless part of the CeCILL-C
//! crate and built on translated code; see crates/plantgl/LICENSE.
//!
//! A plant that will never be edited does not need a scene graph. This drawer
//! discretises each shape as the turtle draws it and concatenates the result
//! into one [`TriangleSet`] per appearance, so a plant arrives at the renderer
//! as a handful of buffers rather than a tree of a few hundred `Shape`s that
//! something else has to walk, merge and throw away.
//!
//! It draws the *same geometry* as [`SceneDrawer`](crate::modelling::SceneDrawer)
//! — both call [`crate::modelling::geometry`] — so the two agree shape for
//! shape, which `tests/turtle.rs` checks.

use std::sync::Arc;

use crate::algo::discretize::{DiscretizeCtx, Discretizer, Explicit};
use crate::algo::merge::merge_explicit;
use crate::algo::tessellate::tessellate;
use crate::error::Result;
use crate::math::{Point3, Real, Vec3};
use crate::modelling::drawer::{DrawCtx, TurtleDrawer};
use crate::modelling::geometry;
use crate::scenegraph::appearance::AppearanceRef;
use crate::scenegraph::curve::Curve2DRef;
use crate::scenegraph::geometry::Geometry;
use crate::scenegraph::mesh::TriangleSet;

/// One appearance's worth of triangles.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshBatch {
    /// The appearance every shape in this batch carried, or `None`.
    pub appearance: Option<AppearanceRef>,
    pub mesh: TriangleSet,
}

/// A [`TurtleDrawer`] that produces one merged [`TriangleSet`] per appearance.
#[derive(Debug, Clone)]
pub struct MeshDrawer {
    discretizer: Discretizer,
    /// Batches in first-appearance order, grouped by appearance identity —
    /// the same grouping [`crate::algo::merge::merge_scene`] uses, and the one
    /// a renderer's material binding cares about.
    batches: Vec<(Option<AppearanceRef>, Vec<Explicit>)>,
    samples: u32,
}

impl Default for MeshDrawer {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshDrawer {
    pub fn new() -> Self {
        Self::with_ctx(DiscretizeCtx::default())
    }

    /// A drawer at a given tessellation density — the LOD knob.
    pub fn with_ctx(ctx: DiscretizeCtx) -> Self {
        Self {
            discretizer: Discretizer::new(ctx),
            batches: Vec::new(),
            samples: ctx.curve_samples,
        }
    }

    /// The batches drawn so far, merged and tessellated.
    pub fn batches(&self) -> Result<Vec<MeshBatch>> {
        self.batches
            .iter()
            .map(|(appearance, parts)| {
                let merged = merge_explicit(parts.clone())?;
                Ok(MeshBatch {
                    appearance: appearance.clone(),
                    mesh: tessellate(&merged)?,
                })
            })
            .collect()
    }

    /// Every batch merged into one mesh, appearances discarded — what an
    /// inventory icon or a silhouette wants.
    pub fn merged(&self) -> Result<Option<TriangleSet>> {
        let parts: Vec<Explicit> = self
            .batches
            .iter()
            .flat_map(|(_, parts)| parts.iter().cloned())
            .collect();
        if parts.is_empty() {
            return Ok(None);
        }
        Ok(Some(tessellate(&merge_explicit(parts)?)?))
    }

    /// How many shapes have been drawn.
    pub fn shape_count(&self) -> usize {
        self.batches.iter().map(|(_, parts)| parts.len()).sum()
    }

    fn add(&mut self, ctx: &DrawCtx, geometry: Option<Geometry>) -> Result<()> {
        let Some(geometry) = geometry else {
            return Ok(());
        };
        let model = self.discretizer.discretize(&geometry)?;
        if matches!(model, Explicit::Polyline(_) | Explicit::PointSet(_)) {
            // A zero-radius tube is a skeleton, not a surface; it has no
            // triangles to merge. The scene drawer keeps it, this one does
            // not — a renderer cannot draw it either way.
            return Ok(());
        }
        let slot = self.batches.iter_mut().find(|(appearance, _)| {
            match (appearance, &ctx.appearance) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
        });
        match slot {
            Some((_, parts)) => parts.push(model),
            None => self.batches.push((ctx.appearance.clone(), vec![model])),
        }
        Ok(())
    }
}

impl TurtleDrawer for MeshDrawer {
    fn reset(&mut self) {
        self.batches.clear();
    }

    fn cylinder(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        radius: Real,
        section_resolution: u32,
    ) -> Result<()> {
        let geometry = geometry::cylinder(ctx, length, radius, section_resolution);
        self.add(ctx, geometry)
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
        self.add(ctx, geometry)
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
        self.add(ctx, geometry)
    }

    fn sphere(&mut self, ctx: &DrawCtx, radius: Real, section_resolution: u32) -> Result<()> {
        let geometry = geometry::sphere(ctx, radius, section_resolution);
        self.add(ctx, geometry)
    }

    fn circle(&mut self, ctx: &DrawCtx, radius: Real, section_resolution: u32) -> Result<()> {
        let geometry = geometry::circle(ctx, radius, section_resolution);
        self.add(ctx, geometry)
    }

    fn box3(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
    ) -> Result<()> {
        let geometry = geometry::box3(ctx, length, bottom_radius, top_radius);
        self.add(ctx, geometry)
    }

    fn quad(
        &mut self,
        ctx: &DrawCtx,
        length: Real,
        bottom_radius: Real,
        top_radius: Real,
    ) -> Result<()> {
        let geometry = geometry::quad(ctx, length, bottom_radius, top_radius);
        self.add(ctx, Some(geometry))
    }

    fn polygon(&mut self, ctx: &DrawCtx, points: &[Point3], concave_test: bool) -> Result<()> {
        let geometry = geometry::polygon(points, concave_test)?;
        self.add(ctx, geometry)
    }

    fn custom_geometry(&mut self, ctx: &DrawCtx, geometry: &Geometry, scale: Real) -> Result<()> {
        let geometry = geometry::custom_geometry(ctx, geometry, scale);
        self.add(ctx, geometry)
    }
}
