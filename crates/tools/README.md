# Plant Previewer Tool

A utility for previewing procedurally generated plants from the Apothecary's Satchel botany system. Generates a plant from a seed value and displays it in a 3D viewer or exports it as a Wavefront OBJ file.

## Building

The 3D viewer requires the Fyrox engine and system audio libraries (`libasound2-dev` on Ubuntu/Debian):

```bash
# Install system dependencies (Ubuntu/Debian)
sudo apt-get install libasound2-dev

# Build the viewer binary
cargo build -p apothecarys-tools --features viewer
```

To build only the library (no Fyrox dependency):

```bash
cargo build -p apothecarys-tools --no-default-features
```

## Usage

### 3D Viewer

Run the plant previewer with an optional seed value:

```bash
# Random seed (based on system time)
cargo run --bin plant_previewer --features viewer

# Specific seed for reproducible results
cargo run --bin plant_previewer --features viewer -- 42
```

The viewer opens a window with:
- An orthographic camera looking at the plant from an elevated angle
- Brown stem geometry built from generalized cylinders
- Colored cube markers for leaves, flowers, and fruit
- A green ground plane

### Console Output

On startup, the tool prints a summary of the generated plant:

```
=== Plant Preview (seed: 42) ===

--- Phenotype ---
  Branch angle:    32.5°
  Branch length:   1.20
  Branch thickness:0.080
  Complexity:      4 iterations
  Branching factor:3
  ...

--- Mesh Statistics ---
  Stem segments:   47
  Vertices:        564
  Triangles:       564
  Leaf instances:  12
  Flower instances:3
  Fruit instances: 0

--- Alchemy Effects ---
  Heal: 15 HP
  Buff: Haste for 3 turns
```

### OBJ Export

The viewer automatically exports a Wavefront OBJ file on startup:

```
plant_seed_42.obj
```

This file can be opened in any 3D modeling application (Blender, MeshLab, etc.) and contains:
- **`stems`** group — triangulated cylinder geometry for branches
- **`leaves`** group — triangle markers at leaf positions
- **`flowers`** group — triangle markers at flower positions
- **`fruit`** group — triangle markers at fruit positions

To generate an OBJ file without the viewer, use the library API:

```rust
use apothecarys_tools::plant_preview::PlantPreviewData;

let preview = PlantPreviewData::from_seed(42);
let obj_string = preview.mesh.to_obj("my_plant.mtl");
let mtl_string = preview.mesh.to_mtl();
std::fs::write("my_plant.obj", obj_string).unwrap();
std::fs::write("my_plant.mtl", mtl_string).unwrap();
```

## Library API

The `PlantPreviewData` struct provides programmatic access to all generated plant data:

```rust
use apothecarys_tools::plant_preview::PlantPreviewData;

let preview = PlantPreviewData::from_seed(42);

// Access plant properties
println!("Vertices: {}", preview.mesh.vertex_count());
println!("Phenotype: {:?}", preview.phenotype);
println!("Effects: {:?}", preview.alchemy_effects);

// Print a formatted summary
preview.print_summary();
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `seed` | `u64` | The seed used for generation |
| `genotype` | `PlantGenotype` | Diploid genetic data (24 gene loci) |
| `phenotype` | `PlantPhenotype` | Expressed visual traits |
| `mesh` | `PlantMeshData` | Vertices, indices, and organ instances |
| `alchemy_effects` | `Vec<AlchemyEffect>` | Potion effects derived from genetics |

## Determinism

The same seed always produces the same plant. This is useful for:
- Reproducing interesting specimens
- Sharing plants by seed number
- Testing and debugging the generation pipeline
