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
  garden/        - Garden plot management (stub)
  persistence/   - Save/load system (stub)
  tools/         - Editor tooling (stub)
```

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

See `docs/design/07-task-breakdown.md` for the full task dependency graph.
