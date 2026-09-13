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

The port is staged; see `docs/design/08-plantgl-port.md` in the repository root
for the full plan.

| Phase | Contents | State |
|---|---|---|
| A | Crate, licensing, math + frames, explicit meshes, scene graph, transforms, OBJ export | **done** |
| B | Parametric primitives, discretizer, tessellator, measurement, bounding volumes, merge | **done** |
| C | Bézier/NURBS curves and patches, `Extrusion` | **this crate today** |
| D | Turtle, guides, tropism, the L-system driver | not started |

Every `Geometry` variant now discretises. `Swung` is the one partial exception:
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
