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

## Read this first: licensing

**PlantGL is CeCILL-C. This repository is MIT. The two do not mix silently.**

Every PlantGL source file carries:

> This software is governed by the CeCILL-C license under French law … Copyright CIRAD/INRIA/INRA

CeCILL-C is a *file-level* copyleft in the spirit of the LGPL. Linking against
it is unencumbered, but a **line-by-line translation of a `.cpp` file into Rust
is a derivative work of that file**, and the result must be distributed under
CeCILL-C. A single translated file would make `crates/plantgl` a CeCILL-C crate
inside an otherwise-MIT workspace.

### Mandated approach: clean-room reimplementation

This port must be written **from the specification, not from the source**.
Permitted inputs:

- The published paper — Pradal C., Boudon F., Nouguier C., Chopard J., Godin C.
  (2009), *PlantGL: A Python-based geometric library for 3D plant modelling at
  different scales*, Graphical Models 71(1):1–21.
- PlantGL's public API documentation and Python docstrings (names, signatures,
  parameter semantics).
- The directory/class inventory in this document.
- *The Algorithmic Beauty of Plants* (Prusinkiewicz & Lindenmayer, 1990), which
  is the source of the turtle command set, tropism model, and generalized
  cylinder construction that PlantGL implements.
- Standard computer-graphics literature for NURBS, Bézier evaluation,
  tessellation, and surface-of-revolution construction.

Not permitted: copying, transliterating, or paraphrasing PlantGL `.h`/`.cpp`
bodies; copying doc comments verbatim; vendoring test fixtures.

**APIs, class names, and mathematical algorithms are not themselves
copyrightable** — mirroring PlantGL's structure and naming so the two libraries
are conceptually interchangeable is fine and is an explicit goal. What must be
independently written is the *expression*: the code.

Every file in the new crate gets this header:

```rust
//! Independent Rust implementation of the PlantGL geometry model.
//!
//! Modelled on the public API and published description of PlantGL
//! (Pradal et al., Graphical Models 71(1):1-21, 2009) and on the turtle
//! geometry of Prusinkiewicz & Lindenmayer, "The Algorithmic Beauty of
//! Plants". Written from specification; contains no code derived from the
//! CeCILL-C licensed PlantGL sources.
```

### Decision required before Phase A starts

Pick one and record it in this document:

1. **Clean-room, MIT** (recommended). Slower for NURBS and tessellation, keeps
   the workspace uniformly MIT, keeps the crate publishable.
2. **Translate, CeCILL-C the crate.** Faster, but `crates/plantgl` and anything
   statically linking it inherits CeCILL-C obligations — source availability for
   the crate and for modifications, which affects shipping the game binary.
3. **Depend on upstream via FFI.** No port at all: `bindgen` over the C++ API,
   conda-installed PlantGL at build time. Rejected — it makes the game
   un-buildable on a plain `cargo build`, drags in Qt/Boost.Python, and defeats
   the point of a Rust workspace.

Everything below assumes **option 1**.

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
               lineicmodel, primitive, hull, patch, sor, interpol, plane,
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
is; the split we adopt matches upstream's.

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

Within `scenegraph/geometry/`, the primitives we port and the ones we defer:

**Port:** `TriangleSet` `QuadSet` `FaceSet` `PointSet` `Polyline` `Polyline2D`
`Group` `Box` `Sphere` `Cone` `Cylinder` `Frustum` `Disc` `Paraboloid`
`Revolution` `Swung` `SOR` `Extrusion` `BezierCurve` `BezierCurve2D`
`NurbsCurve` `NurbsCurve2D` `BezierPatch` `NurbsPatch` `ElevationGrid`
`BoundingBox` `BoundingSphere`.

**Defer:** `AsymmetricHull` `ExtrudedHull` `AmapSymbol` `Text` `Plane`
`ScreenProjected` `IFS` — none are needed for L-system plants; hulls are for
AMAP-style crown envelopes, which we could want later for tree LODs.

### Adapt rather than port

- **Reference counting.** `RCPtr<T>` / `RefCountObject` → `Arc<T>`. Rust's
  ownership makes PlantGL's intrusive refcounting unnecessary.
- **Visitor `Action`.** PlantGL uses double dispatch through a virtual
  `process(Sphere*)`-per-type interface because C++ has no sum types. Our
  primitive set is closed, so `enum Geometry` + `match` is faster, exhaustive
  at compile time, and far less code. A `GeometryVisitor` trait is still
  provided for open-ended traversal (see below).
- **`real_t` = `double`.** We use `f32` throughout: Fyrox's vertex format is
  f32, plant-scale geometry needs nothing more, and it halves our vertex
  buffers. NURBS knot arithmetic is the one place this is dicey — do knot
  spans in `f64` internally and downcast on output.
- **Deep copy.** `DeepCopier` → `#[derive(Clone)]`, with `Arc::make_mut` where
  we want copy-on-write of shared meshes.

### Skip

`gui/` (Qt), `algo/opengl/` (Fyrox renders), `python/` (Boost.Python),
`algo/fitting`, `algo/projection`, `algo/raycasting`, `algo/grid`,
`scenegraph/container` (Rust `Vec`), legacy codecs (VRML, X3D, POV-Ray, LIG,
DTA, VGStar, Geomview, AMAP), CGAL wrappers, `dijkstra`, `pointmanipulation`.

`algo/modelling/spacecolonization` and `algo/base/skelcomputer` are deliberately
parked in Phase F — space colonization is an attractive alternative generator
for shrub/canopy species, but it is not on the critical path.

---

## Target crate

### Identity

- Path: `crates/plantgl`
- Package name: `plantgl` (verified free on crates.io as of this writing)
- Edition 2021, MIT, **no dependency on `apothecarys-core` or on Fyrox** — it
  must stay a general-purpose library that could be published standalone.

The Fyrox bridge lives behind a feature flag so the crate builds (and its tests
run) without pulling the engine in:

```toml
[package]
name = "plantgl"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Geometry and turtle modelling for procedural plants"

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

`nalgebra 0.33.2` is already in `Cargo.lock` via Fyrox 0.36. Pinning to it means
conversions at the Fyrox boundary are transmute-free (`Vector3<f32>` is the same
type on both sides) and adds nothing to build time.

### Layout

```
crates/plantgl/
  src/
    lib.rs
    error.rs                 -- thiserror Error enum
    math/
      mod.rs                 -- Vec2/Vec3/Mat4 aliases, angle helpers
      frame.rs               -- orthonormal (heading, up, left) frame + Gram-Schmidt
      spline.rs              -- de Casteljau, de Boor, knot vectors
    scenegraph/
      mod.rs
      geometry.rs            -- enum Geometry, GeometryRef = Arc<Geometry>, visitor traits
      primitive/
        mod.rs
        box3.rs sphere.rs cone.rs cylinder.rs frustum.rs disc.rs paraboloid.rs
        revolution.rs swung.rs sor.rs extrusion.rs elevation_grid.rs
      mesh/
        mod.rs               -- ExplicitModel: shared point/normal/uv/colour arrays
        triangle_set.rs quad_set.rs face_set.rs point_set.rs polyline.rs group.rs
      curve/
        mod.rs               -- Curve2D / Curve3D traits: eval, tangent, length, sample
        polyline2d.rs bezier.rs nurbs.rs patch.rs
      transform.rs           -- enum Transform + Transformed { transform, child }
      appearance.rs          -- Color3/Color4, Material, ImageTexture, Texture2D
      function.rs            -- QuantisedFunction
      scene.rs               -- Scene { shapes: Vec<Shape> }, Shape { geometry, appearance, id, parent_id }
    algo/
      mod.rs
      discretize.rs          -- Geometry -> ExplicitModel  (Discretizer)
      tessellate.rs          -- ExplicitModel -> TriangleSet  (Tesselator)
      bbox.rs bsphere.rs     -- BBoxComputer, BSphereComputer
      measure.rs             -- SurfComputer, VolComputer
      matrix.rs              -- MatrixComputer: accumulated world transform
      merge.rs               -- Merge: batch many TriangleSets into one buffer
      normals.rs             -- face/vertex normals, crease-angle smoothing
    modelling/
      mod.rs
      turtle.rs              -- Turtle<D: TurtleDrawer>: state, stack, commands
      param.rs               -- TurtleParam (the pushed/popped state record)
      path.rs                -- TurtlePath: guides, positions along a curve
      drawer.rs              -- trait TurtleDrawer
      scene_drawer.rs        -- emits a Scene (mirrors PglTurtleDrawer)
      mesh_drawer.rs         -- emits a merged TriangleSet directly (game fast path)
      tropism.rs             -- tend-to / reflection
    codec/
      mod.rs  obj.rs  ply.rs
  tests/
    golden/                  -- .obj snapshots
    analytic.rs              -- primitives vs closed-form area/volume
    turtle.rs                -- command-sequence behaviour
    determinism.rs
```

---

## Core design

### Geometry as a sum type

```rust
pub type GeometryRef = Arc<Geometry>;

#[derive(Debug, Clone)]
pub enum Geometry {
    // Parametric primitives
    Box(Box3),
    Sphere(Sphere),
    Cone(Cone),
    Cylinder(Cylinder),
    Frustum(Frustum),
    Disc(Disc),
    Paraboloid(Paraboloid),

    // Sweeps and surfaces of revolution
    Revolution(Revolution),
    Swung(Swung),
    Extrusion(Extrusion),      // <- generalized cylinder; the stem workhorse

    // Explicit meshes (share vertex arrays via Arc)
    TriangleSet(TriangleSet),
    QuadSet(QuadSet),
    FaceSet(FaceSet),
    PointSet(PointSet),
    Polyline(Polyline),

    // Curves and patches
    BezierCurve(BezierCurve),
    NurbsCurve(NurbsCurve),
    BezierPatch(BezierPatch),
    NurbsPatch(NurbsPatch),

    ElevationGrid(ElevationGrid),

    // Composition
    Group(Vec<GeometryRef>),
    Transformed(Box<Transformed>),
}
```

Traversal that needs to be open (user-supplied passes) uses a visitor trait,
with a provided `walk` that handles `Group`/`Transformed` recursion:

```rust
pub trait GeometryVisitor {
    type Output;
    fn visit(&mut self, g: &Geometry) -> Result<Self::Output>;
}
```

Everything in `algo/` is implemented as one of these, matching PlantGL's Action
hierarchy one-for-one so the mental model transfers.

### Transformations

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

`Tapered` is the one that bites: PlantGL's `Tapered` is a **`Deformed`, not an
`OrthoTransformed`** — it is a non-affine radial scale along the local Z axis
and cannot be folded into an accumulated 4×4. `MatrixComputer` must therefore
carry an optional deformation stack and `discretize` must apply `Tapered` to
vertex positions *after* the primitive is sampled, not before. Getting this
wrong silently produces cylinders where frusta were intended.

### Extrusion — the generalized cylinder

This is the single most valuable primitive to port, and it is what makes stems
stop looking like stacked cans:

```rust
pub struct Extrusion {
    pub axis: Curve3DRef,                 // Polyline / Bezier / NURBS
    pub cross_section: Curve2DRef,        // closed 2D profile
    pub scale: Vec<Vector2<f32>>,         // per-knot cross-section scale
    pub orientation: Vec<f32>,            // per-knot cross-section twist
    pub knot_list: Option<Vec<f32>>,      // where scale/orientation apply
    pub solid: bool,                      // cap the ends
    pub ccw: bool,
}
```

Discretisation samples the axis, builds a **rotation-minimising frame** (double
reflection method — the naive Frenet frame flips at inflection points and
twists the mesh), places a scaled/rotated copy of the cross-section at each
sample, and stitches consecutive rings. This is also what `Turtle::startGC()` /
`stopGC()` produce, and what `Turtle::sweep()` produces.

### The turtle

Ported command set, grouped as upstream groups it. `f/F/nF`, `left/right/up/
down/rollL/rollR/iRollL/iRollR/turnAround`, `rollToVert/rollToHorizontal`,
`setHead/eulerAngles/transform`, `push/pop`, `move/shift/lineTo/lineRel/
pinpoint/oLineTo/oLineRel`, `setWidth/incWidth/decWidth`, `setColor/incColor/
decColor/interpolateColors`, `scale/multScale/divScale`, `sphere/circle/box/
quad/label/surface`, `startGC/stopGC`, `startPolygon/polygonPoint/stopPolygon`,
`setCrossSection/setSectionResolution`, `setGuide/clearGuide/setPositionOnGuide/
sweep`, `setTropism/setElasticity/leftReflection/upReflection/headingReflection`,
`setDefaultStep/setAngleIncrement/setScaleMultiplier`, `setId/incId/decId`,
`start/stop/reset`, texture transform setters.

Skipped: `frame/arrow/vector` (debug viz — trivial to add later),
`setScreenCoordinatesEnabled`, `registerPushPopHandler`, static error handlers
(we return `Result`), `getScene`-style Qt hooks.

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

`push()`/`pop()` are a `Vec<TurtleState>` — the C++ `std::stack<TurtleParam*>`
with manual lifetime management becomes a plain value stack. `pop()` on an empty
stack returns `Err(TurtleError::EmptyStack)` rather than PlantGL's warn-and-
continue; the L-system driver in `botany` can choose to ignore it.

**Tropism** (ABOP §2.3 "tend to" operator): each `F` rotates the frame about
`heading × tropism` by `elasticity * |heading × tropism|`, then re-orthonormalises.
Gravitropism is `tropism = -Y`; phototropism aims at a light direction. This is
what makes vines droop and shade-plants lean, for a few lines of code.

**Drawer trait** — mirrors PlantGL's `Turtle`/`TurtleDrawer` split so the same
turtle program can produce a scene graph, a merged mesh, or nothing but metrics:

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

- `SceneDrawer` → `Scene` of `Shape`s. Inspectable, exportable, LOD-able later.
- `MeshDrawer` → one merged `TriangleSet` per material. The game path: skips
  the intermediate scene graph entirely for plants that never need editing.
- `MeasureDrawer` → surface area, volume, bbox, segment count. Zero allocation;
  use it to compute harvest yield without building geometry.

---

## Integration with the existing code

### Division of labour after the port

- `crates/plantgl` — geometry, turtle, tessellation. Engine- and game-agnostic.
- `crates/botany` — genetics, phenotype, L-system, stat mapping, and a new
  `interpret.rs` that walks `Vec<LSymbol>` and drives a `plantgl::Turtle`.
  This is precisely the L-Py ↔ PlantGL boundary upstream.
- `crates/game`, `crates/tools` — consume `plantgl::Scene` / `TriangleSet` and
  convert to Fyrox `SurfaceData`.

### File-by-file impact

| File | Change |
|---|---|
| `crates/botany/src/turtle.rs` | **Deleted.** Its `Vec3`, `rotate_around_axis`, and `TurtleInterpreter` are superseded by `plantgl::math` and `plantgl::modelling::turtle`. Its tests move to `botany/src/interpret.rs` and to `plantgl`'s turtle tests. |
| `crates/botany/src/interpret.rs` | **New.** `interpret(symbols, phenotype) -> plantgl::Scene`. Maps `LSymbol::Forward` → `turtle.f_draw(len)`, `TurnLeft` → `turtle.left(a)`, `Leaf` → `turtle.surface("leaf", scale)`, etc. |
| `crates/botany/src/mesh_gen.rs` | Gutted. `PlantMeshData` becomes a thin facade over `plantgl::TriangleSet` + organ instance list, preserved *only* so `tools`/`game` keep compiling during migration; `build_stem_mesh` and the hand-rolled OBJ/MTL writers are deleted in favour of `plantgl::codec::obj`. Removed entirely at the end of Phase E. |
| `crates/botany/src/lsystem.rs` | Extended: an `LSymbol` variant per newly available turtle command (`StartGC`/`StopGC`, `SetCrossSection`, `Tropism`, `Sweep`), plus parametric organ symbols. Rewriting engine unchanged. |
| `crates/botany/src/phenotype.rs` | Extended: `leaf_mesh_index: usize` → a `SurfaceLibrary` key; new genes surface as cross-section profile choice, taper curve, tropism elasticity, axis curvature. |
| `crates/botany/Cargo.toml` | Adds `plantgl = { path = "../plantgl" }`. |
| `crates/tools/src/plant_preview.rs` | `PlantPreviewData.mesh` becomes `plantgl::Scene`; summary reports real surface area and volume. |
| `crates/tools/src/bin/plant_previewer.rs` | Converts `plantgl::TriangleSet` → `SurfaceData` via the new bridge instead of ad-hoc per-instance triangles. |
| `crates/game/src/*` | Only the garden/hub plant spawn path; via the same bridge. |
| `Cargo.toml` (workspace) | Add `crates/plantgl` to `members`; add `nalgebra = "0.33"` to `workspace.dependencies`. |

### Fyrox bridge

Kept out of `plantgl` proper. Add `crates/botany/src/fyrox_bridge.rs` (or a
small `crates/plantgl-fyrox`) with:

```rust
pub fn to_surface_data(ts: &plantgl::TriangleSet) -> SurfaceData;
pub fn scene_to_node(scene: &plantgl::Scene, graph: &mut Graph) -> Handle<Node>;
```

`TriangleSet` is already SoA (`points`, `normals`, `tex_coords`, `indices`), so
this is a per-vertex repack into Fyrox's interleaved `StaticVertex`, and an
index `Vec<u32>` → `TriangleDefinition` map. Shapes sharing an appearance are
merged first by `algo::merge` so one plant is a handful of draw calls, not one
per leaf.

---

## Phase plan

Tasks follow the format of `07-task-breakdown.md`. This becomes **Phase 8** in
that document's numbering, sequenced after T5.5.

### Phase A — Foundation

#### T8.1 — Crate skeleton and math layer

**Deliverable**: `crates/plantgl` builds and is wired into the workspace.

**Steps**:
1. Create `crates/plantgl` with `Cargo.toml` as specified above; add to workspace members and add `nalgebra` to `workspace.dependencies`.
2. `src/math/mod.rs`: `Vec2`/`Vec3`/`Point3`/`Mat3`/`Mat4` aliases over nalgebra, degree/radian helpers, `approx_eq` tolerances.
3. `src/math/frame.rs`: `Frame { position, heading, left, up }`, construction from heading+up, Gram-Schmidt re-orthonormalisation, `to_matrix4`, rotation-minimising frame propagation (double reflection).
4. `src/error.rs`: `thiserror` `Error` covering degenerate geometry, empty stack, bad knot vector, unsupported operation.
5. Add the clean-room provenance header to every file.

**Acceptance**: `cargo build -p plantgl` and `cargo clippy -p plantgl -- -D warnings` clean. Frame tests: orthonormality preserved over 10 000 random rotations to 1e-5; RMF over a helix accumulates < 1e-4 twist.

**Dependencies**: none

**Complexity**: Low

#### T8.2 — Explicit meshes, appearance, scene

**Deliverable**: The container types every later phase writes into.

**Steps**:
1. `mesh/mod.rs`: `ExplicitModel` — `points: Arc<Vec<Point3<f32>>>`, optional `normals`, `tex_coords`, `colors`; `normal_per_vertex`, `color_per_vertex`, `ccw`, `solid`, `skeleton` flags mirroring PlantGL's `Mesh`.
2. `TriangleSet` (`Vec<[u32; 3]>`), `QuadSet`, `FaceSet` (variable-arity), `PointSet`, `Polyline`, `Group`.
3. `appearance.rs`: `Color3`/`Color4` (u8 and f32 forms), `Material { ambient, diffuse, specular, emission, shininess, transparency }`, `ImageTexture`, `Texture2D { image, transformation }`, `Texture2DTransformation { scale, translation, rotation_angle, rotation_center }`.
4. `scene.rs`: `Shape { geometry: GeometryRef, appearance: Arc<Appearance>, id: u32, parent_id: u32 }`, `Scene { shapes: Vec<Shape> }` with `merge`, `iter`, `bbox`.
5. `geometry.rs`: the `Geometry` enum (variants stubbed for not-yet-ported primitives), `GeometryVisitor` trait with a provided `walk` handling `Group`/`Transformed`.
6. `transform.rs`: `Transform` enum + `Transformed`; `Transform::to_matrix4` returning `Option<Matrix4<f32>>` (`None` for `Tapered`, which is a deformation).
7. `algo/matrix.rs`: `MatrixComputer` accumulating affine transforms with a separate deformation stack.

**Acceptance**: Round-trip `Scene` serde tests pass under the `serde` feature. `MatrixComputer` over nested `Translated(Scaled(Sphere))` yields the expected 4×4; `Tapered` is reported as a deformation rather than folded in.

**Dependencies**: T8.1

**Complexity**: Medium

#### T8.3 — OBJ export and the golden-test harness

**Deliverable**: Any `Scene` can be written to `.obj` + `.mtl`, and snapshot tests are in place.

**Steps**:
1. `codec/obj.rs`: writer over a tessellated scene — `v`/`vn`/`vt`/`f`, groups per `Shape`, `usemtl` per appearance, `mtllib`.
2. Golden-file harness in `tests/golden/`: generate, compare against committed `.obj`, `UPDATE_GOLDEN=1` to refresh.
3. Port the two existing OBJ tests from `botany/src/mesh_gen.rs`.

**Acceptance**: A hand-built `TriangleSet` round-trips to OBJ and back to identical vertex/index counts and bbox. Goldens are byte-stable across runs and platforms (fixed float formatting).

**Dependencies**: T8.2

**Complexity**: Low

### Phase B — Parametric primitives

#### T8.4 — Primitives and the discretizer

**Deliverable**: `Box`, `Sphere`, `Cone`, `Cylinder`, `Frustum`, `Disc`, `Paraboloid`, `SOR`, `Revolution`, `Swung`, `ElevationGrid` discretise to `ExplicitModel`.

**Steps**:
1. `primitive/*.rs`: parameter structs matching PlantGL's constructor signatures (`Cylinder { radius, height, solid, slices }`, `Frustum { radius, height, taper, solid, slices }`, `Sphere { radius, slices, stacks }`, …), with the same defaults.
2. `algo/discretize.rs`: `Discretizer { ctx: DiscretizeCtx }` implementing `GeometryVisitor<Output = ExplicitModel>`; `DiscretizeCtx { slices, stacks, curve_samples }` is the LOD knob.
3. Correct normals and UVs per primitive (spherical UV for `Sphere`, cylindrical for `SOR`-family, planar for `Disc`).
4. Apply `Tapered` deformation post-sampling; handle `solid` end caps.

**Acceptance**: For each primitive, discretised surface area and volume converge to the closed-form value within 1% at 64 slices (unit sphere → 4π / 4π/3; unit cylinder → 2πrh + caps). All meshes are manifold: every edge shared by exactly 2 triangles for `solid` primitives. Consistent winding — no inverted normals under `approx` assertions on the divergence test.

**Dependencies**: T8.3

**Complexity**: Medium

#### T8.5 — Tessellator, measurement, bounding volumes

**Deliverable**: `algo::{tessellate, measure, bbox, bsphere, merge, normals}`.

**Steps**:
1. `tessellate.rs`: `ExplicitModel` → `TriangleSet`; fan-triangulate convex faces, ear-clip concave ones (with a convexity fast path).
2. `bbox.rs`, `bsphere.rs`: AABB and Ritter bounding sphere over any `Geometry`, transform-aware.
3. `measure.rs`: `SurfComputer` (Σ triangle areas), `VolComputer` (signed tetrahedron sum about the origin — valid for closed meshes; return `Err` for non-solid geometry rather than a wrong number).
4. `merge.rs`: concatenate `TriangleSet`s with index rebasing, grouped by appearance.
5. `normals.rs`: face normals, area-weighted vertex normals, crease-angle smoothing threshold.

**Acceptance**: `SurfComputer` on the T8.4 primitives matches analytic areas within 1%. `VolComputer` on a translated unit cube returns 1.0 regardless of translation. `merge` of *n* sets preserves total triangle count and bbox. Ear-clipping handles a 20-vertex concave star without self-intersection (validated by area = sum of parts).

**Dependencies**: T8.4

**Complexity**: Medium-High

### Phase C — Curves and sweeps

#### T8.6 — Curve and patch types

**Deliverable**: `Polyline2D`, `BezierCurve(2D)`, `NurbsCurve(2D)`, `BezierPatch`, `NurbsPatch`.

**Steps**:
1. `curve/mod.rs`: `Curve2D` / `Curve3D` traits — `eval(u)`, `tangent(u)`, `first_knot()`, `last_knot()`, `length()`, `discretize(n)`, `is_closed()`.
2. `spline.rs`: de Casteljau for Bézier; de Boor + knot-span search for NURBS (span arithmetic in `f64`); derivative evaluation for tangents.
3. `patch.rs`: tensor-product Bézier and NURBS surfaces with `eval(u, v)` and analytic normals from partial derivatives.
4. Knot-vector validation, returning `Err(BadKnotVector)` for non-monotone or wrong-length input.
5. `Profile`-style helpers: circle-as-NURBS, and the default *n*-gon cross-section used by the turtle.

**Acceptance**: A NURBS circle (9 control points, standard rational weights) evaluates to radius 1 ± 1e-5 at 256 samples. Bézier degree elevation is value-preserving. Patch normals match finite-difference normals to 1e-4. Arc length of a straight NURBS line equals its chord length.

**Dependencies**: T8.5

**Complexity**: High — this is the phase most likely to slip; NURBS is fiddly.

#### T8.7 — Extrusion (generalized cylinder)

**Deliverable**: `Extrusion` discretises to a clean swept mesh.

**Steps**:
1. `primitive/extrusion.rs` as specified above.
2. Sample the axis; build a rotation-minimising frame chain; place transformed cross-sections; stitch rings; cap when `solid`.
3. Per-knot `scale` and `orientation` interpolation between knots.
4. `QuantisedFunction` in `function.rs` — sampled 1D function with linear interpolation and clamped domain, used as a radius profile.
5. Continuous UV parameterisation along and around the sweep.

**Acceptance**: An `Extrusion` of a circle along a straight axis is within 1% surface area of the equivalent `Cylinder`. Sweeping a circle along a helix produces zero frame flips (adjacent ring twist < 1°) — the case where a Frenet frame fails. A radius `QuantisedFunction` from 1.0 → 0.0 produces a cone within 1% of analytic volume.

**Dependencies**: T8.6

**Complexity**: High

### Phase D — The turtle

#### T8.8 — Turtle core

**Deliverable**: `plantgl::modelling::Turtle` with the movement, rotation, stack, width/colour/scale, and primitive-drawing command set.

**Steps**:
1. `param.rs`: `TurtleParam`/`TurtleState` and `TurtleDefaults { step, angle_increment, width_increment, color_increment, scale_multiplier, section_resolution }`.
2. `turtle.rs`: movement (`f`, `F`, `nF`, `move`, `shift`), rotation (`left/right/up/down/rollL/rollR/iRollL/iRollR/turnAround/rollToVert/rollToHorizontal/setHead/eulerAngles/transform`), stack (`push`/`pop`), width/colour/scale families, `lineTo`/`lineRel`/`pinpoint`/`oLineTo`/`oLineRel`, `sphere`/`circle`/`box`/`quad`/`surface`.
3. `drawer.rs` + `scene_drawer.rs` + `mesh_drawer.rs` + `MeasureDrawer`.
4. `SurfaceLibrary`: named `GeometryRef` templates that `surface(name, scale)` instances — this is how leaves and petals get placed.
5. Re-orthonormalise the frame every *N* rotations to stop drift over deep derivations.

**Acceptance**: `F(1)` from the default state produces one segment of length 1 along +Y. `push; F(1); pop; F(1)` produces two segments sharing a start point. `left(90)` then `F(1)` moves purely laterally. 10 000 random rotation commands leave the frame orthonormal to 1e-4. The existing `botany/src/turtle.rs` test suite passes against the new turtle after mechanical translation.

**Dependencies**: T8.7

**Complexity**: Medium-High

#### T8.9 — Turtle geometry modes: GC, polygons, guides, tropism

**Deliverable**: The features that distinguish PlantGL's turtle from a toy turtle.

**Steps**:
1. `startGC`/`stopGC`: accumulate positions, left-vectors and radii; emit one `Extrusion` on `stopGC` (and on `pop` while a GC is open, per upstream semantics).
2. `startPolygon`/`polygonPoint`/`stopPolygon(concave_test)`: emit a `FaceSet`; ear-clip when the concavity test trips. This is how L-system leaves are built without an asset.
3. `setCrossSection(curve, ccw)`, `setDefaultCrossSection(slices)`, `setSectionResolution`.
4. `path.rs`: `setGuide(curve, length)`, `clearGuide`, `setPositionOnGuide(t)`, `sweep(path, section, length, dl, radius, radius_variation)`.
5. `tropism.rs`: `setTropism`, `setElasticity`, the tend-to rotation applied on each forward move, plus `leftReflection`/`upReflection`/`headingReflection`.

**Acceptance**: A GC over 20 `F` commands produces a single connected surface — no interior seam vertices, triangle count matching `(rings-1) * section_res * 2`. A polygon of 5 coplanar points yields 3 triangles with a consistent normal. With `tropism = -Y, elasticity = 0.5`, a stem started horizontally bends monotonically downward and its final heading is closer to -Y than its initial. `setGuide` along an arc produces an axis whose sampled points lie on that arc within 1e-3.

**Dependencies**: T8.8

**Complexity**: High

### Phase E — Game integration

#### T8.10 — Rewire `crates/botany`

**Deliverable**: Plant generation runs entirely through `plantgl`; `botany/src/turtle.rs` is gone.

**Steps**:
1. Add `plantgl` to `botany`'s dependencies.
2. New `botany/src/interpret.rs`: `LSymbol` → turtle command dispatch, including the new symbols.
3. Extend `LSymbol` with `StartGC`, `StopGC`, `SetCrossSection(usize)`, `SetTropism(f32)`, `Surface(SurfaceId, f32)`; extend `LSystem::from_phenotype` to wrap each branch axis in `StartGC`/`StopGC` and to emit `Surface` for leaves/petals/fruit.
4. Extend `PlantPhenotype`: cross-section profile index, taper curve id, tropism elasticity, axis curvature, LOD tier.
5. Build the default `SurfaceLibrary` — a handful of procedurally generated `BezierPatch` leaf and petal shapes indexed by the existing `leaf_mesh_index` / `fruit_mesh_index` genes.
6. Delete `botany/src/turtle.rs`; reduce `mesh_gen.rs` to a re-export facade.

**Acceptance**: `cargo test --workspace` passes. `generate_plant_mesh(genotype, rng)` remains deterministic for a fixed seed. Visual check via the previewer: stems are continuous and tapered, leaves are surfaces. Triangle count for a mid-complexity plant stays under the budget in the Performance section.

**Dependencies**: T8.9

**Complexity**: Medium

#### T8.11 — Fyrox bridge and previewer update

**Deliverable**: `plantgl::Scene` renders in the game and the previewer.

**Steps**:
1. `botany/src/fyrox_bridge.rs`: `to_surface_data(&TriangleSet) -> SurfaceData`, `scene_to_node(&Scene, &mut Graph) -> Handle<Node>`, appearance → `Material`.
2. Merge by appearance before conversion so a plant is a few draw calls.
3. Update `crates/tools/src/plant_preview.rs` to carry a `plantgl::Scene` and report surface area, volume, bbox alongside the existing stats.
4. Update `crates/tools/src/bin/plant_previewer.rs` to build nodes through the bridge.
5. Update `crates/game` garden/hub spawn path.
6. Wire `DiscretizeCtx` to an LOD tier: hub-quality vs. distant vs. icon.

**Acceptance**: `cargo run --bin plant_previewer -- <seed>` renders a recognisable plant with smooth stems, and OBJ/MTL export still works. `cargo build --workspace` and `cargo clippy --workspace --tests -- -D warnings` clean.

**Dependencies**: T8.10

**Complexity**: Medium

#### T8.12 — Documentation and design-doc reconciliation

**Deliverable**: Docs describe the shipped architecture.

**Steps**:
1. Rewrite `04-apothecary-botany.md` §"L-System Procedural Plant Generation" and §"Porting from vlab/L-studio" to describe the `plantgl`/`botany` split.
2. Add T8.x to `07-task-breakdown.md`'s dependency graph as Phase 8.
3. Update `README.md`'s crate table with `plantgl/`.
4. `crates/plantgl/README.md`: scope, the clean-room provenance statement, the upstream mapping table, and a "porting a new primitive" walkthrough.

**Acceptance**: No design doc still describes `TurtleInterpreter` or `PlantMeshData::build_stem_mesh` as current.

**Dependencies**: T8.11

**Complexity**: Low

### Phase F — Optional extensions

Not on the critical path. Pick up if the game wants them.

- **T8.13 Space colonization** (`algo/modelling/spacecolonization`) — envelope-driven branching; a strong alternative generator for shrubs and canopies where an L-system looks too regular.
- **T8.14 PLY and glTF export** — glTF gets plants into Blender and into Fyrox's asset pipeline as baked assets rather than runtime generation.
- **T8.15 Hull primitives** (`AsymmetricHull`, `ExtrudedHull`) — AMAP-style crown envelopes, cheap distant-tree LODs.
- **T8.16 Cross-validation harness** — a dev-only script that runs the same parametric scene through conda-installed Python PlantGL and compares bbox, surface area, volume, and triangle counts against ours. Purely for confidence in our numbers; never a build dependency, and consumes only PlantGL's *outputs*, never its source, so it does not compromise the clean-room posture.
- **T8.17 Instancing / GPU batching** — one `SurfaceData` per organ template with per-instance transforms, rather than merged geometry, once plant counts grow.

---

## Testing strategy

Four layers, because "the mesh looks plausible" is not a test:

1. **Analytic.** Every primitive's discretised surface area and volume is
   checked against the closed-form value; convergence improves monotonically
   with slice count. This catches winding errors, missing caps, and
   double-counted vertices in one assertion.
2. **Invariants (proptest).** Over random command sequences: the turtle frame
   stays orthonormal; balanced `push`/`pop` restores state exactly; `merge`
   preserves triangle count and bbox; tessellation preserves total area;
   `solid` meshes are edge-manifold and closed (Σ signed volume > 0).
3. **Golden files.** Committed `.obj` snapshots for a fixed set of seeds and
   parametric scenes, with fixed float formatting so diffs are readable. These
   are the regression net for refactors.
4. **Determinism.** Same genotype + same seed ⇒ byte-identical OBJ. This
   already matters for saves (a garden stores genotypes, not meshes) and it
   must survive the port.

Optional fifth layer, T8.16: numerical cross-validation against upstream
PlantGL via Python.

---

## Performance budget

Plants are generated at garden load and on harvest, not per frame, so
throughput matters more than latency. Targets on a mid-range desktop, single
thread:

| Operation | Budget |
|---|---|
| Genotype → derived L-system string (6 iterations) | < 1 ms |
| Turtle interpretation + GC construction | < 3 ms |
| Discretise + tessellate + merge, hub LOD | < 10 ms |
| Total per plant, hub LOD | < 15 ms |
| Triangles per plant, hub LOD | < 12 000 |
| Triangles per plant, distant LOD | < 1 500 |
| Triangles per plant, inventory icon | < 400 |

Levers if we miss: lower `DiscretizeCtx::slices` (the single biggest knob),
`MeshDrawer` instead of `SceneDrawer` (skips the scene graph), organ instancing
instead of merging, and caching tessellated organ templates in the
`SurfaceLibrary` rather than re-discretising per placement. A full garden of 12
plots at 15 ms each is ~180 ms of load time — acceptable, and trivially
parallelisable with `rayon` per plot if it is not.

---

## Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **License contamination** — someone consults the C++ while implementing | High | Written policy in this doc and in `crates/plantgl/README.md`; provenance header per file; PR checklist item. If it happens, the affected file must be rewritten by someone who has not seen the source. |
| **NURBS (T8.6) slips** | Medium | It gates T8.7. Ship `Polyline`/`Bezier` curve support first and make `Extrusion` generic over `Curve3D` — NURBS then lands as an added impl, not a rework. |
| **Scope creep into a full CG library** | Medium | The Skip list is binding. Anything not needed by a plant goes to Phase F or gets dropped. |
| **`f32` precision in knot arithmetic** | Low | `f64` internally in `spline.rs`, downcast at the boundary; explicit test on a 200-control-point NURBS curve. |
| **Regression in existing plant visuals** | Medium | Golden OBJs captured from the *current* generator before T8.10, so the diff is visible rather than discovered later. |
| **Triangle-count blowup** — real geometry is much denser than triangle-per-leaf | Medium | LOD tiers are a T8.11 deliverable, not an afterthought; the budget table above is enforced by a test. |
| **Big-bang migration** | Medium | `PlantMeshData` stays as a facade through Phase E so `game`/`tools` never break; it is deleted only at T8.12. |

---

## Effort estimate

Rough, assuming one developer familiar with Rust and comfortable with
computational geometry:

| Phase | Tasks | New Rust (LOC, excl. tests) | Estimate |
|---|---|---|---|
| A — Foundation | T8.1–T8.3 | ~1 200 | 1 week |
| B — Primitives | T8.4–T8.5 | ~2 000 | 1.5 weeks |
| C — Curves & sweeps | T8.6–T8.7 | ~1 800 | 2 weeks |
| D — Turtle | T8.8–T8.9 | ~1 800 | 2 weeks |
| E — Integration | T8.10–T8.12 | ~900 | 1 week |
| **Core total** | | **~7 700** | **~7.5 weeks** |
| F — Optional | T8.13–T8.17 | ~2 000 | as needed |

For comparison, PlantGL's C++ core in the ported scope is roughly 45 000 lines.
The reduction is real, not optimism: we drop the Qt viewer, the OpenGL
renderer, the Python bindings, the legacy codecs, the container library, and
the manual reference counting, and sum types collapse the double-dispatch
boilerplate that dominates the `algo/base` visitors.

---

## Decisions needed before Phase A

1. **Licensing route** — options 1/2/3 above. Recommendation: **1 (clean-room, MIT)**.
2. **Crate name** — `plantgl` is free on crates.io. Alternative: `apothecarys-plantgl`, consistent with the workspace but not publishable as a general library. Recommendation: **`plantgl`**, since the whole point is that it is game-agnostic.
3. **Do we keep `f32`, or go `f64` with a downcast at the Fyrox boundary?** Recommendation: **`f32`**, `f64` only inside spline evaluation.
4. **Does the L-system stay in `botany` or move into `plantgl`?** Recommendation: **stays in `botany`** — it mirrors the upstream L-Py/PlantGL split and keeps `plantgl` a pure geometry library.

---

## References

- Pradal C., Boudon F., Nouguier C., Chopard J., Godin C. (2009). *PlantGL: A Python-based geometric library for 3D plant modelling at different scales.* Graphical Models 71(1):1–21.
- Prusinkiewicz P., Lindenmayer A. (1990). *The Algorithmic Beauty of Plants.* Springer. — turtle command set, tropism, generalized cylinders.
- Boudon F. et al. *L-Py: an L-system simulation framework.* — the upstream L-system engine that drives PlantGL's turtle.
- Wang W., Jüttler B., Zheng D., Liu Y. (2008). *Computation of rotation minimizing frames.* ACM TOG 27(1). — the double-reflection method used in `Extrusion`.
- Piegl L., Tiller W. (1997). *The NURBS Book*, 2nd ed. — de Boor, knot insertion, rational curves.
- [openalea/plantgl](https://github.com/openalea/plantgl) — API reference and structure (source not used; see Licensing).
