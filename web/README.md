# The plant generator demo

A single page that runs the game's plant pipeline in the browser: type a seed,
press **Regenerate**, and the same genetics → phenotype → L-system → turtle
chain that grows a plant in the game grows one on a canvas.

Published to GitLab Pages by the `pages` job in `.gitlab-ci.yml`.

## What is here

| File | |
|---|---|
| `index.html` | The page: canvas, seed box, the panel beside it |
| `style.css` | One dark theme, built around the colour the canvas clears to |
| `main.js` | Loads the `.wasm`, decodes its payload, fills the panel |
| `renderer.js` | A dependency-free WebGL2 view: orbit camera, two lights, a ground disc |
| `build.sh` | Assembles `public/` — the whole build |

The Rust half is [`crates/web-demo`](../crates/web-demo), which wraps
`apothecarys-botany` in a C ABI.

## Running it locally

```bash
web/build.sh
python3 -m http.server --directory public 8000
# then open http://localhost:8000
```

It has to be served over HTTP. Opening `public/index.html` as a `file://` URL
fails: ES modules and `fetch` are both blocked on that origin.

## Why there is no wasm-bindgen

`crates/web-demo` exports plain `extern "C"` functions and two byte buffers,
and `main.js` instantiates the module directly. That makes the whole build
`cargo build --target wasm32-unknown-unknown` plus five `cp`s — no
`wasm-bindgen-cli` to install in CI, no `wasm-pack`, and no generated glue to
keep in step with the Rust.

The cost is that the boundary is bytes, so the layout is written in one place
and read in two. `crates/web-demo/src/lib.rs` documents and writes it,
`decodeMesh` in `main.js` reads it in the browser, and
`crates/web-demo/tests/payload.rs` reads it natively — that last one is what
keeps the other two honest, because a decoding mistake in the browser is an
empty canvas and nothing else.

Two consequences worth knowing about:

- **The seed crosses as two `u32` halves.** A `u64` parameter is a wasm `i64`,
  which the JS API only accepts as a `BigInt`; splitting it keeps the page
  working anywhere the module runs at all. `main.js` still holds the seed as a
  `BigInt` throughout, because a `Number` silently loses the low bits past
  2<sup>53</sup> and two visibly different seeds would grow the same plant.
- **The payload is copied out of the module's memory whole, before anything is
  read from it.** Growing the wasm heap detaches every view onto the old
  buffer, and the payload is a `Vec<u8>`, so its address carries no alignment
  guarantee — `new Float32Array(buffer, offset, …)` throws unless `offset` is a
  multiple of four. One `slice()` up front settles both: the copy starts at
  zero, so every section offset in it is aligned by construction.

## Why the normals are smoothed in Rust

`PlantModel::batches` merges the scene by appearance and tessellates it, but
leaves normals to whoever draws the result. `crates/web-demo` runs
`plantgl::algo::normals::smooth_normals` at the default crease angle before
packing, which returns one normal per vertex sharing the position indices —
exactly the layout a GPU wants. Smoothing there rather than per-face in the
shader is what keeps a swept stem round while a leaf's rim stays sharp.

## Rendering notes

- **Back-face culling is off, and the fragment shader flips the normal toward
  the viewer.** A turtle draws open tubes and single-sided organ patches, so
  roughly half of what the camera sees is a back face. Culling them would eat
  the silhouette; lighting them unflipped would make every other leaf black.
- **The ambient floor is high on purpose.** A flat patch turned away from both
  lights has nothing to catch, and reads as a hole in the plant rather than as
  a leaf in shadow.
- **Batches are drawn opaque-first** so whatever sits behind a translucent
  petal has already been drawn and depth-tested by the time the petal blends
  over it.

## Licensing

The page links `crates/plantgl`, which is CeCILL-C, so it is Derivative
Software under Article 5.3.3 and the Article 6.4 notice of rights has to
travel with it. `build.sh` copies `THIRD-PARTY-LICENSES` and `LICENSE` into
`public/` on every build — for the same reason `crates/game/build.rs` puts them
next to the executable, and the footer links both.
