# The Apothecary's Satchel

An isometric RPG built with Rust and the [Fyrox](https://fyrox.rs/) game engine. You play as an apothecary who collects plants, breeds them using Mendelian genetics, crafts potions, and supports a procedurally-generated party through turn-based dungeon combat.

The plant generator runs in a browser — type a seed and watch the game's own
genetics and L-system pipeline grow a plant:
**<https://seankain.github.io/apothecarys-satchel/>**

## Prerequisites

- **Rust toolchain** (stable, 1.75+): Install via [rustup](https://rustup.rs/)
- **System dependencies**: none. The root `Cargo.toml` patches `alsa-sys`
  with `crates/alsa-sys-stub`, so no ALSA headers are needed and a bare
  `ubuntu-latest` builds the workspace. The cost is that **no build has
  audio** — delete that `[patch]` section and install `libasound2-dev`
  (Debian/Ubuntu) or `alsa-lib-devel` (Fedora) to get sound back.

## Project Structure

```
crates/
  core/          - Shared types: attributes, stats, items, components, config
  game/          - Fyrox game plugin, isometric camera, main binary
  navigation/    - Navmesh pathfinding, player movement, interaction system
  inventory/     - Inventory container with slot-based storage
  combat/        - Turn-based combat (library-only)
  party/         - Party generation and management (library-only)
  world/         - World graph and scene transitions (library-only)
  dialogue/      - YarnSpinner parser and runner (library-only)
  scripting/     - Lua scripting integration (library-only)
  botany/        - Plant genetics, phenotype expression, L-systems, and the
                   turtle driver that feeds plantgl (MIT)
  plantgl/       - Geometry and turtle modelling, ported from PlantGL (CeCILL-C)
  garden/        - Garden plot management (library-only)
  persistence/   - Save/load system (library-only)
  tools/         - Editor tooling: five Fyrox GUI binaries
  mcp-server/    - MCP server exposing scene editing to an agent
  web-demo/      - A wasm entry point for the browser demo in `web/`
web/             - The GitHub Pages plant generator demo: a seed box, a
                   regenerate button and the plant it grows
```

Plant generation runs `botany` → `plantgl`:

```
PlantGenotype → PlantPhenotype → LSystem → [LSymbol] → plantgl::Scene → Fyrox
 genetics.rs     phenotype.rs    lsystem.rs        interpret.rs    fyrox_bridge.rs
```

`botany/src/fyrox_bridge.rs` is behind a `fyrox` feature, off by default, so
the headless crates that depend on `botany` never pull the engine in.
`docs/design/08-plantgl-port.md` covers the port; `docs/design/04-apothecary-botany.md`
covers the plant pipeline.

## Licensing

The workspace is MIT (see `LICENSE`) **except `crates/plantgl`**, which is a
Rust translation of [openalea/plantgl](https://github.com/openalea/plantgl) and
is therefore governed by the **CeCILL-C** license — see
`crates/plantgl/LICENSE`.

| Component | License |
|---|---|
| `crates/plantgl` | CeCILL-C |
| Every other crate — `core`, `botany`, `game`, `garden`, `tools`, `web-demo`, … | MIT |
| Root `LICENSE` | MIT |
| Shipped game binary | MIT, with a third-party notice |

CeCILL-C Article 5.3.3 permits Derivative Software under another license
provided the Article 6.4 notice of rights is carried and the port's source
stays available. `THIRD-PARTY-LICENSES` at the repository root is that notice;
it carries the CeCILL-C text, the CIRAD/INRIA/INRA copyright notices, the
warranty and liability notice and a pointer to the port's source. There is no
LGPL §4 analogue, so Rust's static linking is not a problem.

It **ships with every build**, not only at release time: `crates/game/build.rs`
copies it and the root `LICENSE` next to the executable,
`crates/game/tests/licensing.rs` fails if either is missing or has drifted from
the repository copy, and `web/build.sh` copies both into the published page.
So after `cargo build --release -p apothecarys-game`:

```
target/release/
  game
  LICENSE
  THIRD-PARTY-LICENSES
```

A packaging step that only runs at release time is a step that is discovered to
be missing at release time; `cargo test` catches this one instead.

Work using PlantGL is asked to cite:

> Pradal C., Boudon F., Nouguier C., Chopard J., Godin C. 2009. PlantGL: A
> python-based geometric library for 3D plant modelling at different scales.
> *Graphical Models*, 71: 1–21.

> Not legal advice. Worth a lawyer's eye before commercial distribution.

## Building

```bash
# Build all crates
cargo build --workspace

# Build just the game binary
cargo build -p apothecarys-game
```

## Running the Game

```bash
cargo run --bin game
```

This opens a Fyrox window showing the main menu. **Start Game** enters the hub
blockout — a ground plane, a directional light and the isometric camera — and
that is as far as the binary currently goes. Everything else listed under
*Project Structure* is reachable from its own tests and, for the botany chain,
from the previewer and the web demo, but is not yet wired into the plugin.
`docs/design/09-implementation-status.md` records exactly what is and is not
connected.

## The Plant Generator Demo

**<https://seankain.github.io/apothecarys-satchel/>** — no checkout needed.

`web/` is a single page that runs the plant pipeline in the browser: type a
seed, press **Regenerate**, and the same chain the game runs grows a plant on
a canvas. `crates/web-demo` is `botany` behind a C ABI compiled to
`wasm32-unknown-unknown`; `web/` is the page that instantiates it. There is no
wasm-bindgen and no bundler, so the whole build is one `cargo build` and a
handful of copies:

```bash
web/build.sh
python3 -m http.server --directory public 8000   # then open localhost:8000
```

It has to be served over HTTP — ES modules and `fetch` are both blocked on a
`file://` origin. `.github/workflows/pages.yml` runs the same script on every
push to `main`, so what is published is what a contributor saw locally.
`web/README.md` covers the payload layout the page decodes and why the renderer
is built the way it is.

## Running Tests

```bash
# Run all tests across the workspace
cargo test --workspace

# Run tests for a specific crate
cargo test -p apothecarys-core
cargo test -p apothecarys-navigation
cargo test -p apothecarys-inventory
cargo test -p apothecarys-game
cargo test -p plantgl
cargo test -p apothecarys-web-demo

# Refresh the OBJ golden snapshots after a deliberate change, then read the diff
UPDATE_GOLDEN=1 cargo test -p plantgl --test golden
UPDATE_GOLDEN=1 cargo test -p apothecarys-botany --test golden
```

`crates/botany/tests/golden/pre-plantgl/` holds the snapshots taken *before*
the PlantGL port replaced the plant generator. They are kept, not asserted, so
the change in geometry stays reviewable; do not update them.

The per-plant performance budget is enforced by a test, and its timings only
mean anything in a release build:

```bash
cargo test --release -p apothecarys-botany --test budget
```

## Linting

```bash
# Run clippy on all crates (treating warnings as errors)
cargo clippy --workspace -- -D warnings

# Run clippy including test code
cargo clippy --workspace --tests -- -D warnings
```

## Development Phases

The game is developed in incremental phases:

"Complete" below means the code is written, tested and reachable from the
game. "Library complete" means it is written and tested but nothing in
`cargo run --bin game` calls it — the bulk of what is left is that wiring, not
new systems.

- **Phase 1** (Complete): Workspace setup, core types, Fyrox plugin shell
- **Phase 2** (Camera complete; navigation library complete): Isometric
  camera, navmesh pathfinding, player movement, interaction system, stat
  system, inventory
- **Phase 3** (Library complete): World graph, scene transitions, dialogue,
  Lua scripting
- **Phase 4** (Library complete): Party generation, combat, crafting
- **Phase 5** (Complete): Plant genetics, L-systems, garden
- **Phase 6** (Library complete; UI is model-only): Save/load, hub
  integration, UI
- **Phase 7** (Complete): Editor tooling — map editor, connection editor,
  animation viewer, dialogue tester, plant previewer
- **Phase 8** (Complete through Phase E): PlantGL port — `crates/plantgl`
  replaces the hand-rolled plant mesh generation. Stems are swept generalized
  cylinders and organs are real surfaces. See
  `docs/design/08-plantgl-port.md`; Phase F is optional and unstarted.

The plant generator demo in `web/` is not a phase; it is the pipeline of Phases
5 and 8 pointed at a canvas, so a seed can be looked at without a checkout.

See `docs/design/07-task-breakdown.md` for the full task dependency graph, and
`docs/design/09-implementation-status.md` for a verified per-system status and
the outstanding work.
