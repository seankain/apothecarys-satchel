# 08 — PlantGL Integration

## Purpose

Link [openalea/plantgl](https://github.com/openalea/plantgl) into the workspace
over FFI and rebuild the game's plant mesh generation on top of it.

PlantGL is a C++/Python geometric library for 3D plant modelling from CIRAD /
INRIA / INRA (Pradal, Boudon, Nouguier et al.). It supplies exactly the layer
`crates/botany` is currently faking by hand: a scene graph of parametric plant
geometry, a 3D turtle that speaks the standard cpfg/L-studio command set, and
the discretisation machinery that turns both into triangle meshes.

> **History.** An earlier revision of this document planned a clean-room Rust
> *port* of PlantGL. That was rejected in favour of binding the real library —
> see [§ Decision](#decision-mit--ffi). The port framing survives nowhere in
> this document; if you find it, it is a bug.

### What the game gets out of it

The present `crates/botany/src/mesh_gen.rs` builds stems as a chain of
disconnected, untapered 6-gon tubes and represents every leaf, flower and fruit
as a bare position + direction that the previewer renders as a flat triangle.
Adopting PlantGL's model buys us:

| Capability | Today | After integration |
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
| LOD | None | Tessellation density is a per-primitive parameter; one genotype → hub-quality and inventory-icon-quality meshes |
| Export | Hand-rolled OBJ | OBJ from marshalled buffers; upstream's PLY printer for free |

The surface-area point is worth flagging to design: PlantGL gives us a cheap,
principled number for "how much plant is there", which is a better harvest-yield
driver than a genotype scalar and ties the visible phenotype to the reward.

---

## Decision: MIT + FFI

**The workspace stays MIT. We do not reimplement PlantGL — we link against it.**

| | |
|---|---|
| Workspace license | **MIT**, unchanged |
| PlantGL | external CeCILL-C component, unmodified except build patches |
| Rust bindings, C shim, game | **MIT** |

### Why this is legally sound

PlantGL is CeCILL-C — the LGPL-like member of the CeCILL family ("C" for
*composant*), written for precisely this use. It distinguishes an **Integrated
Contribution** (a change to PlantGL's own source, which stays CeCILL-C) from a
**Related Module / Derivative Software** (code adding functionality *without*
modifying PlantGL's source). Derivative Software **may be distributed under a
license other than CeCILL-C**, provided notices concerning rights over the
Software are preserved.

Calling an API across FFI is not derivation, so our bindings and the game stay
MIT.

> Not legal advice. Worth a lawyer's eye before commercial distribution — the
> obligations below attach to *shipping*, not to developing.

### Obligations we take on

- Ship the full CeCILL-C license text and the CIRAD/INRIA/INRA copyright notices.
- State in the README that the game bundles a CeCILL-C component.
- Provide recipients a route to PlantGL's source **and to any patch we carry** —
  a patch to upstream's CMake is an *Integrated Contribution*, so the patch
  itself is CeCILL-C and must be published.
- Cite Pradal et al. 2009, as upstream requests.

### What the decision buys

- **No clean-room discipline.** We may read PlantGL's source freely. Under the
  port plan this was the single largest process risk; it is now absent.
- No reimplementation of NURBS, tessellation, rotation-minimising frames,
  extrusion or ear-clipping — the parts most likely to be subtly wrong.
- Numerical fidelity exact by construction.

### What it costs

- `cargo build` alone no longer suffices; a C++ toolchain and PlantGL's
  dependency tree enter the picture. **This is the whole cost, and it is
  front-loaded into Phase A.**
- Memory and exception safety across the boundary are ours, not the compiler's.
- Probably no cross-thread parallelism — see [§ Threading](#threading).

### Rejected alternatives

1. **Clean-room Rust port, MIT.** ~7.5 weeks and ~7 700 lines of geometry code,
   with the reimplementation risk concentrated in NURBS and sweeps, plus a
   standing process burden: nobody may consult upstream's source.
2. **Translate the C++, relicense the crate CeCILL-C.** Faster to write, but
   `crates/plantgl` and everything statically linking it inherits CeCILL-C
   source-availability obligations, which reaches the shipped game binary.

---

## The build problem

This is the concrete cost of the decision, so plan around it before anything
else. From upstream's `CMakeLists.txt` and conda recipe:

- **`find_package(Qt6/Qt5 ... REQUIRED)` with no conditional logic**, and there
  is **no core-only toggle.** Building `scenegraph` + `algo` currently drags in
  Qt, OpenGL, Python, PNG, JPEG, ZLIB and Threads.
- Optional but probed: Boost (python, numpy), CGAL, GMP, Eigen3, ANN, Qhull.
- Build tools: **cmake, bison, flex**, a C++ compiler.

Three ways out, in preference order:

| | Approach | Trade |
|---|---|---|
| **1** ✅ | Carry a CMake patch making Qt/OpenGL/Python optional; build only `scenegraph` + `algo` minus `algo/opengl` | Right answer. A patch we maintain against upstream, and being an Integrated Contribution it is CeCILL-C and must be published |
| 2 | Accept Qt as a build dependency | Simplest to stand up. Every contributor, the devcontainer and CI install Qt for a game that never touches PlantGL's viewer |
| 3 | Consume a conda-provided prebuilt PlantGL | Worst for contributors — `cargo build` would depend on an activated conda env |

**Recommendation: 1**, falling back to **2** if the patch proves unmaintainable.
Excluding `algo/codec`'s parser may also drop the bison/flex requirement.

---

## Upstream map

PlantGL's C++ tree (`src/cpp/plantgl/`), annotated with what we bind:

```
math/          vectors, matrices, quaternions            -- not bound (nalgebra our side)
scenegraph/
  core/        Action (visitor), SceneObject, refcounting -- internal; shim hides it
  geometry/    41 primitives                              -- BIND (subset, Phase B/C)
  transformation/  translated, scaled, oriented, axisrotated,
                   eulerrotated, mattransformed, tapered  -- BIND (Phase B)
  appearance/  material, texture, colours, spectra        -- BIND (Phase B)
  container/   typed shared arrays                        -- internal; marshalled at the shim
  function/    QuantisedFunction (radius profiles)        -- BIND (Phase C)
  scene/       Scene, Shape                               -- BIND (Phase B)
algo/
  base/        discretizer, tesselator, bbox/bsphere,
               surf/vol computers, merge, matrix          -- BIND (Phase B)
               skelcomputer, dijkstra, pointmanipulation  -- defer (Phase F)
  modelling/   turtle, turtleparam, turtlepath, pglturtle -- BIND (Phase D)
               spacecolonization                          -- defer (Phase F)
  codec/       PLY, VRML, X3D, POV-Ray, VGStar, LIG, DTA  -- defer (Phase F); OBJ is ours
  opengl/      GL renderer                                -- never (Fyrox renders)
  fitting/ grid/ raycasting/ projection/                  -- defer (Phase F)
gui/           Qt viewer                                  -- never
python/        Boost.Python bindings                      -- never
```

Note what is **not** in PlantGL: the L-system engine. That is
[openalea/lpy](https://github.com/openalea/lpy), a separate project that drives
PlantGL's turtle. `crates/botany/src/lsystem.rs` therefore stays where it is;
the split we adopt matches upstream's own.

### Primitives bound in the core phases

`TriangleSet` `QuadSet` `FaceSet` `PointSet` `Polyline` `Polyline2D` `Group`
`Box` `Sphere` `Cone` `Cylinder` `Frustum` `Disc` `Paraboloid` `Revolution`
`Swung` `Extrusion` `BezierCurve(2D)` `NurbsCurve(2D)` `BezierPatch`
`NurbsPatch` `ElevationGrid`.

Deferred: `AsymmetricHull` `ExtrudedHull` (AMAP crown envelopes — attractive
later for distant-tree LODs), `AmapSymbol` `Text` `Plane` `ScreenProjected`
`IFS`.

---

## Architecture

### Two crates

```
crates/plantgl-sys/          raw FFI — MIT
  build.rs                   cmake crate drives the PlantGL build; cc + bindgen for the shim
  vendor/plantgl/            git submodule, pinned commit, CeCILL-C
  patches/                   CMake patches (CeCILL-C — published)
  shim/pgl_shim.{h,cpp}      extern "C" surface, MIT
  src/lib.rs                 include!(bindgen output); no safety here

crates/plantgl/              safe idiomatic wrapper — MIT
  src/{geometry,curve,turtle,algo,scene,codec}.rs
```

Neither crate depends on `apothecarys-core` or Fyrox. The Fyrox bridge lives in
`crates/botany`.

### Why bindgen alone will not work

PlantGL's API is C++: templates (`RCPtr<T>`), `std::vector`, virtual dispatch,
exceptions, and heavy overloading — `F()`, `F(len)`, `F(len, topradius)` are
three distinct methods. Rust FFI is C-ABI only. So:

**A hand-written `extern "C"` shim is mandatory**, with `bindgen` run over the
*shim header only*, never over PlantGL's own headers. The shim is code we write
that merely calls PlantGL, so it is a Related Module and stays MIT.

`cxx` was considered and rejected for the `-sys` layer: it wants
`std::unique_ptr`/`shared_ptr`, and PlantGL's intrusive `RCPtr` does not map
onto either without a C++ facade — which is the shim again, plus a second
dependency.

### Shim rules

These are not style preferences; each prevents a specific class of UB.

1. **Opaque handles.** The shim heap-allocates an `RCPtr`, hands Rust an opaque
   pointer, and frees through an explicit `pgl_*_free`. Rust never sees a
   refcount.
2. **No exception crosses the boundary.** Every shim function wraps its body in
   `try`/`catch(...)` and returns an error code. A C++ exception unwinding into
   Rust is undefined behaviour.
3. **Flatten overloads mechanically.** `pgl_turtle_F`, `pgl_turtle_F_len`,
   `pgl_turtle_F_len_top`. Pick the convention once; it is most of the shim's
   line count.
4. **Validate on the C++ side.** Degenerate radii, bad knot vectors and wrong
   array lengths become error codes, because upstream may assert or throw.
5. **Owning references for composites.** An `Extrusion` holds its axis and
   cross-section curves; the shim takes owning `RCPtr`s so upstream's refcount
   keeps them alive. Never hand upstream a pointer Rust may free.

```c
/* pgl_shim.h — sketch */
typedef struct PglGeometry PglGeometry;
typedef struct PglTurtle   PglTurtle;
typedef enum { PGL_OK = 0, PGL_ERR_INVALID_ARG, PGL_ERR_EXCEPTION } PglStatus;

PglStatus pgl_cylinder_new(float radius, float height, int solid,
                           uint32_t slices, PglGeometry** out);
PglStatus pgl_nurbscurve_new(const float* ctrl_xyzw, size_t n,
                             const float* knots, size_t nk,
                             uint32_t degree, PglGeometry** out);
PglStatus pgl_tessellate(const PglGeometry* g, PglTriangleSet** out);
PglStatus pgl_triangleset_buffers(const PglTriangleSet* ts,
                                  const float** pts, size_t* npts,
                                  const uint32_t** idx, size_t* nidx);
void      pgl_geometry_free(PglGeometry*);
const char* pgl_last_error(void);
```

```rust
// crates/plantgl — safe wrapper sketch
pub struct Geometry(NonNull<sys::PglGeometry>);

impl Geometry {
    pub fn cylinder(radius: f32, height: f32, solid: bool, slices: u32)
        -> Result<Self, PlantGlError>
    {
        let mut out = ptr::null_mut();
        // SAFETY: shim validates arguments and cannot unwind.
        let st = unsafe {
            sys::pgl_cylinder_new(radius, height, solid as i32, slices, &mut out)
        };
        PlantGlError::check(st)?;
        Ok(Self(NonNull::new(out).ok_or(PlantGlError::NullHandle)?))
    }
}

impl Drop for Geometry {
    fn drop(&mut self) {
        unsafe { sys::pgl_geometry_free(self.0.as_ptr()) }
    }
}
```

### Scalars: `real_t` is `f32`

Upstream declares `typedef float real_t;` unless `PGL_USE_DOUBLE` is defined.
**Build without that flag** and `real_t` is `f32` — matching Fyrox's vertex
format exactly, so the Fyrox bridge is a repack with no numeric conversion.

### Threading

PlantGL's `RCPtr` refcounting is very likely non-atomic. Until proven
otherwise, handle types are `!Send + !Sync`. The practical consequence lands in
Phase E: **`rayon`-per-plot garden generation is off the table**, so the
per-plant budget carries more weight than it did.

### Marshalling and FFI chattiness

Mesh readback copies into Rust `Vec`s rather than borrowing across the
boundary. At ~12 000 triangles per plant that is microseconds against a 15 ms
budget — do the copy, do not get clever.

Driving the turtle one FFI call per L-system symbol is likewise fine: call
overhead is single-digit nanoseconds, so a 10 000-symbol plant costs
microseconds. **Do not build a command-buffer batching protocol.**

### Drawers: bind `PglTurtle`, do not implement one in Rust

Upstream's `TurtleDrawer` is a C++ abstract class. Implementing it in Rust means
a C++ subclass whose virtuals call function pointers back into Rust — putting
Rust frames on a C++ unwind path, the easiest available UB.

Instead: bind `PglTurtle`, call `getScene()`, then tessellate and merge. Same
result, no callbacks. Metrics come from `SurfComputer`/`VolComputer` on the
resulting scene rather than from a measurement drawer. Revisit only if
profiling in Phase E demands it.

---

## Integration with the existing code

### Division of labour

- `crates/plantgl-sys` / `crates/plantgl` — geometry, turtle, tessellation.
  Engine- and game-agnostic.
- `crates/botany` — genetics, phenotype, L-system, stat mapping, plus a new
  `interpret.rs` driving the bound turtle. Precisely the L-Py ↔ PlantGL
  boundary upstream.
- `crates/game`, `crates/tools` — consume marshalled meshes via the Fyrox
  bridge.

### File-by-file impact

| File | Change |
|---|---|
| `crates/botany/src/turtle.rs` | **Deleted.** Superseded by the bound `PglTurtle`. Its tests move to `botany/src/interpret.rs`. |
| `crates/botany/src/interpret.rs` | **New.** `interpret(symbols, phenotype) -> plantgl::Scene`, one turtle call per symbol. |
| `crates/botany/src/mesh_gen.rs` | Gutted. `PlantMeshData` becomes a thin facade *only* so `tools`/`game` keep compiling during migration; the hand-rolled stem builder and OBJ/MTL writers are deleted. Removed entirely at T8.12. |
| `crates/botany/src/lsystem.rs` | Extended: `StartGC`, `StopGC`, `SetCrossSection`, `SetTropism`, `Surface`. Rewriting engine unchanged. |
| `crates/botany/src/phenotype.rs` | Extended: cross-section profile, taper curve, tropism elasticity, axis curvature, **and per-primitive LOD slice counts**. |
| `crates/botany/src/fyrox_bridge.rs` | **New.** Marshalled buffers → `SurfaceData`; appearance → `Material`. |
| `crates/tools/src/plant_preview.rs` | Carries a `plantgl::Scene`; reports real surface area and volume. |
| `crates/tools/src/bin/plant_previewer.rs` | Builds nodes through the bridge. |
| `crates/game/src/*` | Garden/hub plant spawn path only. |
| `Cargo.toml` (workspace) | Add `crates/plantgl-sys`, `crates/plantgl`. |
| `.devcontainer/`, CI, `README.md` | C++ toolchain and per-platform system dependencies. |

### LOD is per-primitive

Upstream takes tessellation density as **constructor arguments** —
`Sphere(radius, slices, stacks)`, `Cylinder(radius, height, solid, slices)` —
and the `Discretizer` action merely applies. There is no global resolution
context. LOD tiers therefore mean threading slice counts through
`LSystem::from_phenotype` into each primitive, not setting one knob.

---

## Phase plan

Tracked as GitHub issues; this table is the index.

| Phase | Issue | Contents | Estimate |
|---|---|---|---|
| A — Foundation | [#17](https://github.com/seankain/apothecarys-satchel/issues/17) | T8.1–T8.3: `plantgl-sys` build, C shim, safe wrapper, marshalling, OBJ, attribution | **2–3 weeks** |
| B — Primitives | [#18](https://github.com/seankain/apothecarys-satchel/issues/18) | T8.4–T8.5: bind geometry construction, discretizer, tessellator, measurement | 4–5 days |
| C — Curves | [#19](https://github.com/seankain/apothecarys-satchel/issues/19) | T8.6–T8.7: bind curves, patches, `Extrusion`, `QuantisedFunction` | 3–4 days |
| D — Turtle | [#20](https://github.com/seankain/apothecarys-satchel/issues/20) | T8.8–T8.9: bind `PglTurtle`, GC, polygons, guides, tropism | ~1 week |
| E — Integration | [#21](https://github.com/seankain/apothecarys-satchel/issues/21) | T8.10–T8.12: rewire botany, Fyrox bridge, doc reconciliation | ~1 week |
| F — Optional | [#22](https://github.com/seankain/apothecarys-satchel/issues/22) | T8.13–T8.17: space colonization, PLY/glTF, hulls, instancing | as needed |

**Core total: ~5.5–6.5 weeks**, against ~7.5 for the port. The saving is smaller
than the "no geometry to write" framing suggests, because Phase A absorbs the
entire cross-platform build cost. What changes more than the total is the
**risk shape**: concentrated in one build task that is tractable and knowable,
rather than spread across NURBS, sweeps and tessellation.

Full task breakdowns, acceptance criteria and open decisions live in the issues.

---

## Testing strategy

The layers survive the change of route, but three of them now test *the
binding* rather than the algorithm — which is the right target, because
marshalling is where FFI fails silently.

1. **Analytic.** A unit sphere at 64 slices measures ≈4π area and ≈4π/3 volume.
   Upstream's maths is not in question; a failure means we passed something in
   wrong — wrong stride, wrong weight convention, off-by-one knot count.
2. **Memory safety.** valgrind/ASan over construction–tessellation–drop loops;
   Miri where it can reach. Explicit lifetime tests: drop an axis curve while
   its `Extrusion` lives.
3. **Golden files.** Committed `.obj` snapshots with fixed float formatting.
   **Capture goldens from the current generator before Phase E** so the visual
   diff is reviewable rather than discovered.
4. **Determinism.** Same genotype + seed ⇒ byte-identical OBJ. This now depends
   on upstream's behaviour too, so assert it explicitly rather than assuming it.

Two error paths deserve dedicated tests, since they are the ones that turn into
UB rather than failures: a degenerate primitive must return `Err` and not
unwind, and a malformed knot vector must return `Err` and not assert.

---

## Performance budget

Plants are generated at garden load and on harvest, not per frame. Targets on a
mid-range desktop, single thread:

| Operation | Budget |
|---|---|
| Genotype → derived L-system string (6 iterations) | < 1 ms |
| Turtle interpretation, incl. per-symbol FFI | < 3 ms |
| Discretise + tessellate + merge + marshal, hub LOD | < 10 ms |
| **Total per plant, hub LOD** | **< 15 ms** |
| Triangles per plant, hub LOD | < 12 000 |
| Triangles per plant, distant LOD | < 1 500 |
| Triangles per plant, inventory icon | < 400 |

Levers if we miss: lower per-primitive slice counts (the biggest knob), cache
tessellated organ templates instead of rebuilding per placement, or bake the
best genotypes to static assets via glTF (T8.14). **`rayon` per plot is not
among them** if handles are `!Send` — a 12-plot garden then costs ~180 ms
serially, acceptable but without an escape hatch.

---

## Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **Cross-platform C++ build** — Linux, macOS and Windows must all work from a fresh clone | **High** | The dominant risk and the reason Phase A is 2–3 weeks. Prove all three platforms early in T8.1, before any binding work. CI caches the built library so a cold build does not gate every PR |
| **Qt patch drifts from upstream** | Medium | Pin the submodule to a commit; bump deliberately. Fallback is build strategy 2 (accept Qt) |
| **UB across the boundary** — exceptions, lifetimes, ownership | Medium | The five shim rules, enforced by review. ASan/valgrind in CI, not just locally |
| **Handles are `!Send`** | Medium | Assume it; verify in T8.2. Removes the parallelism fallback from the budget rather than breaking anything |
| **Attribution obligations missed at ship time** | Medium | Discharged as a T8.3 deliverable with a release-build check, not left to launch |
| **Regression in existing plant visuals** | Medium | Golden OBJs captured from the current generator before T8.10 |
| **Triangle-count blowup** — real geometry is far denser than triangle-per-leaf | Medium | LOD tiers are a T8.11 deliverable, not an afterthought; the budget is enforced by a test |
| **Big-bang migration** | Low | `PlantMeshData` stays a facade through Phase E; deleted only at T8.12 |
| **Binary size / link complexity** | Low | Measure at T8.11; strip the unused `algo` subsystems from the build |

---

## References

- Pradal C., Boudon F., Nouguier C., Chopard J., Godin C. (2009). *PlantGL: A Python-based geometric library for 3D plant modelling at different scales.* Graphical Models 71(1):1–21.
- Prusinkiewicz P., Lindenmayer A. (1990). *The Algorithmic Beauty of Plants.* Springer — the turtle command set and tropism model PlantGL implements.
- Boudon F. et al. *L-Py: an L-system simulation framework* — the upstream L-system engine that drives PlantGL's turtle.
- [openalea/plantgl](https://github.com/openalea/plantgl) — source, API reference, CMake and conda recipe.
- [CeCILL-C license text](http://www.cecill.info/licences/Licence_CeCILL-C_V1-en.html) and the [CeCILL FAQ](http://www.cecill.info/faq.en.html).
- The Rust FFI Omnibus and the `bindgen` user guide — shim and binding conventions.
