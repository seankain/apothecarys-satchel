# 08 — PlantGL Rust Port

## Purpose

Port the core geometry and turtle-modelling functionality of
[openalea/plantgl](https://github.com/openalea/plantgl) into a new Rust crate,
`crates/plantgl`, and rebuild the game's plant mesh generation on top of it.

PlantGL is a C++/Python geometric library for 3D plant modelling from CIRAD /
INRIA / INRA (Pradal, Boudon, Nouguier et al.). It supplies exactly the layer
`crates/botany` is currently faking by hand: a scene graph of parametric plant
geometry, a 3D turtle that speaks the standard cpfg/L-studio command set, and
the discretisation machinery that turns both into triangle meshes.

**This is a direct translation, not a clean-room reimplementation.** We read
upstream's source and write the Rust equivalent. The cost of that freedom is
that `crates/plantgl` is licensed CeCILL-C — see
[§ Licensing](#licensing-cecill-c-for-the-port-mit-for-everything-else).

### What the game gets out of it

The present `crates/botany/src/mesh_gen.rs` builds stems as a chain of
disconnected, untapered 6-gon tubes and represents every leaf, flower and fruit
as a bare position + direction that the previewer renders as a flat triangle.
Adopting PlantGL's model buys us:

| Capability | Today | After the port |
|---|---|---|
| Stem geometry | Independent cylinders, visible seams at every joint | One `Extrusion` (generalized cylinder) per branch axis — continuous, mitred, tapered |
| Stem cross-section | Fixed circle, 6 sides | Arbitrary 2D profile curve (square stems for mints, ridged for grasses, per-species) |
| Radius along an axis | Constant, ×0.9 per segment | `QuantisedFunction` radius profile — true taper from base to tip |
| Leaves / petals | Position + direction, drawn as a triangle | Real `BezierPatch` / `TriangleSet` surfaces, oriented, scaled, with UVs |
| Curvature | Straight segments only | Curve-guided axes (`setGuide`) — drooping vines, arching fronds |
| Tropism | None | Gravitropism / phototropism with elasticity, per ABOP |
| Normals & UVs | Per-ring approximation, no leaf UVs | Computed uniformly by the tessellator for every primitive |
| Bounding volumes | None | `BBoxComputer` / `BSphereComputer` for culling, picking, plot fitting |
| Surface area / volume | None | `SurfComputer` / `VolComputer` — usable as a *gameplay* input (leaf area → harvest yield / potency) |
| LOD | None | Discretisation resolution is a parameter; one genotype → hub-quality and inventory-icon-quality meshes |
| Export | Hand-rolled OBJ | OBJ/PLY from a single tessellated `Scene` |

The surface-area point is worth flagging to design: PlantGL gives us a cheap,
principled number for "how much plant is there", which is a better harvest-yield
driver than a genotype scalar and ties the visible phenotype to the reward.

---

## Licensing: CeCILL-C for the port, MIT for everything else

| Component | License |
|---|---|
| `crates/plantgl` (the port) | **CeCILL-C** |
| Every other crate — `core`, `botany`, `game`, `garden`, `tools`, … | **MIT**, unchanged |
| Root `LICENSE` | **MIT**, unchanged |
| The shipped game binary | **MIT**, with a third-party notice |

### Why the port must be CeCILL-C

CeCILL-C's definition of an **Integrated Contribution** is:

> any or all modifications, corrections, **translations**, adaptations and/or new
> functions integrated into the Source Code by any or all Contributors.

A translation is named explicitly. Porting PlantGL's C++ to Rust therefore
produces *Modified Software* in the license's terms, and Article 5.3.2 governs
its distribution: ship a copy of the Agreement, ship the warranty/liability
notice, and — if ever distributing object code only — provide effective access
to the full source for the entire distribution period at no more than the cost
of transferring the data.

Publishing the crate's source on GitHub satisfies the access requirement
outright.

### Why the rest of the workspace stays MIT

Article **5.3.3** is the provision that makes this cheap:

> When the Licensee creates Derivative Software, this Derivative Software may be
> distributed under a license agreement other than this Agreement, subject to
> compliance with the requirement to include a notice concerning the rights over
> the Software as defined in Article 6.4.

`crates/botany`, `crates/game` and the rest combine our own code with the
CeCILL-C crate. That is Derivative Software, and it may ship under MIT provided
we carry the Article 6.4 notice and keep the port's source available for as long
as we distribute the game.

**There is no LGPL §4 analogue.** CeCILL-C contains no requirement to let users
relink against a modified version of the library, so Rust's static linking is
not a problem. This is the single most important fact about this decision: the
obligation is *source availability for one crate*, not relinking machinery and
not a license change for the game.

> Not legal advice. Worth a lawyer's eye before commercial distribution — the
> obligations attach to *shipping*, not to developing.

### Two corrections to earlier revisions of this document

1. A previous revision claimed that "anything statically linking [the port]
   inherits CeCILL-C obligations … which affects shipping the game binary."
   That overstated it. Article 5.3.3 permits the larger work under another
   license; only the notice and source-availability duties carry over.
2. A previous revision cited Article 5.3.4 as the "Integrated Modules"
   provision. 5.3.4 is *Compatibility with the CeCILL License*; the
   other-license permission is **5.3.3**. The conclusion was right, the citation
   was not.

### Concrete obligations — Phase A deliverables

All discharged in Phase A, and guarded by `crates/plantgl/tests/licensing.rs`
so a deletion or truncation fails the build rather than surfacing at launch.

- [x] `crates/plantgl/LICENSE` — the full CeCILL-C v1 text
- [x] `crates/plantgl/Cargo.toml` — `license = "CECILL-C"` (SPDX identifier)
- [x] **Per-file provenance header** naming the upstream file each module derives
      from (see the template below). Article 5.3.3 asks that Integrated
      Contributions be "clearly identified and documented"; per-file attribution
      discharges that and makes upstream diffs tractable when we rebase
- [x] Root `README.md` — a mixed-licensing note: the workspace is MIT except
      `crates/plantgl`, which is CeCILL-C
- [x] `THIRD-PARTY-LICENSES` (or `NOTICE`) shipped with release builds, carrying
      the CeCILL-C text, the CIRAD/INRIA/INRA copyright notices, the
      warranty/liability notice, and a pointer to the port's source
- [x] Cite Pradal et al. 2009 (Graphical Models 71:1–21), as upstream requests

The upstream commit the port was read at is
`4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189`; every provenance header names it,
so a future rebase is a diff rather than archaeology.

```rust
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/geometry/extrusion.{h,cpp}`.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
```

### What this decision buys, relative to the alternatives

Two routes were considered and rejected:

- **Clean-room reimplementation, MIT.** Keeps the workspace uniformly MIT and
  the crate publishable, but forbids consulting upstream's source — so NURBS,
  tessellation and rotation-minimising frames must be re-derived from the
  literature. Roughly a week and a half more work, materially more risk of
  subtle numerical error, and a standing process burden on every contributor.
- **FFI to upstream.** No geometry to write at all, but upstream's CMake marks
  Qt `REQUIRED` with no core-only toggle, so a plain `cargo build` stops working
  and every contributor needs a C++ toolchain plus Qt. It also puts memory and
  exception safety on us rather than the compiler, and PlantGL's non-atomic
  refcounting would likely force `!Send` handles, killing `rayon` parallelism.

Direct translation lands between them on effort and ahead of both on the things
that matter day to day: it stays pure Rust, `cargo build` just works, there is
no FFI UB surface, `rayon` stays available, and behaviour can be diffed against
the original C++ when something looks wrong — which is a luxury the clean-room
route specifically denies.

---

## Upstream map

PlantGL's C++ tree (`src/cpp/plantgl/`) as of master:

```
math/          vectors, matrices, quaternions, interpolation
scenegraph/
  core/        Action (visitor), SceneObject, DeepCopier, ref-counting
  geometry/    41 primitive types: box, sphere, cone, cylinder, disc, frustum,
               paraboloid, revolution, swung, sor, extrusion, extrudedhull,
               asymmetrichull, elevationgrid, amapsymbol, text,
               triangleset, quadset, faceset, pointset, polyline, group,
               beziercurve, nurbscurve, bezierpatch, nurbspatch, profile,
               curve, mesh, explicitmodel, parametricmodel, planarmodel,
               lineicmodel, primitive, hull, patch, interpol, plane,
               boundingbox, boundingsphere
  transformation/  translated, scaled, oriented, axisrotated, eulerrotated,
                   mattransformed, orthotransformed, tapered, deformed,
                   screenprojected, ifs, transformed
  appearance/  material, texture, colours, spectra
  container/   typed shared arrays (Point3Array, IndexArray, …)
  function/    QuantisedFunction — sampled 1D functions (radius profiles)
  scene/       Scene, Shape, Shape3D, SceneObject ids
algo/
  base/        discretizer, tesselator, bboxcomputer, bspherecomputer,
               surfcomputer, volcomputer, matrixcomputer, merge,
               skelcomputer, wirecomputer, polygoncomputer, statisticcomputer,
               intersection, planeclipping, curvemanipulation,
               pointmanipulation, dijkstra, randompoints, amaptranslator
  modelling/   turtle, turtleparam, turtlepath, pglturtle, pglturtledrawer,
               spacecolonization
  codec/       PLY, VRML, X3D, POV-Ray, Geomview, VGStar, LIG, DTA, binary, XML
  opengl/      GL renderer
  fitting/     primitive fitting to point clouds
  grid/        spatial grids
  raycasting/  ray-shape intersection
  projection/  z-buffer / projection rendering
gui/           Qt viewer
python/        Boost.Python bindings
```

Note what is **not** in PlantGL: the L-system engine. That is
[openalea/lpy](https://github.com/openalea/lpy), a separate project that drives
PlantGL's turtle. Our `crates/botany/src/lsystem.rs` therefore stays where it
is; the split we adopt matches upstream's own.

---

## Scope

### Port (core — Phases A–D)

| Upstream | Rust module | Why |
|---|---|---|
| `math/` | `plantgl::math` (thin layer over `nalgebra`) | Foundation |
| `scenegraph/core/action.h` | `plantgl::scenegraph::Geometry` enum + visitor traits | The dispatch backbone |
| `scenegraph/scene/` | `plantgl::scenegraph::{Scene, Shape}` | Output container |
| `scenegraph/geometry/` (subset) | `plantgl::scenegraph::{primitive, mesh, curve}` | The geometry we actually place |
| `scenegraph/transformation/` | `plantgl::scenegraph::transform` | Placement of organs |
| `scenegraph/appearance/` | `plantgl::scenegraph::appearance` | Colour/material carried to Fyrox |
| `scenegraph/function/` | `plantgl::scenegraph::function::QuantisedFunction` | Radius/taper profiles |
| `algo/base/discretizer` | `plantgl::algo::discretize` | Parametric → explicit |
| `algo/base/tesselator` | `plantgl::algo::tessellate` | Explicit → triangles |
| `algo/base/{bbox,bsphere,surf,vol,merge,matrix}computer` | `plantgl::algo::*` | Culling, gameplay metrics, batching |
| `algo/modelling/turtle*` | `plantgl::modelling::turtle` | **The centrepiece** |
| `algo/codec` (OBJ/PLY only) | `plantgl::codec` | Asset export, golden tests |

Within `scenegraph/geometry/`:

**Port:** `TriangleSet` `QuadSet` `FaceSet` `PointSet` `Polyline` `Polyline2D`
`Group` `Box` `Sphere` `Cone` `Cylinder` `Frustum` `Disc` `Paraboloid`
`Revolution` `Swung` `SOR` `Extrusion` `BezierCurve` `BezierCurve2D`
`NurbsCurve` `NurbsCurve2D` `BezierPatch` `NurbsPatch` `ElevationGrid`
`BoundingBox` `BoundingSphere`.

**Defer:** `AsymmetricHull` `ExtrudedHull` `AmapSymbol` `Text` `Plane`
`ScreenProjected` `IFS`. Hulls are AMAP-style crown envelopes — attractive later
for distant-tree LODs, unnecessary for L-system plants.

### Adapt rather than transliterate

A faithful translation is not a literal one. Five places where idiomatic Rust
should deliberately diverge:

- **Reference counting.** `RCPtr<T>` / `RefCountObject` → `Arc<T>`. Rust's
  ownership makes the intrusive refcounting unnecessary, and `Arc` is atomic, so
  `rayon` stays available.
- **Visitor `Action`.** Upstream uses double dispatch through a virtual
  `process(Sphere*)`-per-type interface because C++ lacks sum types. Our
  primitive set is closed, so `enum Geometry` + `match` is faster, exhaustive at
  compile time, and far less code. A `GeometryVisitor` trait remains for
  open-ended traversal.
- **`real_t`.** Upstream declares `typedef float real_t;` unless
  `PGL_USE_DOUBLE` is set, so `f32` is the faithful default — and it matches
  Fyrox's vertex format exactly. Do knot-span arithmetic in `f64` internally and
  downcast on output.
- **Deep copy.** `DeepCopier` → `#[derive(Clone)]`, with `Arc::make_mut` for
  copy-on-write of shared meshes.
- **Error handling.** Upstream's warn-and-continue and static error handlers →
  `Result` + `thiserror`.

### Skip

`gui/` (Qt), `algo/opengl/` (Fyrox renders), `python/` (Boost.Python),
`algo/fitting`, `algo/projection`, `algo/raycasting`, `algo/grid`,
`scenegraph/container` (Rust `Vec`), the legacy codecs (VRML, X3D, POV-Ray, LIG,
DTA, VGStar, Geomview, AMAP), CGAL wrappers, `dijkstra`, `pointmanipulation`.

`algo/modelling/spacecolonization` and `algo/base/skelcomputer` are parked in
Phase F — space colonization is an attractive alternative generator for
shrub/canopy species, but not on the critical path.

---

## Target crate

- Path: `crates/plantgl`
- Package name: `plantgl` (free on crates.io)
- Edition 2021, **CeCILL-C**, no dependency on `apothecarys-core` or Fyrox — it
  stays a general-purpose library.

```toml
[package]
name = "plantgl"
version = "0.1.0"
edition = "2021"
license = "CECILL-C"
description = "Geometry and turtle modelling for procedural plants, ported from PlantGL"

[dependencies]
nalgebra = "0.33"            # matches fyrox 0.36's version — no duplicate in the graph
serde = { workspace = true, optional = true }
thiserror = "2"

[dev-dependencies]
approx = "0.5"
proptest = "1"

[features]
default = ["serde"]
serde = ["dep:serde", "nalgebra/serde-serialize"]
```

`nalgebra 0.33.2` is already in `Cargo.lock` via Fyrox 0.36, so conversions at
the Fyrox boundary are type-identical and build time is unaffected.

### Layout

```
crates/plantgl/
  LICENSE                    -- CeCILL-C v1
  src/
    lib.rs
    error.rs
    math/{mod,frame,spline}.rs
    scenegraph/
      geometry.rs            -- enum Geometry, GeometryRef = Arc<Geometry>, visitor traits
      primitive/             -- box3, sphere, cone, cylinder, frustum, disc,
                                paraboloid, revolution, swung, sor, extrusion,
                                elevation_grid
      mesh/                  -- ExplicitModel + triangle_set, quad_set, face_set,
                                point_set, polyline, group
      curve/                 -- Curve2D/Curve3D traits, polyline2d, bezier, nurbs, patch
      transform.rs           -- enum Transform + Transformed
      appearance.rs          -- Color3/4, Material, ImageTexture, Texture2D
      function.rs            -- QuantisedFunction
      scene.rs               -- Scene, Shape
    algo/
      discretize.rs tessellate.rs bbox.rs bsphere.rs measure.rs
      matrix.rs merge.rs normals.rs
    modelling/
      turtle.rs param.rs path.rs drawer.rs scene_drawer.rs mesh_drawer.rs tropism.rs
    codec/{mod,obj,ply}.rs
  tests/
    golden/ analytic.rs turtle.rs determinism.rs
```

---

## Core design

### Geometry as a sum type

```rust
pub type GeometryRef = Arc<Geometry>;

#[derive(Debug, Clone)]
pub enum Geometry {
    Box(Box3), Sphere(Sphere), Cone(Cone), Cylinder(Cylinder), Frustum(Frustum),
    Disc(Disc), Paraboloid(Paraboloid),
    Revolution(Revolution), Swung(Swung),
    Extrusion(Extrusion),          // generalized cylinder; the stem workhorse
    TriangleSet(TriangleSet), QuadSet(QuadSet), FaceSet(FaceSet),
    PointSet(PointSet), Polyline(Polyline),
    BezierCurve(BezierCurve), NurbsCurve(NurbsCurve),
    BezierPatch(BezierPatch), NurbsPatch(NurbsPatch),
    ElevationGrid(ElevationGrid),
    Group(Vec<GeometryRef>),
    Transformed(Box<Transformed>),
}

pub trait GeometryVisitor {
    type Output;
    fn visit(&mut self, g: &Geometry) -> Result<Self::Output>;
}
```

Everything in `algo/` is a visitor, mirroring upstream's Action hierarchy
one-for-one so the mental model transfers between the two codebases.

### Transformations — and the `Tapered` trap

```rust
pub struct Transformed { pub transform: Transform, pub child: GeometryRef }

pub enum Transform {
    Translated(Vector3<f32>),
    Scaled(Vector3<f32>),
    AxisRotated { axis: Unit<Vector3<f32>>, angle: f32 },
    EulerRotated { azimuth: f32, elevation: f32, roll: f32 },
    Oriented { primary: Vector3<f32>, secondary: Vector3<f32> },
    Matrix(Matrix4<f32>),
    Tapered { base_radius: f32, top_radius: f32 },
}
```

Upstream's `Tapered` derives from `Deformed`, **not** `OrthoTransformed`: it is a
non-affine radial scale along local Z and cannot be folded into an accumulated
4×4. So `Transform::to_matrix4` returns `Option<Matrix4<f32>>` (`None` for
`Tapered`), `MatrixComputer` carries a separate deformation stack, and
`discretize` applies the taper to vertex positions *after* sampling. Get this
wrong and you silently produce cylinders where frusta were intended.

### Extrusion — the generalized cylinder

The single most valuable primitive to port, and what makes stems stop looking
like stacked cans:

```rust
pub struct Extrusion {
    pub axis: Curve3DRef,                 // Polyline / Bezier / NURBS
    pub cross_section: Curve2DRef,        // closed 2D profile
    pub scale: Vec<Vector2<f32>>,         // per-knot cross-section scale
    pub orientation: Vec<f32>,            // per-knot twist
    pub knot_list: Option<Vec<f32>>,
    pub solid: bool,
    pub ccw: bool,
}
```

Discretisation samples the axis, builds a **rotation-minimising frame** by the
double-reflection method — the naive Frenet frame flips at inflection points and
twists the mesh — places a scaled/rotated cross-section at each sample, and
stitches consecutive rings. This is also what `Turtle::startGC()`/`stopGC()` and
`Turtle::sweep()` produce.

### The turtle

Ported command set, grouped as upstream groups it: `f/F/nF`;
`left/right/up/down/rollL/rollR/iRollL/iRollR/turnAround`;
`rollToVert/rollToHorizontal`; `setHead/eulerAngles/transform`; `push/pop`;
`move/shift/lineTo/lineRel/pinpoint/oLineTo/oLineRel`;
`setWidth/incWidth/decWidth`; `setColor/incColor/decColor/interpolateColors`;
`scale/multScale/divScale`; `sphere/circle/box/quad/label/surface`;
`startGC/stopGC`; `startPolygon/polygonPoint/stopPolygon`;
`setCrossSection/setSectionResolution`;
`setGuide/clearGuide/setPositionOnGuide/sweep`;
`setTropism/setElasticity/leftReflection/upReflection/headingReflection`;
`setDefaultStep/setAngleIncrement/setScaleMultiplier`; `setId/incId/decId`;
`start/stop/reset`; the texture-transform setters.

Skipped: `frame/arrow/vector` (debug viz), `setScreenCoordinatesEnabled`,
`registerPushPopHandler`, the static error handlers.

```rust
pub struct TurtleState {
    pub position: Point3<f32>,
    pub heading: Vector3<f32>,
    pub left: Vector3<f32>,
    pub up: Vector3<f32>,
    pub scale: Vector3<f32>,
    pub width: f32,
    pub color: usize,
    pub texture: TextureState,
    pub tropism: Vector3<f32>,
    pub elasticity: f32,
    pub cross_section: Option<Curve2DRef>,
    pub section_resolution: u32,
    pub guide: Option<GuideState>,
    pub id: Option<u32>,
}

pub struct Turtle<D: TurtleDrawer> {
    state: TurtleState,
    stack: Vec<TurtleState>,
    defaults: TurtleDefaults,
    gc: Option<GcBuilder>,
    polygon: Option<Vec<Point3<f32>>>,
    drawer: D,
}
```

`push`/`pop` become a plain `Vec<TurtleState>`, replacing upstream's
`std::stack<TurtleParam*>` and its manual lifetimes. `pop` on an empty stack
returns `Err(TurtleError::EmptyStack)` rather than warning and continuing; the
L-system driver may ignore it.

**Tropism** (ABOP §2.3 "tend to"): each `F` rotates the frame about
`heading × tropism` by `elasticity * |heading × tropism|`, then
re-orthonormalises. `tropism = -Y` is gravitropism; aiming at a light direction
is phototropism.

**Drawers** mirror upstream's `Turtle`/`TurtleDrawer` split, so one turtle
program can emit a scene graph, a merged mesh, or nothing but metrics:

```rust
pub trait TurtleDrawer {
    fn cylinder(&mut self, frame: &Frame, length: f32, radius: f32, ctx: &DrawCtx);
    fn frustum(&mut self, frame: &Frame, length: f32, base_r: f32, top_r: f32, ctx: &DrawCtx);
    fn generalized_cylinder(&mut self, axis: &[Point3<f32>], lefts: &[Vector3<f32>],
                            radii: &[f32], section: &Curve2D, ctx: &DrawCtx);
    fn sphere(&mut self, frame: &Frame, radius: f32, ctx: &DrawCtx);
    fn polygon(&mut self, points: &[Point3<f32>], ctx: &DrawCtx);
    fn surface(&mut self, name: &str, frame: &Frame, scale: f32, ctx: &DrawCtx);
    /* box, quad, disc, label … */
}
```

- `SceneDrawer` → a `Scene` of `Shape`s. Inspectable, exportable, LOD-able.
- `MeshDrawer` → one merged `TriangleSet` per material. The game path: skips the
  scene graph for plants that never need editing.
- `MeasureDrawer` → surface area, volume, bbox, segment count, zero allocation.
  Compute harvest yield without building geometry.

Being pure Rust, these are plain trait impls — no C++ vtable callbacks, which is
the reason the FFI route could not offer them.

### LOD

Upstream takes tessellation density as constructor arguments
(`Sphere(radius, slices, stacks)`). The port keeps those, and adds a
`DiscretizeCtx { slices, stacks, curve_samples }` default that primitives fall
back to when unspecified — a convenience upstream lacks, and the cleanest way to
render one genotype at three quality tiers.

---

## Integration with the existing code

### Division of labour

- `crates/plantgl` — geometry, turtle, tessellation. Engine- and game-agnostic.
  CeCILL-C.
- `crates/botany` — genetics, phenotype, L-system, stat mapping, plus a new
  `interpret.rs` driving the turtle. MIT. Precisely the L-Py ↔ PlantGL boundary
  upstream.
- `crates/game`, `crates/tools` — consume `plantgl::Scene` / `TriangleSet` via
  the Fyrox bridge. MIT.

### File-by-file impact

| File | Change |
|---|---|
| `crates/botany/src/turtle.rs` | **Deleted.** Its `Vec3`, `rotate_around_axis` and `TurtleInterpreter` are superseded by `plantgl::math` and `plantgl::modelling::turtle`. Tests move to `interpret.rs` and to `plantgl`'s turtle tests. |
| `crates/botany/src/interpret.rs` | **New.** `interpret(symbols, phenotype) -> plantgl::Scene`. |
| `crates/botany/src/mesh_gen.rs` | Gutted. `PlantMeshData` becomes a thin facade *only* so `tools`/`game` keep compiling during migration; `build_stem_mesh` and the hand-rolled OBJ/MTL writers go in favour of `plantgl::codec::obj`. Removed at T8.12. |
| `crates/botany/src/lsystem.rs` | Extended: `StartGC`, `StopGC`, `SetCrossSection`, `SetTropism`, `Surface`. Rewriting engine unchanged. |
| `crates/botany/src/phenotype.rs` | Extended: cross-section profile, taper curve, tropism elasticity, axis curvature, LOD tier. |
| `crates/botany/src/fyrox_bridge.rs` | **New.** `TriangleSet` → `SurfaceData`; appearance → `Material`. |
| `crates/tools/src/plant_preview.rs` | Carries a `plantgl::Scene`; reports real surface area and volume. |
| `crates/tools/src/bin/plant_previewer.rs` | Builds nodes through the bridge. |
| `crates/game/src/*` | Garden/hub spawn path only. |
| `Cargo.toml` (workspace) | Add `crates/plantgl`; add `nalgebra = "0.33"` to `workspace.dependencies`. |
| `README.md` | Crate table, plus the mixed-licensing note. |

`TriangleSet` is already SoA, so the Fyrox bridge is a per-vertex repack into
Fyrox's interleaved `StaticVertex` plus an index map — and with `real_t` as
`f32`, no numeric conversion. Shapes sharing an appearance are merged first by
`algo::merge`, so one plant is a handful of draw calls rather than one per leaf.

---

## Phase plan

Tracked as GitHub issues; this table is the index.

| Phase | Issue | Contents | Estimate | State |
|---|---|---|---|---|
| A — Foundation | [#17](https://github.com/seankain/apothecarys-satchel/issues/17) | T8.1–T8.3: crate + licensing, math/frames, scene graph, OBJ + golden harness | ~1 week | **done** |
| B — Primitives | [#18](https://github.com/seankain/apothecarys-satchel/issues/18) | T8.4–T8.5: parametric primitives, discretizer, tessellator, measurement | ~1.5 weeks | not started |
| C — Curves | [#19](https://github.com/seankain/apothecarys-satchel/issues/19) | T8.6–T8.7: Bézier/NURBS, patches, `Extrusion` | ~1.5 weeks | not started |
| D — Turtle | [#20](https://github.com/seankain/apothecarys-satchel/issues/20) | T8.8–T8.9: turtle core, GC, polygons, guides, tropism | ~1.5 weeks | not started |
| E — Integration | [#21](https://github.com/seankain/apothecarys-satchel/issues/21) | T8.10–T8.12: rewire botany, Fyrox bridge, doc reconciliation | ~1 week | not started |
| F — Optional | [#22](https://github.com/seankain/apothecarys-satchel/issues/22) | T8.13–T8.17: space colonization, PLY/glTF, hulls, instancing | as needed | not started |

**Core total: ~6.5 weeks**, ~6 500 lines of Rust excluding tests. Upstream's C++
in the ported scope is roughly 45 000 lines; the reduction is real, not optimism
— we drop the Qt viewer, the GL renderer, the Python bindings, the legacy
codecs, the container library and the manual refcounting, and sum types collapse
the double-dispatch boilerplate that dominates `algo/base`.

Full task breakdowns and acceptance criteria live in the issues.

### What Phase A actually landed

`crates/plantgl` builds, is wired into the workspace and depends on neither
`apothecarys-core` nor Fyrox:

- `math/` — nalgebra aliases, upstream's tolerances and angle constants, the
  ZYX Euler rotation and orthonormal basis upstream's transformations use, and
  `Frame` with Gram-Schmidt re-orthonormalisation plus rotation-minimising
  frame propagation by double reflection.
- `scenegraph/` — `ExplicitModel`, `IndexedMesh` (`TriangleSet`/`QuadSet`/
  `FaceSet`), `PointSet`, `Polyline`, `Group`, the `Geometry` sum type with a
  `GeometryVisitor` whose provided `walk` handles `Group`/`Transformed`
  recursion, `Transform`/`Transformed`/`Taper`, the appearance types, and
  `Scene`/`Shape`.
- `algo/` — `MatrixComputer` with its separate deformation stack, and
  `BBoxComputer`/`BoundingBox`.
- `codec/obj.rs` — OBJ and MTL writing with fixed float formatting, plus a
  reader, so export round-trips.
- `tests/` — golden snapshots with `UPDATE_GOLDEN=1`, proptest invariants, and
  the licensing guards.

Two deliberate departures from upstream behaviour, both documented at the
divergence:

1. `MatrixComputer::process(Tapered*)` upstream falls through to
   `default_process` and drops the taper. The port records it on a deformation
   stack instead, so "the transform at this leaf" is never silently incomplete.
2. `Mesh::computeNormalPerVertex`'s degenerate-normal guard compares against a
   NaN produced by normalising a zero-length cross product, and every
   comparison against NaN is false, so the NaN reaches the vertex buffer. The
   port checks the input instead.

The pre-port baseline asked for in T8.3 is captured in
`crates/botany/tests/golden/` by `crates/botany/tests/golden_pre_plantgl.rs` —
the *current* generator's own OBJ/MTL output for five fixed seeds, so Phase E's
visual change is a reviewable diff.

---

## Testing strategy

Four layers, because "the mesh looks plausible" is not a test:

1. **Analytic.** Every primitive's discretised surface area and volume is checked
   against the closed-form value, converging monotonically with slice count.
   Catches winding errors, missing caps and double-counted vertices in one
   assertion.
2. **Invariants (proptest).** Over random command sequences: the turtle frame
   stays orthonormal; balanced `push`/`pop` restores state exactly; `merge`
   preserves triangle count and bbox; tessellation preserves total area; `solid`
   meshes are edge-manifold and closed.
3. **Golden files.** Committed `.obj` snapshots with fixed float formatting.
   **Capture goldens from the current generator before Phase E** so the visual
   diff is reviewable rather than discovered.
4. **Determinism.** Same genotype + seed ⇒ byte-identical OBJ. Already matters
   for saves, which store genotypes rather than meshes.

**Differential testing against upstream is now available and should be used.**
Because we may read and run PlantGL, a dev-only harness can drive the same
parametric scene through conda-installed Python PlantGL and compare bbox,
surface area, volume and triangle counts against ours. Under the clean-room
route this was a nice-to-have of dubious legality; here it is the cheapest way
to validate a translation, and it should land alongside Phase B rather than
waiting for Phase F. Never a build dependency.

---

## Performance budget

Plants are generated at garden load and on harvest, not per frame. Targets on a
mid-range desktop, single thread:

| Operation | Budget |
|---|---|
| Genotype → derived L-system string (6 iterations) | < 1 ms |
| Turtle interpretation + GC construction | < 3 ms |
| Discretise + tessellate + merge, hub LOD | < 10 ms |
| **Total per plant, hub LOD** | **< 15 ms** |
| Triangles per plant, hub LOD | < 12 000 |
| Triangles per plant, distant LOD | < 1 500 |
| Triangles per plant, inventory icon | < 400 |

Levers if we miss: lower `DiscretizeCtx::slices` (the biggest knob), `MeshDrawer`
instead of `SceneDrawer`, organ instancing instead of merging, cached tessellated
organ templates — and, since `Arc` is atomic and there is no FFI boundary,
**`rayon` per plot is available**. A 12-plot garden at 15 ms each is ~180 ms
serially and trivially parallelised.

---

## Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **Licensing obligations missed at ship time** | High | Discharged as Phase A deliverables with a release-build check, not left to launch. The crate's source being public satisfies the access requirement; the notice file is the part that is easy to forget |
| **Accidental MIT contamination** — ported code landing in an MIT crate | High | `crates/plantgl` is the only CeCILL-C crate. Nothing translated from upstream may be pasted into `botany`, `core` or `game`. Per-file provenance headers make violations visible in review |
| **NURBS and sweeps (Phase C)** | Medium | Much lower than under clean-room: we translate working code and can diff against the original. The differential harness above is the safety net |
| **`Tapered` folded into an affine matrix** | Medium | Called out in the design above; covered by a dedicated test |
| **`f32` precision in knot arithmetic** | Low | `f64` internally in `spline.rs`, downcast at the boundary; explicit test on a 200-control-point curve |
| **Regression in existing plant visuals** | Medium | Golden OBJs captured from the current generator before T8.10 |
| **Triangle-count blowup** | Medium | LOD tiers are a T8.11 deliverable, not an afterthought; the budget is enforced by a test |
| **Big-bang migration** | Low | `PlantMeshData` stays a facade through Phase E; deleted at T8.12 |
| **Upstream divergence** | Low | Record the upstream commit each file was translated from, so a future rebase is a diff rather than an archaeology project |

---

## References

- Pradal C., Boudon F., Nouguier C., Chopard J., Godin C. (2009). *PlantGL: A Python-based geometric library for 3D plant modelling at different scales.* Graphical Models 71(1):1–21.
- Prusinkiewicz P., Lindenmayer A. (1990). *The Algorithmic Beauty of Plants.* Springer — the turtle command set, tropism, and generalized cylinders PlantGL implements.
- Boudon F. et al. *L-Py: an L-system simulation framework* — the upstream L-system engine that drives PlantGL's turtle.
- Wang W., Jüttler B., Zheng D., Liu Y. (2008). *Computation of rotation minimizing frames.* ACM TOG 27(1) — the double-reflection method used in `Extrusion`.
- Piegl L., Tiller W. (1997). *The NURBS Book*, 2nd ed. — de Boor, knot insertion, rational curves.
- [openalea/plantgl](https://github.com/openalea/plantgl) — the source we translate.
- [CeCILL-C v1 license text](http://www.cecill.info/licences/Licence_CeCILL-C_V1-en.html) and the [CeCILL FAQ](http://www.cecill.info/faq.en.html).
