//! Parametric geometry to explicit meshes.
//!
//! Ported from PlantGL `src/cpp/plantgl/algo/base/discretizer.{h,cpp}`
//! @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! Every primitive's sampling — the point order, the index order and the
//! winding — is translated term for term from upstream's `process` methods, so
//! a mesh from this module has the same vertex numbering as the one Python
//! PlantGL produces. `tests/differential.rs` and `tools/differential/` check
//! that claim against a real upstream install.
//!
//! # Two deliberate divergences
//!
//! **Density has a fallback.** Upstream reads `slices`/`stacks` straight off
//! the primitive. [`DiscretizeCtx`] fills in for primitives that left theirs
//! unset, which is the LOD knob #21 needs; see
//! [`crate::scenegraph::primitive`].
//!
//! **Implicit zeros are written out.** Several of upstream's routines leave the
//! cap-centre slot of the point array unassigned and rely on `Point3Array`
//! zero-initialising it to the origin — true for the base of a `Cone`,
//! `Cylinder` and `Frustum`. The port assigns those points explicitly. The
//! mesh is identical; the intent is not left to the allocator.

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::math::{Point2, Point3, Real, Vec2};
use crate::scenegraph::curve::{
    BezierCurve, BezierPatch, Curve2DRef, Curve3D, NurbsCurve, NurbsPatch, ParametricCurve,
};
use crate::scenegraph::geometry::{Geometry, GeometryVisitor};
use crate::scenegraph::mesh::{
    ExplicitModel, FaceSet, Group, Index, Index3, Index4, PointSet, Polyline, QuadSet, TriangleSet,
};
use crate::scenegraph::primitive::{
    Box3, Cone, Cylinder, Disc, ElevationGrid, Extrusion, Frustum, Paraboloid, Revolution, Sphere,
    Swung, MIN_SLICES, MIN_STACKS,
};
use crate::scenegraph::transform::{Transform, Transformed};

use super::merge::merge_explicit;

/// The tessellation density a primitive falls back to when it does not carry
/// its own — upstream has no equivalent.
///
/// The defaults reproduce upstream's constants exactly, so a scene of
/// default-constructed primitives discretises to upstream's mesh. Lowering
/// them is the single biggest lever on triangle count, which is what the LOD
/// tiers in #21 turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiscretizeCtx {
    /// Subdivisions around an axis of revolution. `SOR::DEFAULT_SLICES`.
    pub slices: u8,
    /// Subdivisions along an axis of revolution. `Sphere::DEFAULT_STACKS`.
    pub stacks: u8,
    /// Segments a parametric curve is evaluated at. Upstream's curves carry a
    /// `Stride` field defaulting to 30.
    pub curve_samples: u32,
}

impl DiscretizeCtx {
    /// `Curve::DEFAULT_STRIDE`.
    pub const DEFAULT_CURVE_SAMPLES: u32 = 30;

    /// A context at some multiple of the default density, for LOD tiers.
    /// Rounds up, so a scale below one still leaves a buildable primitive.
    pub fn scaled(scale: Real) -> Self {
        let default = Self::default();
        let apply = |value: u8| {
            let scaled = (value as Real * scale).ceil();
            scaled.clamp(1.0, u8::MAX as Real) as u8
        };
        Self {
            slices: apply(default.slices),
            stacks: apply(default.stacks),
            curve_samples: ((default.curve_samples as Real * scale).ceil() as u32).max(1),
        }
    }
}

impl Default for DiscretizeCtx {
    fn default() -> Self {
        Self {
            slices: crate::scenegraph::primitive::DEFAULT_SLICES,
            stacks: Sphere::DEFAULT_STACKS,
            curve_samples: Self::DEFAULT_CURVE_SAMPLES,
        }
    }
}

/// What discretisation produces — upstream's `ExplicitModel` subtree, which is
/// wider than this crate's [`ExplicitModel`] payload struct.
///
/// The variant a primitive lands in is upstream's choice, not ours: a solid
/// cylinder is a [`FaceSet`] because its side quads and its cap triangles
/// share one index list, while an open one is a [`QuadSet`].
#[derive(Debug, Clone, PartialEq)]
pub enum Explicit {
    TriangleSet(TriangleSet),
    QuadSet(QuadSet),
    FaceSet(FaceSet),
    PointSet(PointSet),
    Polyline(Polyline),
}

impl Explicit {
    /// The vertex payload, whichever variant this is.
    pub fn model(&self) -> &ExplicitModel {
        match self {
            Explicit::TriangleSet(m) => &m.model,
            Explicit::QuadSet(m) => &m.model,
            Explicit::FaceSet(m) => &m.model,
            Explicit::PointSet(_) | Explicit::Polyline(_) => {
                unreachable!("point sets and polylines carry no ExplicitModel; use points()")
            }
        }
    }

    /// The positions, whichever variant this is.
    pub fn points(&self) -> &[Point3] {
        match self {
            Explicit::TriangleSet(m) => &m.model.points,
            Explicit::QuadSet(m) => &m.model.points,
            Explicit::FaceSet(m) => &m.model.points,
            Explicit::PointSet(p) => &p.points,
            Explicit::Polyline(p) => &p.points,
        }
    }

    /// Whether this model bounds a closed volume.
    pub fn is_solid(&self) -> bool {
        match self {
            Explicit::TriangleSet(m) => m.model.solid,
            Explicit::QuadSet(m) => m.model.solid,
            Explicit::FaceSet(m) => m.model.solid,
            Explicit::PointSet(_) | Explicit::Polyline(_) => false,
        }
    }

    /// The number of faces, or zero for a model that has none.
    pub fn face_count(&self) -> usize {
        match self {
            Explicit::TriangleSet(m) => m.face_count(),
            Explicit::QuadSet(m) => m.face_count(),
            Explicit::FaceSet(m) => m.face_count(),
            Explicit::PointSet(_) | Explicit::Polyline(_) => 0,
        }
    }

    /// Replaces every position, keeping the topology — how a deformation is
    /// applied after sampling.
    pub fn with_points(mut self, points: Vec<Point3>) -> Self {
        let points = Arc::new(points);
        match &mut self {
            Explicit::TriangleSet(m) => m.model.points = points,
            Explicit::QuadSet(m) => m.model.points = points,
            Explicit::FaceSet(m) => m.model.points = points,
            Explicit::PointSet(p) => p.points = points,
            Explicit::Polyline(p) => p.points = points,
        }
        self
    }

    /// Back into the scene graph.
    pub fn into_geometry(self) -> Geometry {
        match self {
            Explicit::TriangleSet(m) => Geometry::TriangleSet(m),
            Explicit::QuadSet(m) => Geometry::QuadSet(m),
            Explicit::FaceSet(m) => Geometry::FaceSet(m),
            Explicit::PointSet(p) => Geometry::PointSet(p),
            Explicit::Polyline(p) => Geometry::Polyline(p),
        }
    }

    /// The explicit model a geometry already is, if it is one.
    pub fn from_geometry(geometry: &Geometry) -> Option<Self> {
        match geometry {
            Geometry::TriangleSet(m) => Some(Explicit::TriangleSet(m.clone())),
            Geometry::QuadSet(m) => Some(Explicit::QuadSet(m.clone())),
            Geometry::FaceSet(m) => Some(Explicit::FaceSet(m.clone())),
            Geometry::PointSet(p) => Some(Explicit::PointSet(p.clone())),
            Geometry::Polyline(p) => Some(Explicit::Polyline(p.clone())),
            _ => None,
        }
    }
}

/// Upstream's `Discretizer` Action.
///
/// ```
/// use plantgl::algo::discretize::{DiscretizeCtx, Discretizer};
/// use plantgl::scenegraph::{Geometry, Sphere};
///
/// let mut discretizer = Discretizer::new(DiscretizeCtx { slices: 16, stacks: 16, ..Default::default() });
/// let mesh = discretizer.discretize(&Geometry::from(Sphere::sized(1.0))).unwrap();
/// // 2 triangles per slice per ring, and a sphere of n stacks has n - 1 rings.
/// assert_eq!(mesh.face_count(), 16 * 2 * 15);
/// ```
#[derive(Debug, Clone, Default)]
pub struct Discretizer {
    ctx: DiscretizeCtx,
    /// `__computeTexCoord` — whether to emit texture coordinates.
    compute_tex_coord: bool,
}

impl Discretizer {
    pub fn new(ctx: DiscretizeCtx) -> Self {
        Self {
            ctx,
            compute_tex_coord: true,
        }
    }

    /// A discretizer at upstream's default density.
    pub fn with_defaults() -> Self {
        Self::new(DiscretizeCtx::default())
    }

    /// `setTexCoordComputation(bool)`. Upstream defaults this to `false`; the
    /// port defaults it to `true`, because a mesh that reaches a renderer
    /// without UVs is a mesh that has to be discretised twice.
    pub fn with_tex_coords(mut self, compute: bool) -> Self {
        self.compute_tex_coord = compute;
        self
    }

    pub fn ctx(&self) -> &DiscretizeCtx {
        &self.ctx
    }

    /// Discretises a whole geometry tree, applying transformations and
    /// deformations as it descends.
    pub fn discretize(&mut self, geometry: &Geometry) -> Result<Explicit> {
        match geometry {
            Geometry::Box(g) => self.box3(g),
            Geometry::Sphere(g) => self.sphere(g),
            Geometry::Cone(g) => self.cone(g),
            Geometry::Cylinder(g) => self.cylinder(g),
            Geometry::Frustum(g) => self.frustum(g),
            Geometry::Disc(g) => self.disc(g),
            Geometry::Paraboloid(g) => self.paraboloid(g),
            Geometry::Revolution(g) => self.revolution(g),
            Geometry::Swung(g) => self.swung(g),
            Geometry::ElevationGrid(g) => self.elevation_grid(g),
            Geometry::BezierCurve(g) => self.bezier_curve(g),
            Geometry::NurbsCurve(g) => self.nurbs_curve(g),
            Geometry::BezierPatch(g) => self.bezier_patch(g),
            Geometry::NurbsPatch(g) => self.nurbs_patch(g),
            Geometry::Extrusion(g) => self.extrusion(g),
            Geometry::Group(g) => self.group(g),
            Geometry::Transformed(t) => self.transformed(t),
            explicit => Ok(Explicit::from_geometry(explicit)
                .expect("every remaining variant is an explicit model")),
        }
    }

    // --- Composition -------------------------------------------------------

    /// `process(Group*)` — upstream discretises the first child and merges
    /// every later one into it.
    fn group(&mut self, group: &Group) -> Result<Explicit> {
        group.is_valid()?;
        let mut parts = Vec::with_capacity(group.geometries.len());
        for child in &group.geometries {
            parts.push(self.discretize(child)?);
        }
        merge_explicit(parts)
    }

    /// `process(Translated*)` and its siblings, plus `process(Tapered*)`.
    ///
    /// An affine transform multiplies the sampled positions; a deformation is
    /// applied to them instead. Both act *after* sampling, which is the whole
    /// reason `Tapered` cannot be folded into an accumulated matrix — see
    /// [`crate::scenegraph::transform`].
    fn transformed(&mut self, transformed: &Transformed) -> Result<Explicit> {
        let child = self.discretize(&transformed.child)?;
        if let Some(deformation) = transformed.transform.to_deformation() {
            let deformed = deformation.transform(child.points());
            return Ok(child.with_points(deformed));
        }
        let matrix = transformed.transform.to_matrix4().ok_or_else(|| {
            Error::degenerate(format!(
                "{:?} has no transformation matrix",
                transformed.transform
            ))
        })?;
        let moved = child
            .points()
            .iter()
            .map(|p| Point3::from_homogeneous(matrix * p.to_homogeneous()).unwrap_or(*p))
            .collect();
        let mut out = child.with_points(moved);
        // A non-uniform scale or a shear invalidates the normals; the cheapest
        // correct answer is to drop them and let the caller recompute. A rigid
        // motion keeps them valid, so it keeps them.
        if !is_rigid(&transformed.transform) {
            out = out.without_normals();
        }
        Ok(out)
    }

    // --- Primitives --------------------------------------------------------

    /// `process(Box*)`.
    fn box3(&mut self, box3: &Box3) -> Result<Explicit> {
        box3.is_valid()?;
        let s = box3.size;
        let points = vec![
            Point3::new(s.x, -s.y, -s.z),
            Point3::new(-s.x, -s.y, -s.z),
            Point3::new(-s.x, s.y, -s.z),
            Point3::new(s.x, s.y, -s.z),
            Point3::new(s.x, -s.y, s.z),
            Point3::new(-s.x, -s.y, s.z),
            Point3::new(-s.x, s.y, s.z),
            Point3::new(s.x, s.y, s.z),
        ];
        let indices: Vec<Index4> = vec![
            [0, 1, 2, 3],
            [0, 3, 7, 4],
            [1, 0, 4, 5],
            [2, 1, 5, 6],
            [3, 2, 6, 7],
            [4, 7, 6, 5],
        ];

        let mut quads = QuadSet::new(points, indices);
        quads.model.normal_per_vertex = false;
        quads.model.ccw = true;
        quads.model.solid = true;
        quads.model.skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::new(0.0, 0.0, -s.z),
            Point3::new(0.0, 0.0, s.z),
        ])));

        if self.compute_tex_coord {
            // Upstream gives all six faces the same unit square, so a texture
            // is repeated once per face rather than unwrapped across the box.
            quads.model.tex_coords = Some(Arc::new(vec![
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 0.0),
                Vec2::new(0.0, 1.0),
                Vec2::new(1.0, 1.0),
            ]));
            quads.tex_coord_indices = Some(vec![[0, 1, 2, 3]; 6]);
        }

        Ok(Explicit::QuadSet(quads))
    }

    /// `process(Cone*)`.
    fn cone(&mut self, cone: &Cone) -> Result<Explicit> {
        cone.is_valid()?;
        let slices = self.slices(cone.slices) as usize;
        let (radius, height, solid) = (cone.radius, cone.height, cone.solid);

        let apex = slices as u32;
        let base_centre = apex + 1;
        let angle_step = std::f32::consts::TAU / slices as Real;

        let mut points = Vec::with_capacity(slices + 1 + usize::from(solid));
        let mut indices: Vec<Index3> = Vec::with_capacity(slices * (1 + usize::from(solid)));
        let mut tex_coords = Vec::with_capacity(slices + 2);
        let mut tex_indices: Vec<Index3> = Vec::with_capacity(indices.capacity());

        for i in 0..slices {
            let (sin, cos) = (i as Real * angle_step).sin_cos();
            points.push(Point3::new(cos * radius, sin * radius, 0.0));
            tex_coords.push(Vec2::new(0.5 + cos / 2.0, 0.5 + sin / 2.0));

            let cur = i as u32;
            let next = ((i + 1) % slices) as u32;
            indices.push([cur, next, apex]);
            // The texture ring is one longer than the point ring so the seam
            // gets u = 1 rather than wrapping back to u = 0.
            tex_indices.push([cur, cur + 1, apex]);

            if solid {
                indices.push([cur, base_centre, next]);
                tex_indices.push([cur, apex, cur + 1]);
            }
        }

        points.push(Point3::new(0.0, 0.0, height));
        tex_coords.push(Vec2::new(0.5, 0.5));
        if solid {
            // Upstream leaves this slot to Point3Array's zero-initialisation.
            points.push(Point3::origin());
            tex_coords.push(Vec2::new(1.0, 0.5));
        }

        let mut triangles = TriangleSet::new(points, indices);
        triangles.model.ccw = true;
        triangles.model.solid = solid;
        triangles.model.skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::origin(),
            Point3::new(0.0, 0.0, height),
        ])));
        if self.compute_tex_coord {
            triangles.model.tex_coords = Some(Arc::new(tex_coords));
            triangles.tex_coord_indices = Some(tex_indices);
        }
        Ok(Explicit::TriangleSet(triangles))
    }

    /// `process(Cylinder*)`.
    fn cylinder(&mut self, cylinder: &Cylinder) -> Result<Explicit> {
        cylinder.is_valid()?;
        let slices = self.slices(cylinder.slices);
        self.swept_tube(
            slices,
            cylinder.radius,
            cylinder.radius,
            cylinder.height,
            cylinder.solid,
            // Upstream's cylinder puts the *height* in v rather than 1, so a
            // texture tiles along a tall stem instead of stretching. The
            // frustum, built from the same code, does not.
            cylinder.height,
        )
    }

    /// `process(Frustum*)`.
    fn frustum(&mut self, frustum: &Frustum) -> Result<Explicit> {
        frustum.is_valid()?;
        let slices = self.slices(frustum.slices);
        self.swept_tube(
            slices,
            frustum.radius,
            frustum.top_radius(),
            frustum.height,
            frustum.solid,
            1.0,
        )
    }

    /// The body shared by `process(Cylinder*)` and `process(Frustum*)`, which
    /// upstream duplicates line for line but for the top radius and the v
    /// coordinate.
    fn swept_tube(
        &mut self,
        slices: u8,
        base_radius: Real,
        top_radius: Real,
        height: Real,
        solid: bool,
        top_v: Real,
    ) -> Result<Explicit> {
        let slices = slices as usize;
        let angle_step = std::f32::consts::TAU / slices as Real;

        let base_centre = (2 * slices) as u32;
        let top_centre = base_centre + 1;
        // Rings of 2 (or 3, with caps) texture coordinates per slice, plus the
        // duplicated seam column, plus the shared cap centre.
        let tex_stride = if solid { 3 } else { 2 };
        let cap_centre_tex = (tex_stride * (slices + 1)) as u32;

        let mut points = Vec::with_capacity(2 * slices + if solid { 2 } else { 0 });
        let mut tex_coords = Vec::with_capacity(tex_stride * (slices + 1) + usize::from(solid));
        let mut faces: Vec<Index> = Vec::with_capacity(slices * 3);
        let mut tex_faces: Vec<Index> = Vec::with_capacity(slices * 3);
        let mut quads: Vec<Index4> = Vec::with_capacity(slices);
        let mut tex_quads: Vec<Index4> = Vec::with_capacity(slices);

        for i in 0..slices {
            let (sin, cos) = (i as Real * angle_step).sin_cos();
            points.push(Point3::new(cos * base_radius, sin * base_radius, 0.0));
            points.push(Point3::new(cos * top_radius, sin * top_radius, height));

            let u = i as Real / slices as Real;
            tex_coords.push(Vec2::new(u, 0.0));
            tex_coords.push(Vec2::new(u, top_v));
            if solid {
                tex_coords.push(Vec2::new(0.5 * cos + 0.5, 0.5 * sin + 0.5));
            }

            let cur = (2 * i) as u32;
            let next = (2 * ((i + 1) % slices)) as u32;
            let cur_tex = (tex_stride * i) as u32;
            let next_tex = (tex_stride * (i + 1)) as u32;

            if solid {
                faces.push(vec![cur, next, next + 1, cur + 1]);
                faces.push(vec![cur + 1, next + 1, top_centre]);
                faces.push(vec![cur, base_centre, next]);
                tex_faces.push(vec![cur_tex, next_tex, next_tex + 1, cur_tex + 1]);
                tex_faces.push(vec![cur_tex + 2, next_tex + 2, cap_centre_tex]);
                tex_faces.push(vec![cur_tex + 2, cap_centre_tex, next_tex + 2]);
            } else {
                quads.push([cur, next, next + 1, cur + 1]);
                tex_quads.push([cur_tex, next_tex, next_tex + 1, cur_tex + 1]);
            }
        }

        if solid {
            // Both implicit in upstream's zero-initialised array but for the
            // top, which it does assign.
            points.push(Point3::origin());
            points.push(Point3::new(0.0, 0.0, height));
        }

        // The seam column, duplicating slice 0 at u = 1.
        tex_coords.push(Vec2::new(1.0, 0.0));
        tex_coords.push(Vec2::new(1.0, top_v));
        if solid {
            tex_coords.push(Vec2::new(1.0, 0.5));
            tex_coords.push(Vec2::new(0.5, 0.5));
        }

        let skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::origin(),
            Point3::new(0.0, 0.0, height),
        ])));

        if solid {
            let mut mesh = FaceSet::new(points, faces);
            mesh.model.ccw = true;
            mesh.model.solid = true;
            mesh.model.skeleton = skeleton;
            if self.compute_tex_coord {
                mesh.model.tex_coords = Some(Arc::new(tex_coords));
                mesh.tex_coord_indices = Some(tex_faces);
            }
            Ok(Explicit::FaceSet(mesh))
        } else {
            let mut mesh = QuadSet::new(points, quads);
            mesh.model.ccw = true;
            mesh.model.solid = false;
            mesh.model.skeleton = skeleton;
            if self.compute_tex_coord {
                mesh.model.tex_coords = Some(Arc::new(tex_coords));
                mesh.tex_coord_indices = Some(tex_quads);
            }
            Ok(Explicit::QuadSet(mesh))
        }
    }

    /// `process(Sphere*)`.
    fn sphere(&mut self, sphere: &Sphere) -> Result<Explicit> {
        sphere.is_valid()?;
        let slices = self.slices(sphere.slices) as usize;
        let stacks = self.stacks(sphere.stacks) as usize;
        let radius = sphere.radius;

        // Rings of points between the two poles; the poles themselves are the
        // last two entries.
        let rings = stacks - 1;
        let bottom = (slices * rings) as u32;
        let top = bottom + 1;

        let az_step = std::f32::consts::TAU / slices as Real;
        let el_step = std::f32::consts::PI / stacks as Real;

        let mut points = Vec::with_capacity(slices * rings + 2);
        let mut indices: Vec<Index3> = Vec::with_capacity(slices * 2 * rings);

        for i in 0..slices {
            let (sin_az, cos_az) = (i as Real * az_step).sin_cos();
            let cur = (i * rings) as u32;
            let next = (((i + 1) % slices) * rings) as u32;

            for j in 0..rings {
                let el = -std::f32::consts::FRAC_PI_2 + el_step * (j + 1) as Real;
                let (sin_el, cos_el) = el.sin_cos();
                points.push(Point3::new(
                    cos_az * cos_el * radius,
                    sin_az * cos_el * radius,
                    sin_el * radius,
                ));

                let j = j as u32;
                if j == 0 {
                    indices.push([cur, bottom, next]);
                    indices.push([cur + rings as u32 - 1, next + rings as u32 - 1, top]);
                } else {
                    indices.push([cur + j, cur + j - 1, next + j - 1]);
                    indices.push([cur + j, next + j - 1, next + j]);
                }
            }
        }

        points.push(Point3::new(0.0, 0.0, -radius));
        points.push(Point3::new(0.0, 0.0, radius));

        let mut mesh = TriangleSet::new(points, indices);
        mesh.model.ccw = true;
        mesh.model.solid = true;
        mesh.model.skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::new(0.0, 0.0, -radius),
            Point3::new(0.0, 0.0, radius),
        ])));

        if self.compute_tex_coord {
            let (tex_coords, tex_indices) = sphere_tex_coords(slices, stacks);
            mesh.model.tex_coords = Some(Arc::new(tex_coords));
            mesh.tex_coord_indices = Some(tex_indices);
        }
        Ok(Explicit::TriangleSet(mesh))
    }

    /// `process(Disc*)`.
    fn disc(&mut self, disc: &Disc) -> Result<Explicit> {
        disc.is_valid()?;
        let slices = self.slices(disc.slices) as usize;
        let radius = disc.radius;
        let centre = slices as u32;
        let angle_step = std::f32::consts::TAU / slices as Real;

        let mut points = Vec::with_capacity(slices + 1);
        let mut tex_coords = Vec::with_capacity(slices + 1);
        let mut indices: Vec<Index3> = Vec::with_capacity(slices);

        for i in 0..slices {
            let (sin, cos) = (i as Real * angle_step).sin_cos();
            points.push(Point3::new(cos * radius, sin * radius, 0.0));
            // Planar UV: the disc maps onto the unit square's inscribed
            // circle, so a leaf texture lands the right way up.
            tex_coords.push(Vec2::new(cos / 2.0 + 0.5, sin / 2.0 + 0.5));
            indices.push([i as u32, ((i + 1) % slices) as u32, centre]);
        }
        points.push(Point3::origin());
        tex_coords.push(Vec2::new(0.5, 0.5));

        let mut mesh = TriangleSet::new(points, indices);
        mesh.model.ccw = true;
        mesh.model.solid = false;
        mesh.model.skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::origin(),
            Point3::origin(),
        ])));
        if self.compute_tex_coord {
            mesh.model.tex_coords = Some(Arc::new(tex_coords));
        }
        Ok(Explicit::TriangleSet(mesh))
    }

    /// `process(Paraboloid*)`.
    fn paraboloid(&mut self, paraboloid: &Paraboloid) -> Result<Explicit> {
        paraboloid.is_valid()?;
        let slices = self.slices(paraboloid.slices) as usize;
        let stacks = self.stacks(paraboloid.stacks) as usize;
        let (radius, height, shape, solid) = (
            paraboloid.radius,
            paraboloid.height,
            paraboloid.shape,
            paraboloid.solid,
        );

        let stacks_by_slices = stacks * slices;
        let bottom = stacks_by_slices as u32;
        let top = bottom + u32::from(solid);

        let angle_step = std::f32::consts::TAU / slices as Real;
        let radius_step = radius / stacks as Real;

        let mut points = Vec::with_capacity(stacks_by_slices + 1 + usize::from(solid));
        let mut indices: Vec<Index3> = Vec::with_capacity(stacks_by_slices * 2);

        for i in 0..slices {
            let (sin, cos) = (i as Real * angle_step).sin_cos();
            let cur = (i * stacks) as u32;
            let next = (((i + 1) % slices) * stacks) as u32;

            points.push(Point3::new(cos * radius, sin * radius, 0.0));
            if solid {
                indices.push([cur, bottom, next]);
            }

            let mut r = radius;
            for j in 1..stacks {
                r -= radius_step;
                let z = height * (1.0 - (r / radius).powf(shape));
                points.push(Point3::new(cos * r, sin * r, z));

                let j = j as u32;
                indices.push([cur + j, cur + j - 1, next + j - 1]);
                indices.push([cur + j, next + j - 1, next + j]);
            }
            indices.push([cur + stacks as u32 - 1, next + stacks as u32 - 1, top]);
        }

        if solid {
            points.push(Point3::origin());
        }
        points.push(Point3::new(0.0, 0.0, height));

        let mut mesh = TriangleSet::new(points, indices);
        mesh.model.ccw = true;
        // Upstream hardcodes `solid` here rather than passing the field, so an
        // open paraboloid is still reported as closing a volume. That is a
        // measurement bug — `VolComputer` would return a volume for a shape
        // with an open base — so the port passes the field through.
        mesh.model.solid = solid;
        mesh.model.skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::origin(),
            Point3::new(0.0, 0.0, height),
        ])));
        Ok(Explicit::TriangleSet(mesh))
    }

    /// `process(Revolution*)`.
    fn revolution(&mut self, revolution: &Revolution) -> Result<Explicit> {
        revolution.is_valid(self.ctx.curve_samples)?;
        let profile = revolution.profile.discretize(self.ctx.curve_samples)?;
        let slices = self.slices(revolution.slices) as usize;
        let solid = profile_closes_a_volume(&profile);
        let (sections, angles) = full_turn(&profile, slices);
        self.sweep_profiles(&sections, &angles, solid, true)
    }

    /// `process(Swung*)`.
    ///
    /// Upstream routes every swung surface through `ProfileInterpolation`,
    /// which *fits* a NURBS of `Swung::degree` through the profiles — a global
    /// interpolation problem, not an evaluation, so the spline evaluators of
    /// Phase C (#19) do not supply it. Degree 1 — piecewise-linear blending —
    /// is translated here; higher degrees are reported as unsupported rather
    /// than approximated, because a silently-linear blend of what was asked to
    /// be a cubic is a wrong surface that still looks plausible.
    fn swung(&mut self, swung: &Swung) -> Result<Explicit> {
        swung.is_valid(self.ctx.curve_samples)?;
        if swung.degree > Swung::MAX_PORTED_DEGREE && swung.profiles.len() > 1 {
            return Err(Error::unsupported(format!(
                "Swung interpolation of degree {} needs upstream's ProfileInterpolation, \
                 which is not ported; \
                 degree 1 is available today",
                swung.degree
            )));
        }

        let stride = self.swung_stride(swung)?;
        let sections = self.resample_profiles(&swung.profiles, stride)?;
        let slices = self.slices(swung.slices) as usize;

        // A single profile has nothing to blend with, so upstream's
        // interpolation collapses to sweeping it unchanged — a `Revolution`
        // over the full turn.
        if sections.len() == 1 {
            let solid = profile_closes_a_volume(&sections[0]);
            let (sections, angles) = full_turn(&sections[0], slices);
            return self.sweep_profiles(&sections, &angles, solid, swung.ccw);
        }

        let angle_min = swung.angles[0];
        let angle_max = swung.angles[swung.angles.len() - 1];
        let angle_step = (angle_max - angle_min) / slices as Real;

        let angles: Vec<Real> = (0..slices)
            .map(|slice| angle_min + angle_step * slice as Real)
            .collect();
        let blended: Vec<Vec<Point2>> = angles
            .iter()
            .map(|angle| blend_sections(&sections, &swung.angles, *angle, stride))
            .collect();

        // Upstream marks the result solid only when every slice starts and
        // ends at the same point, so a surface that does not close is not
        // measured as if it did.
        let closed = blended.iter().all(|section| {
            (section[0] - blended[0][0]).norm() <= SWUNG_CLOSURE_EPSILON
                && (section[section.len() - 1] - blended[0][blended[0].len() - 1]).norm()
                    <= SWUNG_CLOSURE_EPSILON
        });

        self.sweep_profiles(&blended, &angles, closed, swung.ccw)
    }

    /// The shared sweep both `Revolution` and `Swung` end in: place each
    /// section at its angle and stitch it to the next one.
    ///
    /// `sections[i]` is the profile at `angles[i]`, one per slice.
    ///
    /// # Poles are collapsed, unlike upstream
    ///
    /// Where a profile touches the axis of revolution, every slice's copy of
    /// that point lands on the *same* position. Upstream sweeps the strip
    /// regardless, so each slice contributes a zero-area triangle spanning two
    /// coincident vertices, and the zero-length edge between them belongs to
    /// exactly one face — the mesh is not edge-manifold, its pole normals are
    /// whatever `DEFAULT_NORMAL_VALUE` happens to be, and a renderer shows the
    /// seam. That is precisely the tip of every leaf, bud and fruit a
    /// `Revolution` is used for.
    ///
    /// The port emits one shared apex vertex per pole and a proper fan into it,
    /// so the surface is watertight and the degenerate triangles never exist.
    /// The result has `slices` fewer triangles per pole than upstream's;
    /// `tools/differential/` knows about the difference and compares the
    /// non-degenerate counts.
    fn sweep_profiles(
        &mut self,
        sections: &[Vec<Point2>],
        angles: &[Real],
        solid: bool,
        ccw: bool,
    ) -> Result<Explicit> {
        let slices = sections.len();
        if slices < MIN_SLICES as usize || angles.len() != slices {
            return Err(Error::degenerate(format!(
                "a sweep needs at least {MIN_SLICES} sections with one angle each, \
                 got {slices} and {}",
                angles.len()
            )));
        }
        let section_size = sections[0].len();
        if let Some((i, section)) = sections
            .iter()
            .enumerate()
            .find(|(_, s)| s.len() != section_size)
        {
            return Err(Error::degenerate(format!(
                "swept section {i} has {} points, the first had {section_size}",
                section.len()
            )));
        }

        // A point on the axis is shared by every slice rather than duplicated.
        let on_axis = |p: &Point2| p.x.abs() <= crate::math::EPSILON;
        let bottom_pole = sections.iter().all(|s| on_axis(&s[0]));
        let top_pole = sections.iter().all(|s| on_axis(&s[section_size - 1]));

        let ring_start = usize::from(bottom_pole);
        let ring_end = section_size - usize::from(top_pole);
        let ring_count = ring_end.saturating_sub(ring_start);
        if ring_count == 0 {
            return Err(Error::degenerate(
                "a swept profile that lies entirely on the axis of revolution \
                 sweeps to a line, not a surface",
            ));
        }

        let apex_base = (slices * ring_count) as u32;
        let bottom_apex = apex_base;
        let top_apex = apex_base + u32::from(bottom_pole);

        let mut points =
            Vec::with_capacity(slices * ring_count + usize::from(bottom_pole) + usize::from(top_pole));
        let mut indices: Vec<Index3> =
            Vec::with_capacity(slices * (2 * ring_count.saturating_sub(1) + 2));

        for (i, section) in sections.iter().enumerate() {
            let (sin, cos) = angles[i].sin_cos();
            let cur = (i * ring_count) as u32;
            let next = (((i + 1) % slices) * ring_count) as u32;

            for p in &section[ring_start..ring_end] {
                points.push(Point3::new(p.x * cos, p.x * sin, p.y));
            }

            if bottom_pole {
                indices.push([cur, bottom_apex, next]);
            }
            for j in 1..ring_count as u32 {
                indices.push([cur + j, cur + j - 1, next + j - 1]);
                indices.push([cur + j, next + j - 1, next + j]);
            }
            if top_pole {
                let last = ring_count as u32 - 1;
                indices.push([top_apex, cur + last, next + last]);
            }
        }

        if bottom_pole {
            points.push(Point3::new(0.0, 0.0, sections[0][0].y));
        }
        if top_pole {
            points.push(Point3::new(0.0, 0.0, sections[0][section_size - 1].y));
        }

        let mut mesh = TriangleSet::new(points, indices);
        mesh.model.ccw = ccw;
        mesh.model.solid = solid;
        mesh.model.skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::origin(),
            Point3::new(0.0, 0.0, 1.0),
        ])));

        if self.compute_tex_coord {
            // Cylindrical UV: u runs with the sweep and v along the profile's
            // arc length, so a bark texture does not bunch where the profile
            // is sampled densely.
            let vs = arc_length_parameters(&sections[0]);
            let mut tex_coords = Vec::with_capacity(mesh.model.point_count());
            for i in 0..slices {
                let u = i as Real / slices as Real;
                for v in &vs[ring_start..ring_end] {
                    tex_coords.push(Vec2::new(u, *v));
                }
            }
            // A pole is one vertex for the whole sweep, so it gets one u; the
            // middle of the range is the least-wrong choice available.
            if bottom_pole {
                tex_coords.push(Vec2::new(0.5, vs[0]));
            }
            if top_pole {
                tex_coords.push(Vec2::new(0.5, vs[section_size - 1]));
            }
            mesh.model.tex_coords = Some(Arc::new(tex_coords));
        }
        Ok(Explicit::TriangleSet(mesh))
    }

    /// `process(ElevationGrid*)`.
    fn elevation_grid(&mut self, grid: &ElevationGrid) -> Result<Explicit> {
        grid.is_valid()?;
        let x_dim = grid.heights.x_dim();
        let y_dim = grid.heights.y_dim();

        let mut points = Vec::with_capacity(x_dim * y_dim);
        let mut indices: Vec<Index3> = Vec::with_capacity((x_dim - 1) * (y_dim - 1) * 2);

        for j in 0..y_dim {
            for i in 0..x_dim {
                points.push(
                    grid.point_at(i, j)
                        .ok_or_else(|| Error::invalid_index(format!("grid sample ({i}, {j})")))?,
                );
                if i + 1 < x_dim && j + 1 < y_dim {
                    let cur = (j * x_dim + i) as u32;
                    let next = cur + 1;
                    let stride = x_dim as u32;
                    indices.push([cur, next, cur + stride]);
                    indices.push([next, next + stride, cur + stride]);
                }
            }
        }

        let mut mesh = TriangleSet::new(points, indices);
        mesh.model.ccw = grid.ccw;
        mesh.model.solid = false;
        mesh.model.skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::origin(),
            Point3::origin(),
        ])));
        if self.compute_tex_coord {
            // Planar UV over the lattice: the grid is regular in x and y, so
            // the arc-length parameterisation upstream computes reduces to
            // the index ratio and this says the same thing directly.
            let mut tex_coords = Vec::with_capacity(x_dim * y_dim);
            for j in 0..y_dim {
                for i in 0..x_dim {
                    tex_coords.push(Vec2::new(
                        i as Real / (x_dim - 1) as Real,
                        j as Real / (y_dim - 1) as Real,
                    ));
                }
            }
            mesh.model.tex_coords = Some(Arc::new(tex_coords));
        }
        Ok(Explicit::TriangleSet(mesh))
    }

    // --- Curves, patches and the generalized cylinder -----------------------

    /// `process(BezierCurve*)`.
    fn bezier_curve(&mut self, curve: &BezierCurve) -> Result<Explicit> {
        let width = curve.width;
        self.lineic(Curve3D::BezierCurve(curve.clone()), width)
    }

    /// `process(NurbsCurve*)`.
    fn nurbs_curve(&mut self, curve: &NurbsCurve) -> Result<Explicit> {
        let width = curve.width;
        self.lineic(Curve3D::NurbsCurve(curve.clone()), width)
    }

    /// Both curve cases: the polyline through `stride + 1` samples, ending on
    /// the last knot exactly.
    fn lineic(&mut self, curve: Curve3D, width: u32) -> Result<Explicit> {
        let mut polyline = Polyline::new(curve.discretize(self.ctx.curve_samples)?);
        polyline.width = width;
        Ok(Explicit::Polyline(polyline))
    }

    /// `process(BezierPatch*)`.
    fn bezier_patch(&mut self, patch: &BezierPatch) -> Result<Explicit> {
        patch.is_valid()?;
        let (u_samples, v_samples) = self.patch_samples(patch.u_stride, patch.v_stride);
        self.patch_mesh(u_samples, v_samples, patch.ccw, |u, v| patch.eval(u, v))
    }

    /// `process(NurbsPatch*)`.
    ///
    /// Upstream samples the patch across its own knot range rather than
    /// `[0, 1]`; the closure maps the unit square onto it so the two patch
    /// cases share one meshing routine.
    fn nurbs_patch(&mut self, patch: &NurbsPatch) -> Result<Explicit> {
        patch.is_valid()?;
        let (u_samples, v_samples) = self.patch_samples(patch.u_stride, patch.v_stride);
        let (u_first, u_last) = (patch.first_u_knot(), patch.last_u_knot());
        let (v_first, v_last) = (patch.first_v_knot(), patch.last_v_knot());
        self.patch_mesh(u_samples, v_samples, patch.ccw, |u, v| {
            patch.eval(
                u_first + (u_last - u_first) * u,
                v_first + (v_last - v_first) * v,
            )
        })
    }

    /// The shared patch meshing: a `u_samples × v_samples` lattice of quads,
    /// point order u-major with `v` running fastest, as upstream emits it.
    fn patch_mesh<F>(
        &mut self,
        u_samples: usize,
        v_samples: usize,
        ccw: bool,
        eval: F,
    ) -> Result<Explicit>
    where
        F: Fn(Real, Real) -> Result<Point3>,
    {
        let mut points = Vec::with_capacity(u_samples * v_samples);
        let mut indices: Vec<Index4> = Vec::with_capacity((u_samples - 1) * (v_samples - 1));
        for i in 0..u_samples {
            let u = i as Real / (u_samples - 1) as Real;
            for j in 0..v_samples {
                let v = j as Real / (v_samples - 1) as Real;
                points.push(eval(u, v)?);
                if i + 1 < u_samples && j + 1 < v_samples {
                    let cur = (i * v_samples + j) as u32;
                    let stride = v_samples as u32;
                    indices.push([cur, cur + 1, cur + stride + 1, cur + stride]);
                }
            }
        }

        let tex_coords = self
            .compute_tex_coord
            .then(|| grid_tex_coords(&points, u_samples, v_samples));

        let mut mesh = QuadSet::new(points, indices);
        mesh.model.ccw = ccw;
        mesh.model.solid = false;
        // Upstream gives a patch a degenerate skeleton — two copies of the
        // origin — because a surface has no axis to speak of.
        mesh.model.skeleton = Some(Arc::new(Polyline::new(vec![
            Point3::origin(),
            Point3::origin(),
        ])));
        mesh.model.tex_coords = tex_coords.map(Arc::new);
        Ok(Explicit::QuadSet(mesh))
    }

    /// `process(Extrusion*)` — the generalized cylinder.
    ///
    /// The axis is sampled, a frame chain is carried along it, and a copy of
    /// the cross-section is placed in each frame and stitched to the next.
    ///
    /// # The frames are rotation-minimising, unlike upstream's
    ///
    /// Upstream's `getNextFrameAt` re-derives each frame from the *previous
    /// binormal* crossed with the new tangent. That is the projection method:
    /// its per-step error in the twist is second order in the step, so a sweep
    /// along a curve with torsion — a helix, a tendril, a drooping stem —
    /// accumulates a visible rotation of the cross-section that gets worse as
    /// the axis is sampled more coarsely.
    ///
    /// The port carries the frames by the double-reflection method of Wang,
    /// Jüttler, Zheng and Liu (2008), [`Frame::propagate`], whose error is
    /// fourth order. The two agree exactly on a straight axis — no rotation to
    /// minimise — and converge on any other, so this is a divergence in the
    /// vertex *phase* around each ring, not in the surface being swept. The
    /// differential harness pins both halves of that claim: identical meshes on
    /// a straight axis, and a bounded difference on a helix.
    fn extrusion(&mut self, extrusion: &Extrusion) -> Result<Explicit> {
        let samples = self.ctx.curve_samples;
        extrusion.is_valid(samples)?;

        // A closed cross-section repeats its first point. Drop the duplicate
        // and stitch the ring round instead, so the seam has no double vertex.
        let mut section = extrusion.cross_section.discretize(samples)?;
        let closed = section.len() > 2
            && (section[section.len() - 1] - section[0]).norm() <= crate::math::EPSILON;
        if closed {
            section.pop();
        }
        let ring_size = section.len();
        if ring_size < 2 {
            return Err(Error::degenerate(
                "an extrusion cross-section needs at least 2 distinct points",
            ));
        }

        // One rotation-minimising frame per axis sample.
        let axis = extrusion.axis.as_ref();
        let parameters = axis.parameters(samples);
        let rings = parameters.len();
        let mut frame = axis.initial_frame()?;
        let mut frames = Vec::with_capacity(rings);
        frames.push(frame);
        for u in parameters.iter().skip(1) {
            let position = axis.eval(*u)?;
            let heading = axis.tangent(*u)?;
            frame = frame.propagate(position, heading).ok_or_else(|| {
                Error::degenerate(format!(
                    "the extrusion axis has no tangent at u = {u}, so the sweep cannot be framed"
                ))
            })?;
            frames.push(frame);
        }

        let (u_min, u_max) = (extrusion.u_min(), extrusion.u_max());
        let section_lengths = arc_length_parameters(&section_with_seam(&section, closed));
        let axis_lengths = cumulative_fractions(&frames);

        let mut points = Vec::with_capacity(rings * ring_size);
        let mut indices: Vec<Index4> = Vec::with_capacity((rings - 1) * ring_size);
        // A closed ring's texture seam needs the first vertex twice — once at
        // u = 0 and once at u = 1 — so texture coordinates are indexed
        // separately, exactly as upstream does it.
        let tex_ring = ring_size + usize::from(closed);
        let mut tex_coords = Vec::with_capacity(rings * tex_ring);
        let mut tex_indices: Vec<Index4> = Vec::with_capacity((rings - 1) * ring_size);

        for (ring, frame) in frames.iter().enumerate() {
            let along = ring as Real / (rings - 1) as Real;
            let (scale, twist) = extrusion.profile_at(u_min + (u_max - u_min) * along);
            let (sin, cos) = twist.sin_cos();
            let base = (ring * ring_size) as u32;
            let tex_base = (ring * tex_ring) as u32;

            for (i, p) in section.iter().enumerate() {
                // Upstream composes the profile as `scale * orientation`, so
                // the twist is applied first; its rotation matrix is
                // `Matrix2(c, s, -s, c)`, which turns the cross-section
                // clockwise for a positive angle.
                let (x, y) = (p.x * cos + p.y * sin, -p.x * sin + p.y * cos);
                let (x, y) = (x * scale.x, y * scale.y);
                points.push(frame.position + frame.left * x + frame.up * y);

                if self.compute_tex_coord {
                    tex_coords.push(Vec2::new(section_lengths[i], axis_lengths[ring]));
                }
                if ring + 1 == rings {
                    continue;
                }
                let (cur, ahead) = (base + i as u32, base + ((i + 1) % ring_size) as u32);
                if i + 1 < ring_size || closed {
                    indices.push([
                        cur,
                        ahead,
                        ahead + ring_size as u32,
                        cur + ring_size as u32,
                    ]);
                    let (t_cur, t_ahead) = (tex_base + i as u32, tex_base + i as u32 + 1);
                    tex_indices.push([
                        t_cur,
                        t_ahead,
                        t_ahead + tex_ring as u32,
                        t_cur + tex_ring as u32,
                    ]);
                }
            }
            if closed && self.compute_tex_coord {
                // The seam vertex again, at the far end of the texture.
                tex_coords.push(Vec2::new(1.0, axis_lengths[ring]));
            }
        }

        let mut mesh = if extrusion.solid {
            // Cap both ends. Upstream triangulates the two rings as fans and
            // puts them before the swept quads, which makes the result a
            // FaceSet rather than a QuadSet.
            //
            // # The first cap is reversed, unlike upstream
            //
            // Upstream fans both rings in the same order —
            // `range<Index>(nbPoints, 0, 1)` and the same from the last ring —
            // and a cross-section wound counter-clockwise in the frame's
            // `(left, up)` plane fans to a normal along `+heading`. At the far
            // end that points out of the solid; at the near end it points
            // straight into it, so upstream's every solid `Extrusion` has an
            // inverted base. Nothing upstream notices: `VolComputer` sums the
            // *absolute* tetrahedra about the centroid, so a flipped face is
            // invisible to it, and the face count and area are unchanged.
            //
            // It is not invisible to a renderer, to a signed volume, or to any
            // caller that trusts a `solid` mesh's normals. The port reverses
            // the near cap. The differential harness records the inward-facing
            // face count on both sides, so this is asserted as a divergence
            // rather than assumed.
            let mut faces: Vec<Index> = Vec::with_capacity(indices.len() + 2 * (ring_size - 2));
            let mut tex_faces: Vec<Index> = Vec::with_capacity(faces.capacity());
            let last_ring = ((rings - 1) * ring_size) as u32;
            let last_tex_ring = ((rings - 1) * tex_ring) as u32;
            for (base, tex_base, reversed) in
                [(0, 0, true), (last_ring, last_tex_ring, false)]
            {
                for i in 1..ring_size as u32 - 1 {
                    let (a, b) = if reversed { (i + 1, i) } else { (i, i + 1) };
                    faces.push(vec![base, base + a, base + b]);
                    tex_faces.push(vec![tex_base, tex_base + a, tex_base + b]);
                }
            }
            faces.extend(indices.iter().map(|quad| quad.to_vec()));
            tex_faces.extend(tex_indices.iter().map(|quad| quad.to_vec()));

            let mut mesh = FaceSet::new(points, faces);
            if self.compute_tex_coord && closed {
                mesh.tex_coord_indices = Some(tex_faces);
            }
            mesh.model.solid = true;
            Explicit::FaceSet(mesh)
        } else {
            let mut mesh = QuadSet::new(points, indices);
            if self.compute_tex_coord && closed {
                mesh.tex_coord_indices = Some(tex_indices);
            }
            mesh.model.solid = false;
            Explicit::QuadSet(mesh)
        };

        {
            let model = match &mut mesh {
                Explicit::FaceSet(m) => &mut m.model,
                Explicit::QuadSet(m) => &mut m.model,
                _ => unreachable!("an extrusion meshes to a face set or a quad set"),
            };
            model.ccw = extrusion.ccw;
            // The axis is the mesh's skeleton, which is what a later
            // measurement of the swept volume reads.
            model.skeleton = Some(Arc::new(Polyline::new(
                frames.iter().map(|f| f.position).collect(),
            )));
            if self.compute_tex_coord {
                model.tex_coords = Some(Arc::new(tex_coords));
            }
        }
        Ok(mesh)
    }

    /// A patch's sample counts. Upstream's patch `UStride` counts *samples*
    /// where its curve `Stride` counts *segments*; both names are kept as
    /// upstream uses them, and a patch that leaves its strides unset takes
    /// [`DiscretizeCtx::curve_samples`] as its sample count — 30 either way, so
    /// a default patch meshes to upstream's.
    fn patch_samples(&self, u: Option<u32>, v: Option<u32>) -> (usize, usize) {
        (
            u.unwrap_or(self.ctx.curve_samples).max(2) as usize,
            v.unwrap_or(self.ctx.curve_samples).max(2) as usize,
        )
    }

    // --- Density resolution ------------------------------------------------

    fn slices(&self, own: Option<u8>) -> u8 {
        crate::scenegraph::primitive::resolve(own, self.ctx.slices, MIN_SLICES)
    }

    fn stacks(&self, own: Option<u8>) -> u8 {
        crate::scenegraph::primitive::resolve(own, self.ctx.stacks, MIN_STACKS)
    }

    /// `ProfileInterpolation::interpol`'s stride resolution: an explicit
    /// stride wins, otherwise the coarsest profile's own, with a floor of 2.
    fn swung_stride(&self, swung: &Swung) -> Result<u32> {
        if swung.stride > 2 {
            return Ok(swung.stride);
        }
        let mut stride = 0;
        for profile in &swung.profiles {
            let own = match profile.stride() {
                Some(stride) => stride,
                None => self.ctx.curve_samples,
            };
            stride = stride.max(own);
        }
        Ok(stride.max(2))
    }

    /// Resamples every profile to `stride + 1` evenly spaced points, as
    /// upstream does before blending them.
    fn resample_profiles(
        &self,
        profiles: &[Curve2DRef],
        stride: u32,
    ) -> Result<Vec<Vec<Point2>>> {
        profiles
            .iter()
            .map(|profile| {
                let points = profile.discretize(self.ctx.curve_samples)?;
                Ok(resample_polyline(&points, stride as usize + 1))
            })
            .collect()
    }
}

/// Upstream's closure tolerance in `process(Swung*)`.
const SWUNG_CLOSURE_EPSILON: Real = 0.01;

/// Whether an affine transform preserves lengths and angles, and therefore
/// leaves vertex normals valid.
fn is_rigid(transform: &Transform) -> bool {
    match transform {
        Transform::Translated(_)
        | Transform::AxisRotated { .. }
        | Transform::EulerRotated { .. }
        | Transform::Oriented { .. } => true,
        Transform::Scaled(s) => {
            crate::math::approx_eq(s.x, s.y) && crate::math::approx_eq(s.y, s.z)
        }
        Transform::Matrix(_) | Transform::Tapered { .. } => false,
    }
}

/// One profile repeated at every slice of a full turn — what `Revolution`
/// sweeps, and what a single-profile `Swung` collapses to.
fn full_turn(profile: &[Point2], slices: usize) -> (Vec<Vec<Point2>>, Vec<Real>) {
    let angle_step = std::f32::consts::TAU / slices as Real;
    (
        vec![profile.to_vec(); slices],
        (0..slices).map(|i| i as Real * angle_step).collect(),
    )
}

/// Whether a swept profile starts and ends on the axis, and so closes the
/// solid it sweeps. Upstream reads this off `Revolution::isAVolume()`.
fn profile_closes_a_volume(profile: &[Point2]) -> bool {
    match (profile.first(), profile.last()) {
        (Some(first), Some(last)) => {
            first.x.abs() <= crate::math::EPSILON && last.x.abs() <= crate::math::EPSILON
        }
        _ => false,
    }
}

/// A swept cross-section with its seam point restored, so the arc length
/// around a closed ring includes the segment that closes it.
fn section_with_seam(section: &[Point2], closed: bool) -> Vec<Point2> {
    let mut points = section.to_vec();
    if closed {
        points.push(section[0]);
    }
    points
}

/// Cumulative distance along a frame chain, normalised to `[0, 1]` — the v of
/// a swept UV, so a texture runs at a constant rate along the axis rather than
/// along its parameter.
fn cumulative_fractions(frames: &[crate::math::Frame]) -> Vec<Real> {
    let mut lengths = Vec::with_capacity(frames.len());
    let mut total = 0.0;
    lengths.push(0.0);
    for pair in frames.windows(2) {
        total += (pair[1].position - pair[0].position).norm();
        lengths.push(total);
    }
    if total <= crate::math::EPSILON {
        return vec![0.0; frames.len()];
    }
    lengths.iter().map(|l| l / total).collect()
}

/// `Discretizer::gridTexCoord` — per-row and per-column normalised arc length
/// over a sampled lattice, which is what keeps a texture even across a patch
/// whose samples are not evenly spaced in space.
fn grid_tex_coords(points: &[Point3], u_samples: usize, v_samples: usize) -> Vec<Vec2> {
    let mut tex = vec![Vec2::zeros(); points.len()];
    let at = |i: usize, j: usize| points[i * v_samples + j];

    for i in 0..u_samples {
        let total: Real = (1..v_samples).map(|j| (at(i, j) - at(i, j - 1)).norm()).sum();
        let mut walked = 0.0;
        for j in 1..v_samples {
            walked += (at(i, j) - at(i, j - 1)).norm();
            tex[i * v_samples + j].x = if total > 0.0 { walked / total } else { 0.0 };
        }
    }
    for j in 0..v_samples {
        let total: Real = (1..u_samples).map(|i| (at(i, j) - at(i - 1, j)).norm()).sum();
        let mut walked = 0.0;
        for i in 1..u_samples {
            walked += (at(i, j) - at(i - 1, j)).norm();
            tex[i * v_samples + j].y = if total > 0.0 { walked / total } else { 0.0 };
        }
    }
    tex
}

/// Cumulative arc length normalised to `[0, 1]`, the v of a cylindrical UV.
fn arc_length_parameters(section: &[Point2]) -> Vec<Real> {
    let mut lengths = Vec::with_capacity(section.len());
    let mut total = 0.0;
    lengths.push(0.0);
    for pair in section.windows(2) {
        total += (pair[1] - pair[0]).norm();
        lengths.push(total);
    }
    if total <= crate::math::EPSILON {
        return vec![0.0; section.len()];
    }
    lengths.iter().map(|l| l / total).collect()
}

/// Resamples a polyline to `count` points evenly spaced in arc length.
fn resample_polyline(points: &[Point2], count: usize) -> Vec<Point2> {
    if points.len() < 2 || count < 2 {
        return points.to_vec();
    }
    let ts = arc_length_parameters(points);
    (0..count)
        .map(|i| {
            let t = i as Real / (count - 1) as Real;
            // The last sample lands exactly on the last point rather than
            // wherever the search happens to stop.
            if i + 1 == count {
                return points[points.len() - 1];
            }
            let segment = ts
                .windows(2)
                .position(|w| t >= w[0] && t <= w[1])
                .unwrap_or(points.len() - 2);
            let (t0, t1) = (ts[segment], ts[segment + 1]);
            let local = if (t1 - t0).abs() <= crate::math::EPSILON {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            points[segment] + (points[segment + 1] - points[segment]) * local
        })
        .collect()
}

/// Degree-1 `ProfileInterpolation`: blends the two profiles bracketing
/// `angle`, per sample position along the profile.
fn blend_sections(
    sections: &[Vec<Point2>],
    angles: &[Real],
    angle: Real,
    stride: u32,
) -> Vec<Point2> {
    let count = stride as usize + 1;
    if angle <= angles[0] {
        return sections[0].clone();
    }
    if angle >= angles[angles.len() - 1] {
        return sections[sections.len() - 1].clone();
    }
    let upper = angles.iter().position(|a| *a >= angle).unwrap_or(1).max(1);
    let lower = upper - 1;
    let span = angles[upper] - angles[lower];
    let t = if span.abs() <= crate::math::EPSILON {
        0.0
    } else {
        (angle - angles[lower]) / span
    };
    (0..count)
        .map(|i| sections[lower][i] + (sections[upper][i] - sections[lower][i]) * t)
        .collect()
}

/// The spherical UV grid upstream builds in `process(Sphere*)`.
///
/// The `slices + 1`-wide grid duplicates the first meridian at `u = 1` so the
/// seam does not wrap a whole texture backwards in one triangle. Upstream
/// divides the ring's `v` by `stacks + 1` rather than `stacks`, which leaves a
/// band of texture unreachable at each pole; that is reproduced rather than
/// corrected, so the port's UVs match a scene authored against upstream.
fn sphere_tex_coords(slices: usize, stacks: usize) -> (Vec<Vec2>, Vec<Index3>) {
    let rings = stacks - 1;
    let columns = slices + 1;

    let mut tex_coords = Vec::with_capacity(columns * (stacks + 1));
    for i in 0..columns {
        let s = i as Real / slices as Real;
        for j in 1..stacks {
            tex_coords.push(Vec2::new(s, j as Real / (stacks + 1) as Real));
        }
    }
    let bottom = tex_coords.len() as u32;
    for i in 0..columns {
        tex_coords.push(Vec2::new(i as Real / slices as Real, 0.0));
    }
    let top = tex_coords.len() as u32;
    for i in 0..columns {
        tex_coords.push(Vec2::new(i as Real / slices as Real, 1.0));
    }

    let mut indices = Vec::with_capacity(slices * 2 * rings);
    for i in 0..slices {
        // Unlike the positions, the texture columns do not wrap: slice
        // `slices - 1` stitches to the duplicated seam column.
        let cur = (i * rings) as u32;
        let next = ((i + 1) * rings) as u32;
        for j in 0..rings as u32 {
            if j == 0 {
                indices.push([cur, bottom + i as u32, next]);
                indices.push([
                    cur + rings as u32 - 1,
                    next + rings as u32 - 1,
                    top + i as u32,
                ]);
            } else {
                indices.push([cur + j, cur + j - 1, next + j - 1]);
                indices.push([cur + j, next + j - 1, next + j]);
            }
        }
    }
    (tex_coords, indices)
}

impl Explicit {
    /// Drops the normals, so the next reader recomputes them.
    fn without_normals(mut self) -> Self {
        match &mut self {
            Explicit::TriangleSet(m) => {
                m.model.normals = None;
                m.normal_indices = None;
            }
            Explicit::QuadSet(m) => {
                m.model.normals = None;
                m.normal_indices = None;
            }
            Explicit::FaceSet(m) => {
                m.model.normals = None;
                m.normal_indices = None;
            }
            Explicit::PointSet(_) | Explicit::Polyline(_) => {}
        }
        self
    }
}

/// A [`GeometryVisitor`] over whole subtrees.
///
/// `walk` is overridden to call `visit` once on the root rather than descending
/// to leaves: a `Group` discretises to **one** merged model and a `Transformed`
/// must place its child, so flattening the tree first would discard exactly the
/// information discretisation needs. Upstream's `process(Group*)` merges for
/// the same reason.
impl GeometryVisitor for Discretizer {
    type Output = Explicit;

    fn visit(&mut self, geometry: &Geometry) -> Result<Explicit> {
        self.discretize(geometry)
    }

    fn walk(&mut self, geometry: &Geometry) -> Result<Vec<Explicit>> {
        Ok(vec![self.visit(geometry)?])
    }
}

/// Discretises a geometry tree at upstream's default density.
pub fn discretize(geometry: &Geometry) -> Result<Explicit> {
    Discretizer::with_defaults().discretize(geometry)
}

/// Discretises a geometry tree at the given density.
pub fn discretize_with(geometry: &Geometry, ctx: DiscretizeCtx) -> Result<Explicit> {
    Discretizer::new(ctx).discretize(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algo::measure::surface_area;
    use crate::math::Vec3;
    use crate::scenegraph::curve::{Curve2D, Polyline2D};
    use crate::scenegraph::primitive::HeightField;
    use approx::assert_relative_eq;

    fn ctx(slices: u8, stacks: u8) -> DiscretizeCtx {
        DiscretizeCtx {
            slices,
            stacks,
            ..Default::default()
        }
    }

    #[test]
    fn the_default_context_reproduces_upstreams_constants() {
        let ctx = DiscretizeCtx::default();
        assert_eq!(ctx.slices, 8);
        assert_eq!(ctx.stacks, 8);
        assert_eq!(ctx.curve_samples, 30);
    }

    #[test]
    fn scaling_the_context_never_drops_below_one() {
        assert_eq!(DiscretizeCtx::scaled(0.0).slices, 1);
        assert_eq!(DiscretizeCtx::scaled(4.0).slices, 32);
        assert_eq!(DiscretizeCtx::scaled(1.0), DiscretizeCtx::default());
    }

    #[test]
    fn the_default_box_discretises_to_the_unit_cube() {
        let mesh = discretize(&Geometry::from(Box3::default())).unwrap();
        assert_eq!(mesh.points().len(), 8);
        assert_eq!(mesh.face_count(), 6);
        assert!(mesh.is_solid());
        for p in mesh.points() {
            assert_relative_eq!(p.coords.abs(), Vec3::repeat(0.5), epsilon = 1e-6);
        }
        assert_relative_eq!(surface_area(&mesh).unwrap(), 6.0, epsilon = 1e-5);
    }

    #[test]
    fn a_solid_cone_gets_a_base_cap() {
        let open = discretize_with(
            &Geometry::from(Cone::default().with_solid(false)),
            ctx(8, 8),
        )
        .unwrap();
        let solid = discretize_with(&Geometry::from(Cone::default()), ctx(8, 8)).unwrap();
        assert_eq!(open.face_count(), 8);
        assert_eq!(solid.face_count(), 16);
        assert_eq!(open.points().len(), 9);
        assert_eq!(solid.points().len(), 10);
        assert!(!open.is_solid());
        assert!(solid.is_solid());
    }

    #[test]
    fn an_open_cylinder_is_quads_and_a_solid_one_is_a_face_set() {
        let open = discretize_with(
            &Geometry::from(Cylinder::default().with_solid(false)),
            ctx(8, 8),
        )
        .unwrap();
        assert!(matches!(open, Explicit::QuadSet(_)));
        assert_eq!(open.face_count(), 8);

        let solid = discretize_with(&Geometry::from(Cylinder::default()), ctx(8, 8)).unwrap();
        assert!(matches!(solid, Explicit::FaceSet(_)));
        // One side quad and two cap triangles per slice.
        assert_eq!(solid.face_count(), 24);
        assert_eq!(solid.points().len(), 18);
    }

    #[test]
    fn a_frustums_top_ring_is_scaled_by_its_taper() {
        let mesh = discretize_with(
            &Geometry::from(Frustum::new(2.0, 4.0, 0.25, false, 8)),
            ctx(8, 8),
        )
        .unwrap();
        // Points interleave base, top, base, top …
        assert_relative_eq!(mesh.points()[0].coords.xy().norm(), 2.0, epsilon = 1e-5);
        assert_relative_eq!(mesh.points()[1].coords.xy().norm(), 0.5, epsilon = 1e-5);
        assert_relative_eq!(mesh.points()[1].z, 4.0, epsilon = 1e-6);
    }

    #[test]
    fn a_frustum_of_taper_one_is_a_cylinder() {
        let frustum = discretize_with(
            &Geometry::from(Frustum::new(1.0, 3.0, 1.0, true, 16)),
            ctx(8, 8),
        )
        .unwrap();
        let cylinder = discretize_with(
            &Geometry::from(Cylinder::new(1.0, 3.0, true, 16)),
            ctx(8, 8),
        )
        .unwrap();
        assert_eq!(frustum.points(), cylinder.points());
        assert_eq!(frustum.face_count(), cylinder.face_count());
    }

    #[test]
    fn sphere_points_all_sit_on_the_radius() {
        let mesh = discretize_with(&Geometry::from(Sphere::sized(2.0)), ctx(12, 10)).unwrap();
        assert_eq!(mesh.points().len(), 12 * 9 + 2);
        assert_eq!(mesh.face_count(), 12 * 2 * 9);
        for p in mesh.points() {
            assert_relative_eq!(p.coords.norm(), 2.0, epsilon = 1e-5);
        }
    }

    #[test]
    fn a_disc_is_flat_and_faces_positive_z() {
        let mesh = discretize_with(&Geometry::from(Disc::sized(1.5)), ctx(8, 8)).unwrap();
        assert_eq!(mesh.face_count(), 8);
        assert!(!mesh.is_solid());
        for p in mesh.points() {
            assert_relative_eq!(p.z, 0.0, epsilon = 1e-6);
        }
        let Explicit::TriangleSet(triangles) = &mesh else {
            panic!("a disc discretises to triangles");
        };
        assert_relative_eq!(triangles.face_normal(0).unwrap(), Vec3::z(), epsilon = 1e-5);
    }

    #[test]
    fn a_paraboloid_runs_from_its_rim_to_its_apex() {
        let mesh =
            discretize_with(&Geometry::from(Paraboloid::sized(1.0, 4.0, 2.0)), ctx(8, 6)).unwrap();
        let top = mesh.points()[mesh.points().len() - 1];
        assert_relative_eq!(top, Point3::new(0.0, 0.0, 4.0), epsilon = 1e-6);
        assert_relative_eq!(mesh.points()[0].coords.xy().norm(), 1.0, epsilon = 1e-5);
        assert_relative_eq!(mesh.points()[0].z, 0.0, epsilon = 1e-6);
    }

    #[test]
    fn an_open_paraboloid_is_not_reported_solid() {
        // Upstream hardcodes solid = true here; the port passes the field.
        let open = discretize_with(
            &Geometry::from(Paraboloid::sized(1.0, 1.0, 2.0).with_solid(false)),
            ctx(8, 6),
        )
        .unwrap();
        assert!(!open.is_solid());
    }

    #[test]
    fn a_straight_profile_revolves_into_a_cylinder() {
        let profile = Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 2.0),
        ]))
        .into_ref();
        let mesh = discretize_with(
            &Geometry::from(Revolution::from_profile(profile)),
            ctx(32, 8),
        )
        .unwrap();
        for p in mesh.points() {
            assert_relative_eq!(p.coords.xy().norm(), 1.0, epsilon = 1e-5);
        }
        // 2πrh, within the polygonal approximation's shortfall.
        assert_relative_eq!(
            surface_area(&mesh).unwrap(),
            std::f32::consts::TAU * 2.0,
            max_relative = 0.01
        );
    }

    /// The divergence from upstream documented on [`Discretizer::sweep_profiles`]:
    /// a profile touching the axis gets one shared apex, not one duplicate per
    /// slice with a zero-area triangle between them.
    #[test]
    fn a_profile_touching_the_axis_collapses_to_one_apex() {
        let profile = Curve2D::from(Polyline2D::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(0.0, 2.0),
        ]))
        .into_ref();
        let mesh =
            discretize_with(&Geometry::from(Revolution::from_profile(profile)), ctx(8, 8)).unwrap();

        // One ring of 8, plus exactly two apexes — not 8 copies of each.
        assert_eq!(mesh.points().len(), 8 + 2);
        // One triangle per slice per pole, rather than upstream's two (of
        // which one is degenerate).
        assert_eq!(mesh.face_count(), 16);

        let Explicit::TriangleSet(triangles) = &mesh else {
            panic!("a revolution discretises to triangles");
        };
        for i in 0..triangles.face_count() {
            let area = crate::algo::measure::triangle_area(
                triangles.face_point_at(i, 0).unwrap(),
                triangles.face_point_at(i, 1).unwrap(),
                triangles.face_point_at(i, 2).unwrap(),
            );
            assert!(area > 1e-6, "triangle {i} is degenerate");
        }
    }

    #[test]
    fn a_profile_lying_entirely_on_the_axis_is_degenerate() {
        let line = Curve2D::from(Polyline2D::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 1.0),
        ]))
        .into_ref();
        assert!(matches!(
            discretize_with(&Geometry::from(Revolution::from_profile(line)), ctx(8, 8)),
            Err(Error::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn a_revolution_whose_profile_meets_the_axis_is_solid() {
        let closed = Curve2D::from(Polyline2D::new(vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(0.0, 2.0),
        ]))
        .into_ref();
        let mesh = discretize_with(
            &Geometry::from(Revolution::from_profile(closed)),
            ctx(16, 8),
        )
        .unwrap();
        assert!(mesh.is_solid());
    }

    #[test]
    fn a_single_profile_swung_matches_the_same_revolution() {
        // Three points, so the profile's own stride already meets the floor
        // of 2 that `ProfileInterpolation::interpol` imposes and neither path
        // resamples. See `a_swung_profile_is_resampled_to_at_least_two_segments`.
        let profile = Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(1.0, 2.0),
        ]))
        .into_ref();
        let swung = discretize_with(
            &Geometry::from(Swung::linear(vec![profile.clone()], vec![0.0])),
            ctx(16, 8),
        )
        .unwrap();
        let revolution = discretize_with(
            &Geometry::from(Revolution::from_profile(profile)),
            ctx(16, 8),
        )
        .unwrap();
        assert_eq!(swung.face_count(), revolution.face_count());
        assert_relative_eq!(
            surface_area(&swung).unwrap(),
            surface_area(&revolution).unwrap(),
            epsilon = 1e-4
        );
    }

    /// `ProfileInterpolation::interpol` floors the stride at 2, so a two-point
    /// profile is resampled to three before it is swept — a `Swung` is
    /// therefore finer than the `Revolution` of the same profile, not equal
    /// to it.
    #[test]
    fn a_swung_profile_is_resampled_to_at_least_two_segments() {
        let profile = Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 2.0),
        ]))
        .into_ref();
        let swung = discretize_with(
            &Geometry::from(Swung::linear(vec![profile.clone()], vec![0.0])),
            ctx(16, 8),
        )
        .unwrap();
        let revolution = discretize_with(
            &Geometry::from(Revolution::from_profile(profile)),
            ctx(16, 8),
        )
        .unwrap();
        assert_eq!(swung.points().len(), 16 * 3);
        assert_eq!(revolution.points().len(), 16 * 2);
        assert_eq!(swung.face_count(), 2 * revolution.face_count());
        // Denser sampling of the same surface, so the same area.
        assert_relative_eq!(
            surface_area(&swung).unwrap(),
            surface_area(&revolution).unwrap(),
            max_relative = 1e-4
        );
    }

    #[test]
    fn swung_blends_between_its_profiles() {
        let narrow = Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0),
        ]))
        .into_ref();
        let wide = Curve2D::from(Polyline2D::new(vec![
            Point2::new(3.0, 0.0),
            Point2::new(3.0, 1.0),
        ]))
        .into_ref();
        let swung = Swung::linear(
            vec![narrow, wide],
            vec![0.0, std::f32::consts::TAU],
        );
        let mesh = discretize_with(&Geometry::from(swung), ctx(4, 8)).unwrap();

        // Slice 0 sits on the narrow profile, and the radius grows with the
        // sweep angle.
        let radius_of = |slice: usize, section_size: usize| {
            mesh.points()[slice * section_size].coords.xy().norm()
        };
        let section_size = mesh.points().len() / 4;
        assert_relative_eq!(radius_of(0, section_size), 1.0, epsilon = 1e-4);
        assert!(radius_of(1, section_size) > radius_of(0, section_size));
        assert!(radius_of(3, section_size) > radius_of(2, section_size));
    }

    #[test]
    fn a_higher_degree_swung_names_the_phase_that_will_carry_it() {
        let profile = Curve2D::from(Polyline2D::new(vec![
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0),
        ]))
        .into_ref();
        let swung = Swung::new(
            vec![profile.clone(), profile],
            vec![0.0, 1.0],
            8,
            true,
            Swung::DEFAULT_DEGREE,
            0,
        );
        let error = discretize(&Geometry::from(swung)).unwrap_err();
        assert!(matches!(error, Error::Unsupported(_)));
        assert!(format!("{error}").contains("ProfileInterpolation"));
    }

    #[test]
    fn an_elevation_grid_lays_two_triangles_per_cell() {
        let heights = HeightField::from_rows(vec![
            vec![0.0, 1.0, 0.0],
            vec![1.0, 2.0, 1.0],
            vec![0.0, 1.0, 0.0],
        ])
        .unwrap();
        let mesh = discretize(&Geometry::from(ElevationGrid::from_heights(heights))).unwrap();
        assert_eq!(mesh.points().len(), 9);
        assert_eq!(mesh.face_count(), 8);
        assert!(!mesh.is_solid());
        assert_relative_eq!(mesh.points()[4], Point3::new(1.0, 1.0, 2.0), epsilon = 1e-6);
    }

    #[test]
    fn a_group_discretises_to_one_merged_mesh() {
        let group = Group::new(vec![
            Geometry::from(Box3::cube(1.0)).into_ref(),
            Geometry::from(Box3::cube(2.0)).into_ref(),
        ]);
        let mesh = discretize(&Geometry::from(group)).unwrap();
        assert_eq!(mesh.points().len(), 16);
        assert_eq!(mesh.face_count(), 12);
    }

    #[test]
    fn a_transform_is_baked_into_the_sampled_points() {
        let tree = Geometry::from(Transformed::new(
            Transform::Translated(Vec3::new(10.0, 0.0, 0.0)),
            Geometry::from(Box3::cube(1.0)).into_ref(),
        ));
        let mesh = discretize(&tree).unwrap();
        for p in mesh.points() {
            assert!((9.4..=10.6).contains(&p.x), "{p:?}");
        }
    }

    #[test]
    fn a_taper_is_applied_after_sampling() {
        // A cylinder tapered to nothing is a cone, which a matrix cannot do.
        let tree = Geometry::from(Transformed::new(
            Transform::Tapered {
                base_radius: 1.0,
                top_radius: 0.0,
            },
            Geometry::from(Cylinder::new(1.0, 2.0, false, 16)).into_ref(),
        ));
        let mesh = discretize(&tree).unwrap();
        for p in mesh.points() {
            if p.z > 1.0 {
                assert_relative_eq!(p.coords.xy().norm(), 0.0, epsilon = 1e-5);
            } else {
                assert_relative_eq!(p.coords.xy().norm(), 1.0, epsilon = 1e-5);
            }
        }
    }

    #[test]
    fn an_explicit_mesh_passes_through_unchanged() {
        let mesh = TriangleSet::new(
            vec![
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            vec![[0, 1, 2]],
        );
        let out = discretize(&Geometry::from(mesh.clone())).unwrap();
        assert_eq!(out, Explicit::TriangleSet(mesh));
    }

    #[test]
    fn a_degenerate_extrusion_is_rejected_rather_than_meshed() {
        let axis = Curve3D::from(Polyline::new(vec![Point3::origin(), Point3::origin()]));
        let extrusion = Geometry::from(Extrusion::new(
            axis.into_ref(),
            crate::scenegraph::curve::Curve2D::from(
                crate::scenegraph::curve::Polyline2D::circle(1.0, 6),
            )
            .into_ref(),
        ));
        assert!(matches!(
            discretize(&extrusion),
            Err(Error::DegenerateGeometry(_))
        ));
    }

    #[test]
    fn density_comes_from_the_primitive_when_it_has_one() {
        // The context says 4 slices, the primitive says 20; the primitive wins.
        let pinned = discretize_with(&Geometry::from(Disc::new(1.0, 20)), ctx(4, 4)).unwrap();
        assert_eq!(pinned.face_count(), 20);

        let deferred = discretize_with(&Geometry::from(Disc::sized(1.0)), ctx(4, 4)).unwrap();
        assert_eq!(deferred.face_count(), 4);
    }

    #[test]
    fn density_is_clamped_to_what_the_primitive_can_be_built_from() {
        let mesh = discretize_with(&Geometry::from(Disc::sized(1.0)), ctx(1, 1)).unwrap();
        assert_eq!(mesh.face_count(), MIN_SLICES as usize);
    }

    #[test]
    fn tex_coords_can_be_turned_off() {
        let mesh = Discretizer::with_defaults()
            .with_tex_coords(false)
            .discretize(&Geometry::from(Box3::default()))
            .unwrap();
        assert!(!mesh.model().has_tex_coords());
    }

    #[test]
    fn resampling_keeps_the_endpoints() {
        let points = vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 1.0),
            Point2::new(0.0, 4.0),
        ];
        let resampled = resample_polyline(&points, 5);
        assert_eq!(resampled.len(), 5);
        assert_relative_eq!(resampled[0], points[0], epsilon = 1e-6);
        assert_relative_eq!(resampled[4], points[2], epsilon = 1e-6);
        // Evenly spaced in arc length, so the midpoint is at y = 2.
        assert_relative_eq!(resampled[2], Point2::new(0.0, 2.0), epsilon = 1e-5);
    }

    #[test]
    fn the_visitor_impl_does_not_flatten_transforms_away() {
        let tree = Geometry::from(Transformed::new(
            Transform::Translated(Vec3::new(10.0, 0.0, 0.0)),
            Geometry::from(Box3::cube(1.0)).into_ref(),
        ));
        let walked = Discretizer::with_defaults().walk(&tree).unwrap();
        assert_eq!(walked.len(), 1);
        assert!(walked[0].points().iter().all(|p| p.x > 9.0));
    }
}
