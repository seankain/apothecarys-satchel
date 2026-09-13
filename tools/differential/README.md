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

Two defects in upstream's analytic formulas, both dimensional errors — a length
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

## The port's own divergences, asserted rather than excused

Two places where the port deliberately differs. Both are checked *as
divergences*, with their predicted magnitude derived from upstream's own
measurements rather than hardcoded, so neither can drift into a silent
mismatch.

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

Note the contrast with a zero-taper `Frustum`, which also collapses a ring to a
point: that goes through upstream's own `process(Frustum*)` code, which the
port translates verbatim, so the port reproduces upstream's degenerate cap
triangles exactly. The harness asserts that too — the port diverges where it
has decided to, and nowhere else.

## Adding a case

1. Add it to `CASES` in `upstream_measure.py`.
2. Add the same name and parameters to `build()` in
   `crates/plantgl/tests/differential.rs`.
3. Regenerate the reference and run the tests.

`every_reference_case_is_covered` fails if the two lists drift apart, so a typo
cannot silently drop a primitive from the comparison.

## Licensing

This directory drives PlantGL but contains **no translated PlantGL code**, and
it sits outside `crates/plantgl`. It is MIT, like the rest of the workspace.
`crates/plantgl/tests/differential.rs` is inside the CeCILL-C crate and carries
that licence, but it too is original: it checks the translation rather than
being part of it.

[upstream]: https://github.com/openalea/plantgl
