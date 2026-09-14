# Differential testing against upstream PlantGL

`crates/plantgl` is a **translation** of [openalea/plantgl][upstream], so the
strongest available check on it is to drive the same scene through both
implementations and compare the numbers. Because the CeCILL-C decision
([#17](https://github.com/seankain/apothecarys-satchel/issues/17)) lets us not
only read but *run* upstream, that check is legitimate and cheap — and it is
worth far more than analytic spot-checks on `Revolution`, `Swung`,
`ElevationGrid` and `Paraboloid`, where there is no closed form to check
against.

## How it is wired

Two halves, deliberately split so that **Python is never a build or test
dependency**:

| | |
|---|---|
| `upstream_measure.py` | Drives a conda-installed PlantGL and writes `crates/plantgl/tests/reference.json`. Needs conda. Run by hand. |
| `crates/plantgl/tests/differential.rs` | Reads the committed `reference.json` and compares. Needs nothing but `cargo test`. |

Because the reference is committed, the differential gate runs on **every**
`cargo test` rather than being an optional CI job somebody remembers to
trigger. Regenerating the reference is the manual step, and it is only needed
when the case list changes or upstream is rebased.

This is the "documented manual gate per phase" that
[#18](https://github.com/seankain/apothecarys-satchel/issues/18) asks for. The
repository has no CI workflows at present; if one is added later, the gate to
wire in is the regeneration step below, not the Rust test — that already runs
with everything else.

## Regenerating the reference

Create the environment once:

```bash
conda create -n plantgl-diff --override-channels \
    -c openalea3 -c conda-forge python=3.11 openalea.plantgl
```

The environment uses **only `conda-forge` and `openalea3`**. Anaconda's
`defaults` channels are deliberately excluded: they carry Terms of Service that
must be accepted before use and restrict commercial use at some organisation
sizes. Nothing here needs them, so `--override-channels` keeps the project
clear of that question entirely.

`environment.yml` in this directory records the same spec, but prefer the
command above. `conda env create -f` has no `--override-channels`, and its
`nodefaults` entry is **not** sufficient on its own — conda still consults the
`defaults` channels from your global config while solving and fails with
`CondaToSNonInteractiveError`. If you want to use the file anyway, neutralise
those channels for the one command:

```bash
CONDA_DEFAULT_CHANNELS="" conda env create -f tools/differential/environment.yml
```

Then regenerate:

```bash
conda run -n plantgl-diff python tools/differential/upstream_measure.py \
    --out crates/plantgl/tests/reference.json
cargo test -p plantgl --test differential
```

Review the diff to `reference.json` before committing it. A change there is a
change in what we are claiming upstream does, and it should be explicable.

To investigate one discrepancy:

```bash
conda run -n plantgl-diff python tools/differential/upstream_measure.py \
    --only revolution_closed --out /tmp/one.json
```

## Curves are compared too, differently

A curve is not a mesh: upstream's `Discretizer` reduces it to a `Polyline`, so
there is no area, volume or face count to compare. `CURVE_CASES` in the script
records what there is instead — for each curve, the point and the tangent at 33
parameters spanning its knot range, the polyline it discretises to at a fixed
stride, and its length. `crates/plantgl/tests/differential.rs` rebuilds the same
curves and compares all four.

The tolerance there is `1e-4` absolute, on coordinates of order 5. That is
deliberately loose relative to the port's own accuracy and deliberately tight
relative to any mistranslation: upstream evaluates in `f32` throughout and is
the *less* accurate of the two by around `1e-6` here, while a wrong knot span, a
transposed control point or a dropped weight moves a point by a tenth of the
curve.

## What is compared, and how tightly

Not everything upstream reports is comparable to what we report, and pretending
otherwise would produce a test that passes by being loose. The Rust side makes
three kinds of claim:

1. **Mesh topology** — point, face and triangle counts, the `solid` and `ccw`
   flags, the bounding box. Both sides run their own discretizer, so these must
   match **exactly** (the bbox to `1e-4`). This is the check that catches a
   mis-translated index expression, and it is the reason the harness exists.
2. **Mesh-measured area and volume** — where upstream's computer discretises
   too, both numbers describe the same mesh and must agree to `1e-4` relative.
3. **Analytic area and volume** — where upstream's computer returns the closed
   form of the *ideal* surface instead, our discretised measure can only
   converge towards it: the issue's 1% at 32 slices, looser at 8.

That third case is not a detail. Upstream's `SurfComputer` and `VolComputer` do
**not** always measure the mesh, and the two do not even agree with each other:

| Primitive | `SurfComputer` | `VolComputer` |
|---|---|---|
| Box, Cone, Cylinder, Disc, Frustum, Sphere | analytic | analytic |
| Paraboloid | measures the mesh | analytic (`π h r² s / (s + 2)`) |
| ElevationGrid | measures the mesh | analytic |
| Revolution, Swung | measures the mesh | measures the mesh |

`upstream_measure.py` records the split per measure, as `area_measures_mesh`
and `volume_measures_mesh`, so the Rust side picks the right tolerance instead
of guessing.

## What the harness has already found

**Six defects**, each pinned by a test that fails if an upstream rebase fixes
it, so a workaround can never outlive the thing it works around.

Two are in upstream's analytic area formulas, both dimensional errors — a length
returned where an area belongs, so neither is a matter of convention. The port
measures the mesh and therefore converges to the correct value; it does not
reproduce either. Both are pinned by
`known_upstream_area_defects_are_still_present`, which fails if a rebase fixes
them so the workaround can be removed.

- **`SurfComputer::process(Disc*)` returns the circumference.** The body is
  `__result = GEOM_TWO_PI * disc->getRadius();`, and the comment above it says
  `// 2 PI r`. The area of a disc is `π r²`. A unit disc hides this — `2π` vs
  `π` reads as a stray factor of two — which is why the case list carries
  `disc_r3`: for radius 3, upstream reports **18.85** (`2π·3`) where the area
  is **28.27** (`π·9`).
- **`SurfComputer::process(Frustum*)` adds its end caps as `π(r + q)`** instead
  of `π(r² + q²)`. Its non-solid branch is correct, which is why `frustum_open`
  matches us to `1e-4` and `frustum_solid` is 14% out. `frustum_cone` has
  `r = 1`, where `π r` and `π r²` coincide, so the bug is invisible there and
  that case is compared against upstream as normal.

The other two are in `BezierCurve::getTangentAt`, found when the curve cases
went in. Its *interior* branch is the true derivative; its two special cases are
not, and neither is the rational path:

- **A rational Bézier's tangent is not a tangent.** The function differences the
  stored control points — which are Cartesian points plus a weight, not
  homogeneous points — and calls `project()` on the difference, dividing by a
  difference of *weights* rather than applying the quotient rule. With every
  weight 1 the difference has `w = 0` and a guard returns the right answer,
  which is why the non-rational Bézier cases compare normally.
  `the_rational_bezier_tangent_defect_is_still_present` pins it from both sides:
  the port's tangent matches a central difference of *upstream's own points*,
  and upstream's does not — by up to a full reversal, mean direction error 0.83
  on `bezier_curve_rational`.
- **Both end points are special-cased, and both are wrong.** At `u = 0` the
  function returns `P1 - P0` **normalised**, where the derivative has magnitude
  `n · |P1 - P0|`; at `u = 1` it returns `P[n] - P[n-1]` **unscaled**, short by
  the factor `n`. Upstream's tangent field is therefore discontinuous at both
  ends of every Bézier curve. `the_bezier_endpoint_tangent_defects_are_still_present`
  asserts each of those two shapes exactly, and that the port's value is the
  limit of upstream's own interior branch.

`NurbsCurve::getTangentAt` overrides all of this with `deriveAt`, which is
correct, so the NURBS cases — including the rational circle — are compared
tangent for tangent with no exception at all.

The fifth is in `Discretizer::process(Extrusion*)`:

- **Every solid `Extrusion` has an inverted base.** Both end caps are fanned in
  the same vertex order — `range<Index>(nbPoints, 0, 1)` at the near end and the
  same from the last ring — and a cross-section wound counter-clockwise in the
  frame's `(left, up)` plane fans to a normal along `+heading`. At the far end
  that points out of the solid; at the near end it points straight in. Upstream
  cannot see it: `VolComputer` sums the *absolute* tetrahedra about the
  centroid, so the flip changes neither its volume nor the face count nor the
  area, and every other comparison in this file passes on that case regardless.
  `upstream_measure.py` therefore records `inward_faces` — how many faces point
  back towards the middle of the mesh — for every case, and
  `the_inverted_extrusion_base_is_upstreams_alone` asserts that upstream's count
  is exactly one cap's fan there, **zero on every other solid case** (so the
  measure means something and upstream is not being accused wholesale), and zero
  for the port on the same mesh.

The sixth is in `PglTurtleDrawer::generalizedCylinder`, found when the turtle
cases went in (#20):

- **The first ring of a sweep that starts after a turn is an ellipse.** The
  drawer sets `Extrusion::InitialNormal` to the turtle's `left` at the first
  recorded point, and `Extrusion::getInitialFrameAt` crosses that vector with
  the axis's first tangent *without orthogonalising it against that tangent or
  renormalising the result*. When the turtle turned between recording the point
  and drawing the first segment — which every branch does, and which tropism
  does on every step — the cross product is short by the cosine of that turn,
  and the ring is squashed along one axis by exactly that factor. It is in the
  reference in plain sight: `turtle_gc_branch` turns 40° and its first ring's
  radii run 0.0383 … 0.05, and `0.05 · cos 40° = 0.0383`.
  `the_skewed_first_ring_is_upstreams_alone` asserts that factor against the
  turn the program made, that a sweep which starts *without* a turn is
  unaffected (so the measure means something), and that the port's own first
  ring is the cross-section it was given in every program. The port expresses
  the same initial normal as a *rotation* of the cross-section — an angle,
  which cannot come out non-unit.

## Turtle programs are compared too, differently again

A turtle case is neither a geometry nor a curve: it is a *program*. `TURTLE_CASES`
in the script drives the same command sequence through upstream's `PglTurtle`,
and `turtle_programs_match_upstream` drives it through the port's. What is
compared per program is the scene each drew — the number of shapes, the kind of
each (under however many transformations placed it), and per shape its point
count, triangle count, surface area, bounding box and first swept ring — plus
the frame the turtle ended in: position, heading, left, up and width. That last
part matters as much as the geometry, because a turtle whose frame drifts
places every later organ wrongly and no single mesh would show it.

The twelve programs cover what T8.8 and T8.9 name: plain and tapered segments,
branching with `push`/`pop`, the standalone primitives, generalized cylinders
(with and without a branch), polygons, a set cross-section, tropism, a guide, a
guided sweep, and one plant-scale program that uses most of them at once.

Two of upstream's Python bindings are not callable in the build this was
generated against — `Turtle::sweep` (neither overload accepts a `Polyline`
path) and the radius-varying `nF` — so `turtle_guided_sweep` drives the
composition `sweep` performs (`setGuide` + `setCrossSection` + `nF`) directly.
The port's own `sweep` and its radius profile are covered by
`crates/plantgl/tests/turtle.rs`.

## The port's own divergences, asserted rather than excused

Three places where the port deliberately differs. Each is checked *as a
divergence*, with its predicted magnitude derived from upstream's own
measurements or from the geometry rather than hardcoded, so none can drift into
a silent mismatch.

- **Swept poles are collapsed.** Where a `Revolution` or `Swung` profile
  touches the axis, upstream sweeps the strip through it anyway, emitting one
  zero-area triangle per slice per pole and a zero-length edge that belongs to
  exactly one face — a non-manifold mesh with junk normals at the tip of every
  leaf, bud and fruit. The port emits one shared apex and fans into it.
  `upstream_measure.py` counts upstream's degenerate faces, and the Rust test
  asserts the port has exactly that many fewer and the identical surface area
  (a zero-area triangle contributes nothing).
- **Volume of a non-solid mesh is an error, not `0`.** Upstream returns `0`,
  which a caller cannot distinguish from a genuinely flat solid.
- **Swept frames are rotation-minimising.** Upstream's
  `Extrusion::getNextFrameAt` crosses the previous binormal with the new tangent
  — the projection method, whose error in the twist is second order in the step.
  The port carries frames by double reflection (Wang, Jüttler, Zheng and Liu
  2008), which is fourth order, so a sweep along a torsional axis does not wind
  its cross-section. Both produce the *same tangent* at every ring, so the two
  meshes differ only by a rotation of each ring about its own axis — bounded,
  not open-ended. `extrusion_straight` and its siblings pin the half of that
  claim where the two provably coincide: on a straight axis there is no rotation
  to minimise, and the meshes must match vertex for vertex. `extrusion_helix`
  pins the other half, with a bound derived from the geometry: a cross-section
  circle sampled as an *n*-gon moves by at most the sagitta `r(1 - cos(π/n))`
  under rotation, so that is the bounding-box tolerance, and
  `the_frame_divergence_is_only_a_rotation_of_each_ring` additionally checks
  that every vertex still lies on the tube — which a frame *flip*, the failure
  this is all here to avoid, would not.

Note the contrast with a zero-taper `Frustum`, which also collapses a ring to a
point: that goes through upstream's own `process(Frustum*)` code, which the
port translates verbatim, so the port reproduces upstream's degenerate cap
triangles exactly. The harness asserts that too — the port diverges where it
has decided to, and nowhere else.

## One thing that is not a divergence: the patch control matrix

Upstream's two patch classes read the same `Point4Matrix` transposed relative to
each other. `NurbsPatch::getPointAt` reads `getAt(uspan - p + k, vspan - q + l)`
and sizes its u knot vector from `getColumnSize()`, so its rows are indexed by
**u**. `BezierPatch::getPointAt` runs de Casteljau along `getRow(j)` for
`j <= vDegree` and defines `getUDegree() = getRowSize() - 1`, so its rows are
indexed by **v**. Both are self-consistent; together they are contradictory, and
a control net moved between the two classes in upstream comes out transposed.

The port picks `NurbsPatch`'s reading for both, and `bezier_net()` in the script
transposes on the way into upstream's `BezierPatch`. That transposition is
itself under test: `bezier_patch_bump` and `nurbs_patch_bump` describe the same
surface and come back with identical areas, and would not if the reading were
wrong on either side.

## Adding a case

1. Add it to `CASES` (a mesh), `CURVE_CASES` (a curve) or `TURTLE_CASES` (a
   turtle program) in `upstream_measure.py`.
2. Add the same name and parameters to `build()`, `build_curve()` or
   `run_turtle()` in `crates/plantgl/tests/differential.rs`.
3. Regenerate the reference and run the tests.

`every_reference_case_is_covered`, `every_reference_curve_is_covered` and
`every_reference_turtle_is_covered` fail if the two lists drift apart, so a typo
cannot silently drop a primitive, a curve or a program from the comparison.

## Licensing

This directory drives PlantGL but contains **no translated PlantGL code**, and
it sits outside `crates/plantgl`. It is MIT, like the rest of the workspace.
`crates/plantgl/tests/differential.rs` is inside the CeCILL-C crate and carries
that licence, but it too is original: it checks the translation rather than
being part of it.

[upstream]: https://github.com/openalea/plantgl
