# 06 — Editor Tooling

## Scope

Suite of development tools for map editing, mesh placement, location connectivity editing, animation preview, and dialogue testing. All tools built as separate binaries sharing game crates.

## Tool Architecture

All tools live in `crates/tools/` and compile as separate binaries. They share the same Fyrox engine and game data crates but provide specialized UIs.

```toml
# crates/tools/Cargo.toml
[[bin]]
name = "map-editor"
path = "src/bin/map_editor.rs"

[[bin]]
name = "connection-editor"
path = "src/bin/connection_editor.rs"

[[bin]]
name = "animation-viewer"
path = "src/bin/animation_viewer.rs"

[[bin]]
name = "dialogue-tester"
path = "src/bin/dialogue_tester.rs"

[[bin]]
name = "plant-previewer"
path = "src/bin/plant_previewer.rs"
```

Run with: `cargo run --bin map-editor`

---

## Map Editor

### Purpose

Place and arrange scene meshes (environment, props, spawn points, exits) within a location. Produces placement data that the game uses to populate scenes at runtime.

### Features

1. **3D Viewport**: Fyrox scene rendered with isometric camera (same as game)
2. **Asset Browser Panel**: Browse and drag meshes from `assets/models/`
3. **Scene Hierarchy Panel**: Tree view of placed objects
4. **Properties Panel**: Transform (position, rotation, scale), component data
5. **Grid Snapping**: Optional snap to configurable grid (default 0.5 units)
6. **Gizmos**: Translation, rotation, scale handles on selected objects
7. **Undo/Redo**: Command stack for all placement operations

### Placement Data Format

The editor produces a `.placement.ron` file per location:

```ron
// assets/scenes/hub_town.placement.ron
LocationPlacement(
    location_id: "hub_town",
    objects: [
        PlacedObject(
            name: "market_stall_1",
            mesh_path: "assets/models/environment/hub_buildings/market_stall.glb",
            position: (5.0, 0.0, 3.0),
            rotation: (0.0, 45.0, 0.0),    // Euler degrees
            scale: (1.0, 1.0, 1.0),
            components: [
                Interactable(interaction_type: Npc(npc_id: "merchant_bela"), display_name: "Bela's Shop"),
            ],
        ),
        PlacedObject(
            name: "spawn_from_garden",
            mesh_path: "",   // Invisible marker
            position: (5.0, 0.0, 3.0),
            rotation: (0.0, 0.0, 0.0),
            scale: (1.0, 1.0, 1.0),
            components: [
                SpawnPoint(name: "from_garden", spawn_type: PlayerArrival),
            ],
        ),
        PlacedObject(
            name: "exit_to_garden",
            mesh_path: "assets/models/environment/props/garden_gate.glb",
            position: (8.0, 0.0, 1.0),
            rotation: (0.0, 90.0, 0.0),
            scale: (1.0, 1.0, 1.0),
            components: [
                Interactable(interaction_type: Exit(exit_id: "to_garden"), display_name: "Garden"),
                ExitTrigger(target_location: "garden", arrival_spawn: "from_hub"),
            ],
        ),
    ],
)
```

### UI Layout

```
┌──────────────────────────────────────────────────────────────┐
│  File  Edit  View  Tools                                     │
├────────────┬──────────────────────────────┬──────────────────┤
│            │                              │                  │
│  Asset     │      3D Viewport             │   Properties     │
│  Browser   │                              │                  │
│            │   (Isometric view of scene   │   Transform:     │
│  models/   │    with placed objects)      │   Pos: x y z     │
│  ├─env/    │                              │   Rot: x y z     │
│  ├─props/  │                              │   Scl: x y z     │
│  └─items/  │                              │                  │
│            │                              │   Components:    │
│            │                              │   [Interactable] │
│            │                              │   [SpawnPoint]   │
│            │                              │                  │
├────────────┴──────────────────────────────┴──────────────────┤
│  Scene Hierarchy                                             │
│  ├─ terrain_ground                                           │
│  ├─ market_stall_1 (selected)                                │
│  ├─ spawn_from_garden                                        │
│  └─ exit_to_garden                                           │
└──────────────────────────────────────────────────────────────┘
```

### Key Operations

| Action | Implementation |
|--------|---------------|
| Place mesh | Drag from browser → raycast to ground → instantiate |
| Select | Click object in viewport or hierarchy |
| Move | Drag gizmo or edit position fields |
| Rotate | Rotation gizmo or input degrees |
| Delete | Delete key on selected object |
| Add component | Dropdown in properties panel |
| Save | Serialize to `.placement.ron` |
| Load | Deserialize and rebuild scene |
| Test play | Launch game in this location (F5) |

**Task Goal**: Implement the map editor as a Fyrox application with:
1. Fyrox scene rendering with the isometric camera
2. Asset browser that scans `assets/models/` recursively
3. Drag-and-drop placement with ground-plane raycasting
4. Transform gizmos (can leverage Fyrox editor gizmo code)
5. RON serialization/deserialization of placement data
6. Undo/redo via command pattern

---

## Connection Editor

### Purpose

Visually edit the world graph — which locations connect to which, with what exits and spawns.

### Features

1. **2D Graph View**: Locations as nodes, connections as edges
2. **Node properties**: Location ID, display name, scene path, type
3. **Edge properties**: Exit node name, arrival spawn name
4. **Validation**: Highlight errors (missing targets, orphan nodes)
5. **Auto-layout**: Force-directed graph layout

### UI Layout

```
┌──────────────────────────────────────────────────────┐
│  File  Edit  Validate                                 │
├──────────────────────────────────────────┬───────────┤
│                                          │           │
│     ┌──────────┐      ┌──────────┐      │ Selected: │
│     │ Hub Town │──────│ Garden   │      │           │
│     └─────┬────┘      └──────────┘      │ Location: │
│           │                              │ hub_town  │
│     ┌─────┴────┐                        │           │
│     │Dungeon F1│                        │ Exits:    │
│     └─────┬────┘                        │ → garden  │
│           │                              │ → dungeon │
│     ┌─────┴────┐                        │           │
│     │Dungeon F2│                        │ Spawns:   │
│     └──────────┘                        │ from_gard │
│                                          │ from_dung │
│     (drag to pan, scroll to zoom)       │           │
├──────────────────────────────────────────┴───────────┤
│  Validation: ✓ All connections valid                  │
└──────────────────────────────────────────────────────┘
```

### Data Format

Reads/writes `data/locations.ron` (same format as the game's `WorldGraph`).

**Task Goal**: Implement the connection editor with:
1. 2D canvas rendering of location nodes and connection edges
2. Drag-to-create connections between nodes
3. Property panel for editing location and exit details
4. Validation pass checking all references
5. RON file I/O

---

## Animation Viewer

### Purpose

Load character/creature meshes and preview their embedded animations. Useful for verifying animation clips, checking transitions, and tuning ABSM parameters.

### Features

1. **Model loader**: Open any `.glb` / `.fbx` file
2. **Animation list**: Show all animation clips in the file
3. **Playback controls**: Play, pause, stop, scrub timeline, speed control
4. **Loop toggle**: One-shot or looping playback
5. **Bone visualization**: Optional skeleton overlay
6. **ABSM preview**: Load and test animation blend state machines
7. **Camera orbit**: Free orbit camera around the model

### UI Layout

```
┌──────────────────────────────────────────────────────┐
│  File  View                                           │
├──────────┬───────────────────────────────────────────┤
│          │                                           │
│ Clips:   │       3D Viewport                         │
│          │                                           │
│ • idle   │    (Character model with                  │
│ • walk   │     current animation playing)            │
│ • run    │                                           │
│ • attack │                                           │
│ • hit    │                                           │
│ • death  │                                           │
│          │                                           │
├──────────┴───────────────────────────────────────────┤
│  ◀◀  ▶  ◼  ▶▶  │ Speed: [1.0x] │ Loop: [✓]         │
│  ─────────●──────────────────── 0:02.4 / 0:03.0     │
└──────────────────────────────────────────────────────┘
```

**Task Goal**: Implement the animation viewer with:
1. Model loading via Fyrox resource manager
2. Animation clip enumeration from loaded model
3. Playback control (play, pause, scrub, speed)
4. Orbit camera for inspection
5. Optional skeleton/bone debug rendering

---

## Dialogue Tester

### Purpose

Load and interactively test YarnSpinner dialogue files without running the full game.

### Features

1. **File loader**: Open `.yarn` files
2. **Node browser**: List all dialogue nodes, jump to any
3. **Interactive playback**: Show dialogue lines, present choices
4. **Variable inspector**: View/edit Yarn variables in real-time
5. **Command log**: Show executed commands (without game side effects)
6. **Syntax highlighting**: Yarn markup with colors

### UI Layout

```
┌──────────────────────────────────────────────────────┐
│  File  View                                           │
├──────────┬───────────────────────────────┬───────────┤
│          │                               │           │
│ Nodes:   │  Herbalist: Welcome to my     │ Variables │
│          │  shop, apothecary.            │           │
│ • Start  │                               │ $visited: │
│ • Greet  │  What brings you here today?  │   false   │
│ • Shop   │                               │ $gold:    │
│ • Quest  │  [1] I need rare seeds.       │   50      │
│          │  [2] Just browsing.           │ $has_moon │
│          │  [3] I found this moonpetal.  │   true    │
│          │      (greyed - needs item)    │           │
│          │                               │           │
├──────────┴───────────────────────────────┴───────────┤
│  Command Log:                                         │
│  > set $visited_herbalist = true                      │
│  > jump Herbalist_SeedShop                            │
└──────────────────────────────────────────────────────┘
```

**Task Goal**: Implement the dialogue tester with:
1. Yarn file loading via the `dialogue` crate parser
2. Step-through dialogue execution using `DialogueRunner`
3. Choice selection UI
4. Live variable editing panel
5. Command execution log (commands are logged but not dispatched to game systems)

---

## Plant Previewer

### Purpose

Preview procedurally generated plants by adjusting genetic parameters and viewing the resulting L-system mesh in real-time.

### Features

1. **Gene sliders**: Adjust each gene value (allele_a, allele_b) with sliders
2. **Live preview**: 3D viewport showing the generated plant mesh
3. **Phenotype readout**: Display the derived phenotype parameters
4. **Alchemy readout**: Show what effects this plant's genetics would produce
5. **Export**: Save genotype as RON data, or export mesh as glTF
6. **Breeding sim**: Load two parent genotypes, preview offspring range
7. **Seed control**: Set RNG seed for reproducible generation

### UI Layout

```
┌──────────────────────────────────────────────────────┐
│  File  View  Tools                                    │
├─────────────────┬────────────────────┬───────────────┤
│ Gene Editor     │                    │ Phenotype     │
│                 │   3D Viewport      │               │
│ Stem Height:    │                    │ Height: 1.4m  │
│ A:[===●==] 0.6  │   (Generated       │ Branches: 3   │
│ B:[==●===] 0.4  │    plant mesh      │ Leaf: broad   │
│                 │    rotating)       │ Flowers: yes  │
│ Leaf Size:      │                    │ Petals: 5     │
│ A:[====●=] 0.8  │                    │ Color: violet │
│ B:[=●====] 0.2  │                    │               │
│                 │                    │ Alchemy       │
│ Petal Color:    │                    │ Heal: 12      │
│ A:[●=====] 0.1  │                    │ STR +2 (3t)   │
│ B:[===●==] 0.6  │                    │ Toxicity: low │
│                 │                    │               │
│ [Randomize]     │                    │ [Export .ron] │
│ [Breed ...]     │                    │ [Export .glb] │
└─────────────────┴────────────────────┴───────────────┘
```

**Task Goal**: Implement the plant previewer with:
1. Slider UI for all gene parameters
2. Real-time L-system evaluation and mesh generation on parameter change
3. Phenotype and alchemy effect display panels
4. Orbit camera for 3D inspection
5. RON export of genotype data

---

## Shared Tool Infrastructure

### Common UI Widgets

Build reusable widgets used across all tools:

```rust
// crates/tools/src/widgets/
mod file_browser;      // Open/save file dialogs
mod property_grid;     // Key-value property editor
mod tree_view;         // Hierarchical tree widget
mod slider;            // Labeled value slider
mod viewport_3d;       // Fyrox 3D viewport with camera controls
mod viewport_2d;       // 2D canvas with pan/zoom
```

### Tool Configuration

Each tool reads from `tools.ron` for shared settings:

```ron
// tools.ron
ToolConfig(
    assets_root: "assets/",
    data_root: "data/",
    default_scene: "assets/scenes/hub_town.gltf",
    grid_size: 0.5,
    recent_files: [],
)
```

## Key Implementation Files

| File | Purpose |
|------|---------|
| `crates/tools/src/bin/map_editor.rs` | Map editor binary entry point |
| `crates/tools/src/bin/connection_editor.rs` | Connection editor binary |
| `crates/tools/src/bin/animation_viewer.rs` | Animation viewer binary |
| `crates/tools/src/bin/dialogue_tester.rs` | Dialogue tester binary |
| `crates/tools/src/bin/plant_previewer.rs` | Plant previewer binary |
| `crates/tools/src/map_editor.rs` | Map editor logic |
| `crates/tools/src/connection_editor.rs` | Connection editor logic |
| `crates/tools/src/animation_viewer.rs` | Animation viewer logic |
| `crates/tools/src/dialogue_tester.rs` | Dialogue tester logic |
| `crates/tools/src/widgets/` | Shared UI widget library |
