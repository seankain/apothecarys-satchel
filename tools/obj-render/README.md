# obj-render

A flat-shaded orthographic renderer for Wavefront OBJ, in the standard library
and nothing else — no PIL, no numpy, no GL context, no display.

It exists for one job: the before/after comparison #21's definition of done
asks for. The game's previewer needs a window and a GPU, which a CI box and a
headless dev container do not have, and a screenshot of a window is in any case
one step further from the thing that changed than the exported geometry is.
This renders the OBJs themselves, so what you are looking at is the mesh the
golden tests assert.

## Use

```bash
python3 tools/obj-render/render_obj.py out.png first.obj [second.obj …]
```

Each OBJ becomes one panel, left to right, and **every panel shares one camera
and one scale** — fitted to the union of their bounding boxes. That is the
point: a plant that got taller or denser should look taller or denser, not be
silently normalised to the same frame.

Colours come from the `.mtl` sitting beside each `.obj` (same stem, `Kd` per
`newmtl`), so a material change shows up too. Shading is Lambertian on the face
normal with a fixed key light; hidden surfaces are removed with a per-panel
depth buffer.

## Regenerating the Phase E comparison

```bash
for seed in 1 42 100 999 12345; do
  python3 tools/obj-render/render_obj.py \
    docs/design/images/plantgl-phase-e/seed-$seed.png \
    crates/botany/tests/golden/pre-plantgl/plant_seed_$seed.obj \
    crates/botany/tests/golden/plant_seed_$seed.obj
done
```

Left panel: the hand-rolled generator, from the T8.3 baseline snapshots. Right
panel: the same seed through `plantgl`. Refresh the post-port goldens first if
the generator has changed —
`UPDATE_GOLDEN=1 cargo test -p apothecarys-botany --test golden` — and never
refresh the pre-port ones; they are a historical record.

## Limits

No perspective, no shadows, no transparency, no textures, no anti-aliasing, and
a painter's-algorithm depth buffer at one sample per pixel. It is a diff tool,
not a renderer.
