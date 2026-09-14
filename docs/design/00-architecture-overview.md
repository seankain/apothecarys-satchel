# Apothecary's Satchel — Architecture Overview

## Game Summary

A 3D isometric RPG where the player is an apothecary who explores dungeons with a party, collects plants with hidden genetic properties, brews potions and medicines, and manages a hub garden. Combat is turn-based and handled autonomously by party members — the player's role is support through items only.

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Performance, safety, ecosystem |
| Game Engine | Fyrox (rg3d) | Rust-native 3D engine with scene editor, animation, physics, UI |
| Asset Format | glTF / FBX | Industry standard; Fyrox supports both |
| Dialogue | YarnSpinner | `.yarn` files parsed into dialogue trees |
| Scripting | Lua (via `mlua` crate) | Mature ecosystem, lightweight, excellent Rust bindings |
| Procedural Botany | L-system engine (custom, ported from vlab/L-studio) | Generates plant meshes from genetic parameters |
| Save/Load | `serde` + MessagePack (`rmp-serde`) | Compact binary, fast, versionable |

### Why Fyrox over SDL2

- Fyrox provides a full 3D scene graph, skeletal animation, physics (rapier), UI framework, and an editor — SDL2 would require building all of these from scratch.
- Fyrox's scene editor can serve as the foundation for the map/placement tooling.
- SDL2 bindings are better suited for 2D or custom-engine projects.

### Why Lua over Squirrel

- `mlua` crate is actively maintained with async support and strong safety guarantees.
- Lua has vastly larger community, tooling, and documentation.
- Squirrel's Rust bindings are unmaintained.

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Game Application                    │
├──────────┬──────────┬───────────┬───────────┬───────────┤
│  Core    │ Gameplay │  Content  │  Tooling  │   Botany  │
│  Engine  │ Systems  │  Pipeline │  Suite    │   Engine  │
├──────────┼──────────┼───────────┼───────────┼───────────┤
│ Renderer │ Combat   │ Asset     │ Map       │ L-System  │
│ Scene    │ Party    │ Loader    │ Editor    │ Genetics  │
│ Input    │ Inventory│ Yarn      │ Animation │ Mesh Gen  │
│ Physics  │ Navigation Parser   │ Viewer    │ Phenotype  │
│ Audio    │ Save/Load│ Scripting │ Dialogue  │ Mapping   │
│ UI/HUD   │ Hub/Gard │ Lua VM   │ Tester    │           │
└──────────┴──────────┴───────────┴───────────┴───────────┘
                          │
                    ┌─────┴─────┐
                    │   Fyrox   │
                    │  Engine   │
                    └───────────┘
```

## Crate Organization

```
apothecarys-satchel/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── game/                     # Main game binary
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── app.rs            # Game plugin for Fyrox
│   │   │   ├── states/           # Game states (menu, hub, dungeon, combat)
│   │   │   └── ui/               # HUD, menus, inventory screens
│   │   └── Cargo.toml
│   │
│   ├── core/                     # Shared types, ECS components, config
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── components.rs     # All game components
│   │   │   ├── stats.rs          # DnD-style stat system
│   │   │   ├── items.rs          # Item definitions
│   │   │   └── config.rs         # Game configuration
│   │   └── Cargo.toml
│   │
│   ├── navigation/               # Pathfinding, click-to-move, WASD
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── navmesh.rs        # Navigation mesh generation
│   │   │   ├── pathfinding.rs    # A* on navmesh
│   │   │   └── input.rs          # Input → movement translation
│   │   └── Cargo.toml
│   │
│   ├── combat/                   # Turn-based combat system
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── turn_manager.rs   # Turn order, phase management
│   │   │   ├── actions.rs        # Attack, defend, skill, item use
│   │   │   ├── ai.rs             # Autonomous party member decisions
│   │   │   └── status.rs         # Buffs, debuffs, status effects
│   │   └── Cargo.toml
│   │
│   ├── party/                    # Party member generation, management
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── generation.rs     # Procedural party member creation
│   │   │   ├── permadeath.rs     # Death handling, removal
│   │   │   ├── recruitment.rs    # Hub recruitment pool
│   │   │   └── roster.rs         # Active party management
│   │   └── Cargo.toml
│   │
│   ├── inventory/                # Items, potions, crafting
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── container.rs      # Generic inventory container
│   │   │   ├── crafting.rs       # Potion/medicine recipes
│   │   │   ├── items.rs          # Item instances with genetic data
│   │   │   └── interaction.rs    # Pickup, use, give mechanics
│   │   └── Cargo.toml
│   │
│   ├── botany/                   # Plant genetics and the L-system driver (MIT)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── genetics.rs       # Genotype representation
│   │   │   ├── phenotype.rs      # Genotype → visual trait mapping
│   │   │   ├── lsystem.rs        # L-system string rewriting
│   │   │   ├── interpret.rs      # L-symbol → plantgl turtle dispatch
│   │   │   ├── surfaces.rs       # Procedural leaf/petal/fruit templates
│   │   │   ├── lod.rs            # Quality tiers and triangle budgets
│   │   │   ├── fyrox_bridge.rs   # plantgl geometry → Fyrox nodes (feature)
│   │   │   ├── mesh_gen.rs       # Re-export facade over interpret.rs
│   │   │   └── stat_mapping.rs   # Genetics → gameplay effect mapping
│   │   └── Cargo.toml
│   │
│   ├── plantgl/                  # Geometry and turtle modelling (CeCILL-C)
│   │   ├── src/                  # A port of openalea/plantgl; see
│   │   │                         # docs/design/08-plantgl-port.md
│   │   ├── LICENSE               # CeCILL-C v1 — NOT the workspace's MIT
│   │   └── Cargo.toml
│   │
│   ├── garden/                   # Hub garden management
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── plots.rs          # Garden plot state
│   │   │   ├── growth.rs         # Growth simulation
│   │   │   └── breeding.rs       # Cross-pollination, genetic mixing
│   │   └── Cargo.toml
│   │
│   ├── dialogue/                 # YarnSpinner parser and runner
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── parser.rs         # .yarn file parser
│   │   │   ├── runner.rs         # Dialogue state machine
│   │   │   └── commands.rs       # Yarn commands → game actions
│   │   └── Cargo.toml
│   │
│   ├── scripting/                # Lua scripting integration
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── vm.rs             # Lua VM lifecycle
│   │   │   ├── bindings.rs       # Rust → Lua API surface
│   │   │   └── hot_reload.rs     # Script hot-reloading
│   │   └── Cargo.toml
│   │
│   ├── persistence/              # Save/load system
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── save.rs           # Serialization
│   │   │   ├── load.rs           # Deserialization + migration
│   │   │   └── versioning.rs     # Schema versioning
│   │   └── Cargo.toml
│   │
│   ├── world/                    # Maps, locations, connections
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── location.rs       # Location definition
│   │   │   ├── map_graph.rs      # Location connectivity graph
│   │   │   ├── spawning.rs       # Entity spawn points
│   │   │   └── transitions.rs    # Scene transitions
│   │   └── Cargo.toml
│   │
│   └── tools/                    # Editor and testing tools
│       ├── src/
│       │   ├── lib.rs
│       │   ├── map_editor.rs     # Mesh placement tool
│       │   ├── connection_editor.rs # Location graph editor
│       │   ├── animation_viewer.rs  # Animation preview/test
│       │   └── dialogue_tester.rs   # Dialogue tree tester
│       └── Cargo.toml
│
├── assets/
│   ├── models/                   # .glTF / .fbx files
│   ├── textures/
│   ├── animations/               # Embedded in model files
│   ├── dialogues/                # .yarn files
│   ├── scripts/                  # .lua files
│   ├── audio/
│   └── ui/
│
├── docs/
│   └── design/                   # These design documents
│
└── data/
    ├── items.ron                  # Item definitions (RON format)
    ├── recipes.ron                # Crafting recipes
    ├── plant_genetics.ron         # Base genetic parameter ranges
    ├── party_templates.ron        # Party member generation tables
    └── locations.ron              # World graph definition
```

## Game States

```
┌────────┐    ┌─────────┐    ┌──────────┐
│  Menu  │───▶│   Hub   │◀──▶│ Dungeon  │
└────────┘    └─────────┘    └──────────┘
                  │               │
                  ▼               ▼
             ┌─────────┐    ┌──────────┐
             │ Garden  │    │ Combat   │
             └─────────┘    └──────────┘
```

- **Menu**: Title screen, load game, settings.
- **Hub**: Town center — recruitment, inventory management, shop, quest board.
- **Garden**: Subplot of hub — plant management, breeding, harvesting.
- **Dungeon**: Exploration maps — navigation, item pickup, encounters.
- **Combat**: Turn-based encounters triggered in dungeons.

## Data Flow

```
Assets (.gltf, .fbx, .yarn, .lua, .ron)
         │
         ▼
   Asset Loader (Fyrox resource manager + custom parsers)
         │
         ▼
   Scene Graph + ECS Components
         │
    ┌────┴────┐
    ▼         ▼
 Systems   Scripting (Lua)
    │         │
    └────┬────┘
         ▼
   Game State (serializable)
         │
         ▼
   Save File (MessagePack)
```

## Cross-Cutting Concerns

| Concern | Approach |
|---------|----------|
| Error handling | `anyhow` for applications, `thiserror` for libraries |
| Logging | `tracing` crate with `tracing-subscriber` |
| Configuration | RON files loaded at startup, hot-reloadable in dev |
| Testing | Unit tests per crate, integration tests in `game` crate |
| CI | `cargo clippy`, `cargo test`, `cargo fmt --check` |

## Design Document Index

| # | Document | Scope |
|---|----------|-------|
| 00 | This document | Architecture overview |
| 01 | [Engine & Rendering](01-engine-rendering.md) | Fyrox integration, camera, isometric projection |
| 02 | [World & Navigation](02-world-navigation.md) | Maps, navmesh, input, location graph |
| 03 | [Combat & Party](03-combat-party.md) | Turn system, AI, party generation, permadeath |
| 04 | [Apothecary & Botany](04-apothecary-botany.md) | Inventory, crafting, genetics, L-systems, garden |
| 05 | [Dialogue & Scripting & Persistence](05-dialogue-scripting-persistence.md) | Yarn parser, Lua VM, save/load |
| 06 | [Editor Tooling](06-editor-tooling.md) | Map editor, animation viewer, dialogue tester |
| 07 | [Task Breakdown](07-task-breakdown.md) | Concrete tasks, dependencies, ordering |
