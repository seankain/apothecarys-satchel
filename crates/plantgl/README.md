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
| A | Crate, licensing, math + frames, explicit meshes, scene graph, transforms, OBJ export | **this crate today** |
| B | Parametric primitives, discretizer, tessellator, measurement | not started |
| C | Bézier/NURBS curves and patches, `Extrusion` | not started |
| D | Turtle, generalized cylinders, guides, tropism | not started |

Variants of `Geometry` whose primitives are not yet ported are present as
stubs so the enum shape is stable; visitors report them as
`Error::Unsupported`.

## Design notes

The port deliberately diverges from upstream where Rust does it better:

- `RCPtr<T>` / `RefCountObject` → `Arc<T>`, which is atomic, so `rayon` stays
  available to callers.
- The double-dispatch `Action` visitor → `enum Geometry` + `match`, exhaustive
  at compile time.
- `DeepCopier` → `#[derive(Clone)]`.
- Warn-and-continue error handling → `Result` + `thiserror`.
- `real_t` stays `f32`, matching upstream's default and the engine's vertex
  format.
