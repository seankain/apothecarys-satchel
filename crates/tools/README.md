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

# At a level-of-detail tier: hub (default), distant, or icon
cargo run --bin plant_previewer --features viewer -- 42 distant
```

The viewer opens a window with:
- An orthographic camera framed on the plant's own bounding box
- Stems swept as generalized cylinders — one continuous, tapering surface per
  branch axis, not a ring pair per internode
- Real leaf, petal and fruit surfaces, each in its own material
- A green ground plane

Geometry is built by `crates/plantgl` and handed to Fyrox through
`apothecarys_botany::fyrox_bridge`, merged by appearance first, so the plant
arrives as at most four draw calls.

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

--- Geometry (Hub LOD) ---
  Symbols:         1893
  Stem segments:   25
  Shapes:          90
  Draw calls:      4
  Triangles:       2168 (budget 12000)
  Leaves:          57
  Petals:          15
  Fruit:           3

--- Measurements ---
  Surface area:    3.8412
  Volume:          0.004917
  Bounding box:    2.41 x 3.02 x 2.18  (height 3.02)

--- Alchemy Effects ---
  Heal: 15 HP
  Buff: Haste for 3 turns
```

Surface area and volume come from `plantgl`'s `MeasureDrawer`, which reads them
off the turtle's own command sequence rather than off a mesh — the same numbers
a harvest yield is computed from, available without building geometry.

### OBJ Export

The viewer automatically exports a Wavefront OBJ file on startup:

```
plant_seed_42.obj
```

...alongside the `plant_seed_42.mtl` it references. Both are written by
`plantgl::codec::obj` from the merged, discretised scene, so the file can be
opened in any 3D modelling application (Blender, MeshLab, …). It carries one
group per material — `stem`, `leaf`, `petal`, `fruit` — with real surfaces in
each, not marker triangles.

To generate the files without the viewer, use the library API:

```rust
use apothecarys_tools::plant_preview::PlantPreviewData;

let preview = PlantPreviewData::from_seed(42);
let files = preview.to_obj("my_plant.mtl");
std::fs::write("my_plant.obj", files.obj).unwrap();
std::fs::write("my_plant.mtl", files.mtl).unwrap();
```

## Library API

The `PlantPreviewData` struct provides programmatic access to all generated plant data:

```rust
use apothecarys_tools::plant_preview::PlantPreviewData;

let preview = PlantPreviewData::from_seed(42);

// Access plant properties
println!("Triangles: {}", preview.plant.triangle_count().unwrap());
println!("Area: {}", preview.plant.measures().unwrap().surface_area);
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
| `plant` | `PlantModel` | The `plantgl::Scene`, its counts, and its measurements |
| `alchemy_effects` | `Vec<AlchemyEffect>` | Potion effects derived from genetics |

## Determinism

The same seed always produces the same plant. This is useful for:
- Reproducing interesting specimens
- Sharing plants by seed number
- Testing and debugging the generation pipeline
