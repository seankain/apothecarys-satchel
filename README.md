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
  botany/        - Plant genetics and L-system generation (stub)
  plantgl/       - Geometry and turtle modelling, ported from PlantGL (CeCILL-C)
  garden/        - Garden plot management (stub)
  persistence/   - Save/load system (stub)
  tools/         - Editor tooling (stub)
```

## Licensing

The workspace is MIT (see `LICENSE`) **except `crates/plantgl`**, which is a
Rust translation of [openalea/plantgl](https://github.com/openalea/plantgl) and
is therefore governed by the **CeCILL-C** license — see
`crates/plantgl/LICENSE`.

| Component | License |
|---|---|
| `crates/plantgl` | CeCILL-C |
| Every other crate — `core`, `botany`, `game`, `garden`, `tools`, … | MIT |
| Root `LICENSE` | MIT |
| Shipped game binary | MIT, with a third-party notice |

CeCILL-C Article 5.3.3 permits Derivative Software under another license
provided the Article 6.4 notice of rights is carried and the port's source
stays available. `THIRD-PARTY-LICENSES` at the repository root is that notice
and **must ship with release builds**; it carries the CeCILL-C text, the
CIRAD/INRIA/INRA copyright notices, the warranty and liability notice and a
pointer to the port's source. There is no LGPL §4 analogue, so Rust's static
linking is not a problem.

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

# Refresh the OBJ golden snapshots after a deliberate change, then read the diff
UPDATE_GOLDEN=1 cargo test -p plantgl --test golden
UPDATE_GOLDEN=1 cargo test -p apothecarys-botany --test golden_pre_plantgl
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
  mesh generation. See `docs/design/08-plantgl-port.md`.

See `docs/design/07-task-breakdown.md` for the full task dependency graph.
