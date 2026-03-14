# 02 — World & Navigation

## Scope

Map/location system, location connectivity graph, navigation mesh, player movement (click-to-move and WASD), scene transitions, item pickup, and spawn points.

## World Graph

### Location Model

The game world is a graph of **locations**. Each location is a Fyrox scene file (`.rgs` or loaded from `.gltf` + placement data).

```rust
// crates/world/src/location.rs

/// Unique identifier for a location
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocationId(pub String);

/// A single game location (map)
#[derive(Debug, Serialize, Deserialize)]
pub struct LocationDef {
    pub id: LocationId,
    pub display_name: String,
    pub scene_path: String,           // Path to scene asset
    pub location_type: LocationType,
    pub spawn_points: Vec<SpawnPoint>,
    pub exits: Vec<ExitDef>,          // Connections to other locations
}

#[derive(Debug, Serialize, Deserialize)]
pub enum LocationType {
    Hub,
    Garden,
    Dungeon { floor: u32, difficulty: u32 },
    Town,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExitDef {
    pub target_location: LocationId,
    pub exit_node_name: String,       // Name of exit trigger node in scene
    pub arrival_spawn: String,        // Spawn point name at target location
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnPoint {
    pub name: String,
    pub position: Vector3<f32>,
    pub rotation: f32,                // Y-axis rotation in radians
    pub spawn_type: SpawnType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SpawnType {
    PlayerArrival,                    // Where player appears when entering
    Enemy { template: String },       // Enemy spawn
    Item { item_id: String },         // Item pickup location
    Npc { npc_id: String },           // NPC placement
}
```

### Location Graph

```rust
// crates/world/src/map_graph.rs

/// The world graph loaded from data/locations.ron
pub struct WorldGraph {
    locations: HashMap<LocationId, LocationDef>,
}

impl WorldGraph {
    pub fn load(path: &str) -> Result<Self>;
    pub fn get_location(&self, id: &LocationId) -> Option<&LocationDef>;
    pub fn get_exits(&self, id: &LocationId) -> &[ExitDef];
    pub fn get_connected(&self, id: &LocationId) -> Vec<&LocationId>;
}
```

**Task Goal**: Implement `WorldGraph` that loads from a RON file. Provide validation (no dangling exit references, every exit has a matching spawn point at the target).

### Example RON Data

```ron
// data/locations.ron
[
    LocationDef(
        id: "hub_town",
        display_name: "Willowmere",
        scene_path: "assets/scenes/hub_town.gltf",
        location_type: Hub,
        spawn_points: [
            SpawnPoint(name: "from_garden", position: (5.0, 0.0, 3.0), rotation: 0.0, spawn_type: PlayerArrival),
            SpawnPoint(name: "from_dungeon_1", position: (12.0, 0.0, 8.0), rotation: 3.14, spawn_type: PlayerArrival),
        ],
        exits: [
            ExitDef(target_location: "garden", exit_node_name: "exit_to_garden", arrival_spawn: "from_hub"),
            ExitDef(target_location: "dungeon_floor_1", exit_node_name: "exit_to_dungeon", arrival_spawn: "entrance"),
        ],
    ),
    // ...
]
```

## Scene Transitions

```
Player enters exit trigger
         │
         ▼
Save transient state (party HP, inventory)
         │
         ▼
Fade-out (0.5s black overlay)
         │
         ▼
Unload current scene
         │
         ▼
Load target scene (async)
         │
         ▼
Place player at arrival spawn point
         │
         ▼
Fade-in (0.5s)
         │
         ▼
Resume gameplay
```

**Task Goal**: Implement `SceneTransitionManager` in `crates/world/src/transitions.rs`:
1. Detect player collision with exit trigger nodes
2. Initiate async scene load while showing loading/fade overlay
3. Transfer player + party state across scenes
4. Place entities at the correct spawn points

## Navigation Mesh

### Generation

Each location scene includes a navigation mesh that defines walkable surfaces. Two approaches:

**Option A (Recommended)**: Author navmeshes in the scene editor or 3D tool, export as part of the scene.

**Option B**: Runtime navmesh generation from scene geometry using Fyrox's built-in navmesh support or the `recast` crate.

```rust
// crates/navigation/src/navmesh.rs

pub struct NavigationSystem {
    navmesh: NavMesh,              // Fyrox NavMesh node from scene
}

impl NavigationSystem {
    /// Load navmesh from scene node named "navmesh"
    pub fn from_scene(scene: &Scene) -> Self;

    /// Find path from start to end on the navmesh
    pub fn find_path(&self, start: Vector3<f32>, end: Vector3<f32>) -> Option<Vec<Vector3<f32>>>;

    /// Test if a point is on walkable ground
    pub fn is_walkable(&self, point: Vector3<f32>) -> bool;
}
```

**Task Goal**: Implement navmesh loading from Fyrox scene nodes and A* pathfinding over the navmesh polygons.

### Path Smoothing

Raw A* paths are jagged. Apply string-pulling (funnel algorithm) to produce smooth paths:

```
Raw A* waypoints:  P1 ──── P2 ──── P3 ──── P4
                        \              /
Smoothed:          P1 ────────── P3' ────── P4
```

## Player Movement

### Dual Input System

The player can move via **click-to-move** OR **WASD keys**. Both feed into the same movement pipeline.

```rust
// crates/navigation/src/input.rs

pub enum MovementCommand {
    /// Click-to-move: pathfind to target and follow path
    NavigateTo(Vector3<f32>),
    /// WASD: direct velocity in camera-relative direction
    DirectMove(Vector2<f32>),  // normalized direction
    /// Stop all movement
    Stop,
}
```

### Click-to-Move Flow

```
Mouse click on ground
         │
         ▼
Raycast through ortho camera → world position
         │
         ▼
Check navmesh.is_walkable(hit_point)
         │
    ┌────┴────┐
    │ Yes     │ No → ignore click
    ▼
find_path(player_pos, hit_point)
    │
    ▼
Store path in PlayerMovement component
    │
    ▼
Each frame: move player toward next waypoint
    │         speed: 4.0 units/sec
    ▼
Reached waypoint? → pop, continue to next
    │
    ▼
Path empty? → stop, idle animation
```

### WASD Movement Flow

```
WASD key held
    │
    ▼
Compute camera-relative direction
(W = camera forward projected onto ground plane,
 A = camera left, etc.)
    │
    ▼
Apply velocity directly: pos += dir * speed * dt
    │
    ▼
Clamp to navmesh (slide along edges if hitting boundary)
    │
    ▼
Key released → decelerate to stop
```

**Task Goal**: Implement `PlayerMovementSystem` in `crates/navigation/src/input.rs`:
1. Handle both `NavigateTo` and `DirectMove` commands
2. WASD produces camera-relative movement directions
3. Click-to-move triggers pathfinding and path-following
4. Either input mode cancels the other (clicking while WASD-ing overrides, and vice versa)
5. Feed movement state to animation system (idle/walk/run)

### Camera-Relative Direction Calculation

Since the camera is at a fixed isometric angle, the WASD directions map to consistent world directions:

```
Camera faces (-1, -1.4, -1) normalized
Camera right = cross(forward, up) projected to XZ plane

W → forward_xz (into the screen, toward top-right of isometric view)
S → -forward_xz
A → -right_xz
D → right_xz
```

## Item Interaction

### Interactable Components

```rust
// crates/core/src/components.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interactable {
    pub interaction_type: InteractionType,
    pub display_name: String,
    pub highlight_material: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InteractionType {
    Pickup { item_id: String },
    Examine { dialogue_node: String },
    Exit { exit_id: String },
    Npc { npc_id: String },
    GardenPlot { plot_index: usize },
}
```

### Interaction Flow

```
Mouse moves over scene
         │
         ▼
Raycast → hit node
         │
         ▼
Node has Interactable component?
    │           │
   Yes          No → clear any highlight
    │
    ▼
Apply highlight material
Show tooltip (display_name)
         │
         ▼
Player clicks while highlighted?
    │
    ▼
Player within interaction range (2.0 units)?
    │           │
   Yes          No → navigate to object first, then interact
    │
    ▼
Execute interaction:
  Pickup → add to inventory, remove from scene
  Examine → start dialogue
  Exit → trigger scene transition
  Npc → open NPC dialogue/recruitment
  GardenPlot → open garden UI
```

**Task Goal**: Implement the full interaction pipeline in `crates/navigation/src/interaction.rs`:
1. Per-frame raycast from mouse position
2. Component query on hit node
3. Highlight toggle
4. Click handling with range check
5. Dispatch to appropriate system (inventory, dialogue, transition, etc.)

## Key Implementation Files

| File | Purpose |
|------|---------|
| `crates/world/src/location.rs` | Location data structures |
| `crates/world/src/map_graph.rs` | World connectivity graph |
| `crates/world/src/transitions.rs` | Scene loading and transitions |
| `crates/world/src/spawning.rs` | Entity placement at spawn points |
| `crates/navigation/src/navmesh.rs` | Navmesh loading and pathfinding |
| `crates/navigation/src/input.rs` | WASD + click-to-move input handling |
| `crates/navigation/src/interaction.rs` | Mouse hover, highlight, click interaction |
| `data/locations.ron` | World graph definition |
