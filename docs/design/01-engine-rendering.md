# 01 — Engine & Rendering

## Scope

Fyrox engine integration, isometric camera setup, asset pipeline, animation system, and rendering configuration.

## Fyrox Integration

### Game Plugin

The game registers as a Fyrox `Plugin`. All game systems run inside the plugin's `update` loop.

```rust
// crates/game/src/app.rs
use fyrox::plugin::{Plugin, PluginContext};

pub struct GamePlugin {
    state: GameState,
}

impl Plugin for GamePlugin {
    fn update(&mut self, ctx: &mut PluginContext) {
        // Dispatch to current game state (Menu, Hub, Dungeon, Combat)
        self.state.update(ctx);
    }
}
```

**Task Goal**: Implement `GamePlugin` with state enum dispatching to state-specific update/render logic. Fyrox handles the main loop, window, and rendering pipeline.

### Dependencies

```toml
[dependencies]
fyrox = "0.35"  # Pin to specific version; check latest stable
```

## Isometric Camera

### Configuration

The game uses a **dimetric projection** (common "isometric" in games):
- Camera rotation: 45° around Y-axis, 30° downward pitch
- Projection: **Orthographic** (not perspective) for true isometric feel
- Zoom: Adjustable orthographic size, default covers ~20 game units width

```rust
// Camera setup pseudocode
fn create_isometric_camera(scene: &mut Scene) -> Handle<Node> {
    let camera = CameraBuilder::new(
        BaseBuilder::new()
            .with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(10.0, 14.0, 10.0))
                    .with_local_rotation(
                        // Look-at target produces the isometric angle
                        UnitQuaternion::face_towards(
                            &Vector3::new(-1.0, -1.4, -1.0).normalize(),
                            &Vector3::UP,
                        )
                    )
                    .build()
            )
    )
    .with_projection(Projection::Orthographic(OrthographicProjection {
        z_near: 0.1,
        z_far: 512.0,
        vertical_size: 10.0, // Half-height in world units
    }))
    .build(&mut scene.graph);

    camera
}
```

**Task Goal**: Create a camera system in `crates/game/src/camera.rs` that:
1. Sets up the orthographic isometric view
2. Follows the player character with smooth interpolation (`lerp`)
3. Supports zoom in/out (scroll wheel adjusts `vertical_size`)
4. Clamps to map bounds

### Camera Follow

```
Player moves → Camera target updates → Camera position lerps toward target
                                        (rate: 5.0 * dt for smooth follow)
```

## Asset Pipeline

### Supported Formats

| Asset Type | Format | Loader |
|-----------|--------|--------|
| Scene meshes | `.gltf` / `.glb` | Fyrox built-in |
| Character models | `.fbx` / `.gltf` | Fyrox built-in |
| Animations | Embedded in `.fbx` / `.gltf` | Fyrox animation system |
| Textures | `.png` / `.jpg` | Fyrox built-in |
| Audio | `.ogg` / `.wav` | Fyrox built-in |

### Asset Directory Structure

```
assets/
├── models/
│   ├── characters/
│   │   ├── player.glb          # Player character mesh + rig
│   │   ├── warrior_base.glb    # Party member archetype
│   │   ├── mage_base.glb
│   │   └── ...
│   ├── environment/
│   │   ├── dungeon_tiles/      # Modular dungeon pieces
│   │   ├── hub_buildings/      # Hub location meshes
│   │   └── props/              # Interactable objects, furniture
│   ├── items/
│   │   ├── potions/
│   │   ├── ingredients/
│   │   └── equipment/
│   └── plants/                 # Base plant part meshes (leaves, stems, petals)
│       ├── leaf_templates/
│       ├── stem_segments/
│       ├── flower_parts/
│       └── fruit_parts/
├── textures/
│   ├── characters/
│   ├── environment/
│   └── ui/
└── audio/
    ├── music/
    ├── sfx/
    └── ambient/
```

### Asset Loading Strategy

1. **Eager loading**: UI assets, player model, hub scene — loaded at startup
2. **Lazy loading**: Dungeon scenes, enemy models — loaded on scene transition
3. **Async**: Use Fyrox's async resource manager to avoid frame drops
4. **Caching**: Fyrox caches loaded resources by path; reuse handles

**Task Goal**: Create `crates/game/src/asset_manifest.rs` defining asset path constants and a preload manifest for each game state.

## Animation System

### Architecture

Fyrox provides `AnimationPlayer` and `AnimationBlendingStateMachine` (ABSM) nodes. Each animated entity gets:

1. An `AnimationPlayer` loaded from the model file
2. An ABSM defining transitions between animation states

### Character Animation States

```
        ┌────────┐
        │  Idle  │◀──────────────────┐
        └───┬────┘                   │
            │ movement input         │ velocity ≈ 0
            ▼                        │
        ┌────────┐              ┌────┴───┐
        │  Walk  │─────────────▶│  Run   │
        └───┬────┘  speed > th  └────────┘
            │
            │ interaction trigger
            ▼
        ┌──────────┐
        │ Interact │──▶ Idle
        └──────────┘

Combat-specific states:
  Idle_Combat ◀──▶ Attack ──▶ Hit ──▶ Idle_Combat
                               │
                               ▼
                             Death
```

**Task Goal**: Define ABSM configurations for player and party member archetypes. Animations are embedded in model files — the ABSM references animation clips by name within the loaded model.

### Animation Events

Fyrox supports animation signals (events at specific keyframes). Use these for:
- Footstep sounds (at foot-down frames)
- Attack hit detection (at impact frames)
- VFX triggers (potion splash, spell cast)

## Rendering Configuration

### Lighting

- **Directional light**: Simulates sun, casts shadows from isometric angle
- **Ambient**: Moderate ambient to avoid harsh shadows in dungeon interiors
- **Point lights**: Torches, magic effects in dungeons
- Shadow quality: Medium by default, configurable

### Post-Processing

Keep minimal for performance:
- SSAO (subtle, for depth)
- Optional bloom (for magic effects)
- No motion blur (isometric games look cleaner without it)

### Performance Targets

- 60 FPS on mid-range hardware
- Draw call batching via Fyrox's built-in instancing
- LOD not critical (isometric camera = consistent distance)

## Interactable Object Highlighting

When the mouse hovers over an interactable object:
1. Raycast from mouse position through orthographic camera into scene
2. Check if hit node has `Interactable` component
3. If yes, apply outline/glow effect

```rust
// Highlight approach: Fyrox material override
fn highlight_interactable(node: Handle<Node>, graph: &mut Graph, highlight: bool) {
    // Option A: Swap to highlight material (adds emission/outline)
    // Option B: Use stencil-based outline post-process
    // Recommended: Material swap — simpler, predictable in orthographic view
}
```

**Task Goal**: Implement mouse-hover raycasting and material-swap highlighting in `crates/navigation/src/interaction.rs`. The system needs:
1. Orthographic ray from screen coords
2. Scene graph query for `Interactable` component
3. Material parameter toggle for highlight (emission color boost)

## Key Implementation Files

| File | Purpose |
|------|---------|
| `crates/game/src/app.rs` | Fyrox plugin, game state machine |
| `crates/game/src/camera.rs` | Isometric camera setup and follow |
| `crates/game/src/asset_manifest.rs` | Asset paths and preload lists |
| `crates/game/src/states/mod.rs` | Game state enum and transitions |
| `crates/navigation/src/interaction.rs` | Mouse hover raycast + highlight |
