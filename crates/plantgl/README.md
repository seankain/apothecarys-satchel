# plantgl

A Rust port of the core geometry and turtle-modelling layers of
[openalea/plantgl](https://github.com/openalea/plantgl), the C++/Python
geometric library for 3D plant modelling from CIRAD / INRIA / INRA.

## Licensing — read this first

**This crate is licensed CeCILL-C, not MIT**, unlike the rest of this
workspace. It is a *translation* of PlantGL's C++ source, and the CeCILL-C
definition of an Integrated Contribution names translations explicitly, so the
port is Modified Software under Article 5.3.2 and carries upstream's license.

The full text is in [`LICENSE`](LICENSE). Every module carries a provenance
header naming the upstream file it derives from and the upstream commit it was
read at, per Article 5.3.3's requirement that Integrated Contributions be
clearly identified and documented.

Software distributed under CeCILL-C comes with **no warranty**; see Articles 9
and 10 of the Agreement. Crates that merely *use* this one are Derivative
Software under Article 5.3.3 and may be distributed under another license,
provided the Article 6.4 notice is carried and this crate's source stays
available for as long as the larger work is distributed — see the repository
root's `THIRD-PARTY-LICENSES`.

> Not legal advice. Worth a lawyer's eye before commercial distribution.

## Upstream

- Source: <https://github.com/openalea/plantgl>
- Commit read for this port: `4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189`
- Copyright CIRAD/INRIA/INRA; authored by Frédéric Boudon, Christophe Pradal,
  Christophe Nouguier, with contributions of Christophe Godin, Nicolas Dones,
  Boris Adam and Pierre Barbier de Reuille.

Upstream asks that work using PlantGL cite:

> Pradal C., Boudon F., Nouguier C., Chopard J., Godin C. 2009. PlantGL: A
> python-based geometric library for 3D plant modelling at different scales.
> *Graphical Models*, 71: 1–21.

## Scope

This crate is PlantGL's **geometry and turtle-modelling layers**, and nothing
else: no Qt viewer, no GL renderer, no Python bindings, no legacy codecs, no
fitting, no spatial grids, no ray casting. What it does carry is everything an
L-system driver needs to turn a derived string into geometry a renderer can
take, and everything a gameplay system needs to measure that geometry without
building it.

The port was staged; see `docs/design/08-plantgl-port.md` in the repository
root for the full plan.

| Phase | Contents | State |
|---|---|---|
| A | Crate, licensing, math + frames, explicit meshes, scene graph, transforms, OBJ export | **done** |
| B | Parametric primitives, discretizer, tessellator, measurement, bounding volumes, merge | **done** |
| C | Bézier/NURBS curves and patches, `Extrusion` | **done** |
| D | Turtle, guides, tropism, cross-sections, the three drawers | **done** |
| E | Wiring it into the game — `crates/botany`, the Fyrox bridge | **done** (lives in `crates/botany`, not here) |
| F | Space colonization, PLY/glTF, hulls, instancing | as needed |

The L-system engine is deliberately *not* here. It is not in PlantGL either:
upstream's driver is [openalea/lpy](https://github.com/openalea/lpy), a
separate project. This repository's equivalent is `crates/botany`, which is
MIT and calls into this crate — the same split upstream makes, and the reason
the CeCILL-C boundary lands where it does.

## Upstream mapping

Where each part of PlantGL's C++ tree (`src/cpp/plantgl/`) ended up. Every
source file also carries its own header naming the upstream file it derives
from, so this table is a map, not the authority.

| Upstream | Here | Notes |
|---|---|---|
| `math/` | `src/math/` | `Vec3`/`Mat4` are nalgebra's; `Frame` is the turtle's basis, translated from `turtleparam` |
| `scenegraph/core/` (`Action`, `RCPtr`, `DeepCopier`) | — | Replaced by `enum Geometry` + `match`, `Arc<T>`, and `#[derive(Clone)]` |
| `scenegraph/geometry/` parametric | `src/scenegraph/primitive/` | `Box3`, `Sphere`, `Cone`, `Cylinder`, `Frustum`, `Disc`, `Paraboloid`, `Revolution`, `Swung`, `ElevationGrid`, `Extrusion` |
| `scenegraph/geometry/` explicit | `src/scenegraph/mesh.rs` | `ExplicitModel` + `TriangleSet`, `QuadSet`, `FaceSet`, `PointSet`, `Polyline`, `Group` |
| `scenegraph/geometry/` curves | `src/scenegraph/curve/` | `Polyline2D`, `BezierCurve(2D)`, `NurbsCurve(2D)`, `BezierPatch`, `NurbsPatch` |
| `scenegraph/transformation/` | `src/scenegraph/transform.rs` | `enum Transform` + `Transformed`; `Tapered` is the non-affine one |
| `scenegraph/appearance/` | `src/scenegraph/appearance.rs` | `Color3`/`Color4`, `Material`, `ImageTexture`, `Texture2D` |
| `scenegraph/container/` | — | `Vec<T>` and `Arc<Vec<T>>` |
| `scenegraph/function/` | `src/scenegraph/function.rs` | `QuantisedFunction` — the radius profile a sweep tapers along |
| `scenegraph/scene/` | `src/scenegraph/scene.rs` | `Scene`, `Shape`, `NOID` |
| `algo/base/discretizer` | `src/algo/discretize.rs` | Plus `DiscretizeCtx`, the LOD knob upstream lacks |
| `algo/base/tesselator` | `src/algo/tessellate.rs` | With an ear-clipping fallback for concave faces |
| `algo/base/{surf,vol}computer` | `src/algo/measure.rs` | Three upstream defects corrected; see below |
| `algo/base/{bbox,bsphere}computer` | `src/algo/{bbox,bsphere}.rs` | |
| `algo/base/matrixcomputer` | `src/algo/matrix.rs` | Transform accumulation down a geometry tree |
| `algo/base/merge` | `src/algo/merge.rs` | Plus `merge_scene`, which batches by appearance |
| `algo/base/mesh` normals | `src/algo/normals.rs` | Plus crease-angle smoothing, which upstream has no equivalent of |
| `algo/modelling/turtle*`, `pglturtle*` | `src/modelling/` | The cpfg/L-studio command set, `TurtleState`, guides, tropism |
| `algo/modelling/pglturtledrawer` | `src/modelling/{scene_drawer,geometry}.rs` | Split so every drawer builds the *same* shapes |
| — | `src/modelling/{mesh_drawer,measure_drawer}.rs` | Original: the game's fast path and the yield path |
| `algo/codec/` | `src/codec/obj.rs` | OBJ/MTL only |
| `algo/opengl/`, `gui/`, `python/` | — | Out of scope |

Every `Geometry` variant discretises. `Swung` is the one partial exception:
its cross-profile interpolation is available at degree 1 only, because upstream
*fits* a NURBS through the profiles (`ProfileInterpolation`) rather than
evaluating one, and that fitting routine is not ported; higher degrees report
`Error::Unsupported` rather than silently blending linearly.

`Extrusion` sweeps a 2D cross-section along a 3D axis under
**rotation-minimising frames**, carried by the double-reflection method of Wang
et al. (2008) rather than upstream's projection method — see the divergences
below. `NurbsCurve2D::circle` is the exact rational circle a cross-section
usually wants; `Polyline2D::circle` is the inscribed *n*-gon the turtle uses by
default.

## Differential testing

Because the CeCILL-C decision lets us read *and run* upstream, the port is
checked against real PlantGL rather than only against hand-derived closed
forms. `tools/differential/` in the repository root drives a conda-installed
PlantGL and writes `tests/reference.json`; `tests/differential.rs` compares
against it and needs no Python, so the gate runs on every `cargo test`. See
`tools/differential/README.md`.

It has already found five defects in upstream, each pinned by a test that fails
if a rebase fixes it:

- `SurfComputer::process(Disc*)` returns the **circumference**, not the area.
- `SurfComputer::process(Frustum*)`'s solid branch adds its end caps as
  `π(r + q)` rather than `π(r² + q²)`.
- `BezierCurve::getTangentAt` differences the stored control points and calls
  `project()` on the result, dividing by a difference of *weights* instead of
  applying the quotient rule — so a **rational** Bézier's tangent is not a
  tangent at all. (`NurbsCurve::getTangentAt` goes through `deriveAt` and is
  correct.)
- The same function special-cases both **end points** and gets both wrong: a
  normalised `P1 - P0` at `u = 0`, and `P[n] - P[n-1]` without the factor `n` at
  `u = 1`, so upstream's tangent field is discontinuous at both ends of every
  Bézier curve.
- `Discretizer::process(Extrusion*)` fans **both** end caps in the same vertex
  order, so every solid `Extrusion` it produces has a base that faces into the
  solid. Upstream cannot see it: `VolComputer` sums *absolute* tetrahedra about
  the centroid, and the face count and area are unchanged either way.

The port computes all five correctly and does not reproduce any of them.

## Porting a new primitive

The shape of the work, using `Cylinder` as the worked example. Everything here
is mechanical; the judgement is in step 5.

**1. Read the upstream pair.** `src/cpp/plantgl/scenegraph/geometry/cylinder.{h,cpp}`
at the commit named above, and the `Discretizer::process(Cylinder*)` case in
`src/cpp/plantgl/algo/base/discretizer.cpp`. Note its default field values and
its `isValid()` conditions — both are part of the interface.

**2. Add the struct** in `src/scenegraph/primitive/`, one file per primitive,
with a provenance header naming the upstream file and the license. `tests/licensing.rs`
fails the build if either is missing. Density fields are `Option<u8>`, not
`u8`: `None` means "ask the `DiscretizeCtx`", which is how the LOD tiers work
and the one place the port deliberately parts company with upstream's
constructor-argument model.

```rust
pub struct Cylinder {
    pub radius: Real,
    pub height: Real,
    pub solid: bool,
    pub slices: Option<u8>,
}
```

**3. Add the variant** to `enum Geometry` in `src/scenegraph/geometry.rs`, and
a `From` impl. The match arms this breaks are the checklist for the rest of the
work: the compiler will name every one.

**4. Translate `process()`** into a method on `Discretizer` in
`src/algo/discretize.rs`. Term for term, including the point order, the index
order and the winding — a mesh from this module must have upstream's vertex
numbering, because `tests/differential.rs` compares against a real PlantGL
install. Where upstream relies on `Point3Array` zero-initialising a cap-centre
slot, assign the point explicitly; the mesh is the same and the intent stops
being the allocator's business.

**5. Decide about the bugs.** Read the upstream routine adversarially before
translating it. Five defects have been found this way so far — they are
listed under **Differential testing** above — and each is pinned by a test
that *fails if a later upstream rebase fixes it*. If
you find another: compute it correctly here, add the test, and write the
divergence down in this README. Silently reproducing a defect and silently
fixing one are equally bad — the first ships a bug, the second makes the
differential harness unexplainable.

**6. Wire up the readers.** `src/algo/{bbox,bsphere,measure,matrix}.rs` and
`src/codec/obj.rs` each have a match over `Geometry`. Most primitives need
nothing beyond "discretise, then measure the mesh"; add a closed form only
where upstream has one, and make the test assert the closed form and the mesh
agree.

**7. Test it three ways.**
- `tests/analytic.rs` — the closed form. A cylinder of radius `r` and height
  `h`, solid, at `n` slices has the lateral area of a regular `n`-gonal prism,
  not `2πrh`; assert what the mesh actually is.
- `tests/invariants.rs` — properties that hold for every input: the bounding
  box contains every vertex, a solid mesh's volume is positive, normals are
  unit length, the vertex count matches the slice count.
- `tests/differential.rs` — against upstream. Regenerate `tests/reference.json`
  with `tools/differential/` (a conda-installed PlantGL) and add the case;
  the committed JSON means the gate runs on every `cargo test` with no Python.

**8. If the turtle should be able to draw it**, add a `TurtleDrawer` method in
`src/modelling/drawer.rs` with a default that draws nothing, the shape itself
in `src/modelling/geometry.rs`, and then the three drawers — scene, mesh,
measure. `tests/turtle.rs` asserts all three agree on the same command
sequence, which is the property that makes them interchangeable.

## Design notes

The port deliberately diverges from upstream where Rust does it better:

- `RCPtr<T>` / `RefCountObject` → `Arc<T>`, which is atomic, so `rayon` stays
  available to callers.
- The double-dispatch `Action` visitor → `enum Geometry` + `match`, exhaustive
  at compile time.
- `DeepCopier` → `#[derive(Clone)]`.
- Warn-and-continue error handling → `Result` + `thiserror`.
- `real_t` stays `f32`, matching upstream's default and the engine's vertex
  format — **except** inside spline evaluation, where knots, basis functions and
  de Boor accumulations run in `f64` and only the finished point is narrowed.
  That is the one place `f32`'s margin is genuinely thin: the basis functions
  divide by differences of knots.
- Swept frames are rotation-minimising by double reflection (fourth-order error)
  where upstream re-derives each frame from the previous binormal (second
  order). The two agree exactly on a straight axis and differ by a rotation of
  each ring on a curved one, which `tests/differential.rs` bounds rather than
  excuses.
- Upstream's two patch classes read the same control matrix transposed relative
  to each other — `BezierPatch` as `[v][u]`, `NurbsPatch` as `[u][v]`. The port
  uses `[u][v]` for both.
