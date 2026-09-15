# The Apothecary's Satchel

An isometric RPG built with Rust and the [Fyrox](https://fyrox.rs/) game engine. You play as an apothecary who collects plants, breeds them using Mendelian genetics, crafts potions, and supports a procedurally-generated party through turn-based dungeon combat.

## Prerequisites

- **Rust toolchain** (stable, 1.75+): Install via [rustup](https://rustup.rs/)
- **System dependencies** (Linux):
  ```bash
  # Ubuntu/Debian
  sudo apt-get install -y libasound2-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev pkg-config

  # Fedora
  sudo dnf install alsa-lib-devel libxcb-devel libxkbcommon-devel
  ```
- **macOS**: No additional system packages needed (CoreAudio is used).
- **Windows**: No additional system packages needed.

## Project Structure

```
crates/
  core/          - Shared types: attributes, stats, items, components, config
  game/          - Fyrox game plugin, isometric camera, main binary
  navigation/    - Navmesh pathfinding, player movement, interaction system
  inventory/     - Inventory container with slot-based storage
  combat/        - Turn-based combat (stub)
  party/         - Party generation and management (stub)
  world/         - World graph and scene transitions (stub)
  dialogue/      - YarnSpinner parser and runner (stub)
  scripting/     - Lua scripting integration (stub)
  botany/        - Plant genetics, phenotype expression, L-systems, and the
                   turtle driver that feeds plantgl (MIT)
  plantgl/       - Geometry and turtle modelling, ported from PlantGL (CeCILL-C)
  garden/        - Garden plot management (stub)
  persistence/   - Save/load system (stub)
  tools/         - Editor tooling (stub)
  web-demo/      - A wasm entry point for the browser demo in `web/`
web/             - The GitLab Pages plant generator demo: a seed box, a
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

This opens a Fyrox window with the game plugin. Currently displays an empty scene (content is being developed in phases).

## The Plant Generator Demo

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
`file://` origin. `.gitlab-ci.yml` runs the same script in its `pages` job, so
what is published is what a contributor saw locally. `web/README.md` covers the
payload layout the page decodes and why the renderer is built the way it is.

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

- **Phase 1** (Complete): Workspace setup, core types, Fyrox plugin shell
- **Phase 2** (Complete): Isometric camera, navmesh pathfinding, player movement, interaction system, stat system, inventory
- **Phase 3**: World graph, scene transitions, dialogue, Lua scripting
- **Phase 4**: Party generation, combat, crafting
- **Phase 5**: Plant genetics, L-systems, garden
- **Phase 6**: Save/load, hub integration, UI
- **Phase 7**: Editor tooling
- **Phase 8**: PlantGL port — `crates/plantgl` replaces the hand-rolled plant
  mesh generation. Complete: stems are swept generalized cylinders and organs
  are real surfaces. See `docs/design/08-plantgl-port.md`.

The plant generator demo in `web/` is not a phase; it is the pipeline of Phases
5 and 8 pointed at a canvas, so a seed can be looked at without a checkout.

See `docs/design/07-task-breakdown.md` for the full task dependency graph.
