# 04 — Apothecary, Botany & Genetics

## Scope

Plant genetics system, L-system procedural mesh generation (ported from vlab/L-studio), phenotype expression, genotype-to-stat mapping, crafting/alchemy, garden management, and inventory.

## Plant Genetics System

### Overview

Every plant in the game has a **genotype** — a set of hidden genetic parameters that the player never directly sees. The genotype determines:
1. **Phenotype** (visual appearance): size, shape, color, leaf shape, flowers, fruits
2. **Alchemy properties**: what stat buffs/debuffs the plant produces when used in crafting

The player must experiment with plants to discover their properties through crafting and observation.

### Genotype Representation

```rust
// crates/botany/src/genetics.rs

/// A single gene with diploid alleles (simplified Mendelian model)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gene {
    pub allele_a: f32,  // Range [0.0, 1.0]
    pub allele_b: f32,
    pub dominance: Dominance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Dominance {
    Complete,        // Higher allele dominates
    Incomplete,      // Blended (average)
    Codominant,      // Both expressed
}

impl Gene {
    /// Express the gene as a single phenotype value
    pub fn express(&self) -> f32 {
        match self.dominance {
            Dominance::Complete => self.allele_a.max(self.allele_b),
            Dominance::Incomplete => (self.allele_a + self.allele_b) / 2.0,
            Dominance::Codominant => self.allele_a + self.allele_b, // clamped later
        }
    }
}

/// Full plant genotype
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlantGenotype {
    // Morphology genes
    pub stem_height: Gene,          // Affects overall plant height
    pub stem_thickness: Gene,       // Stem/trunk diameter
    pub branching_angle: Gene,      // Angle between branches
    pub branching_density: Gene,    // Number of branches per segment
    pub internode_length: Gene,     // Distance between branch points

    // Leaf genes
    pub leaf_size: Gene,            // Scale of leaf meshes
    pub leaf_shape: Gene,           // Index into leaf shape variants (0.0–1.0 mapped)
    pub leaf_density: Gene,         // Leaves per branch segment
    pub leaf_color_hue: Gene,       // HSV hue shift
    pub leaf_color_saturation: Gene,

    // Flower genes
    pub has_flowers: Gene,          // Expression > 0.5 = produces flowers
    pub petal_count: Gene,          // Mapped to discrete: 3, 4, 5, 6, 8
    pub petal_color_hue: Gene,
    pub petal_size: Gene,
    pub flower_density: Gene,

    // Fruit genes
    pub has_fruit: Gene,            // Expression > 0.5 = produces fruit
    pub fruit_size: Gene,
    pub fruit_color_hue: Gene,
    pub fruit_shape: Gene,          // Index into fruit shape variants

    // Alchemy genes (hidden from player, mapped to effects)
    pub potency: Gene,              // Overall effect strength
    pub healing_affinity: Gene,     // Healing vs damage
    pub stat_target: Gene,          // Which stat is affected
    pub duration_gene: Gene,        // Effect duration
    pub toxicity: Gene,             // Side effect severity
}
```

### Genetic Crossover (Breeding)

When the player cross-pollinates plants in the garden:

```rust
pub fn crossover(parent_a: &PlantGenotype, parent_b: &PlantGenotype, rng: &mut impl Rng) -> PlantGenotype {
    // For each gene: randomly select one allele from each parent
    // With small mutation chance (5%) per allele
    PlantGenotype {
        stem_height: cross_gene(&parent_a.stem_height, &parent_b.stem_height, rng),
        // ... repeat for all genes
    }
}

fn cross_gene(a: &Gene, b: &Gene, rng: &mut impl Rng) -> Gene {
    let allele_a = if rng.gen_bool(0.5) { a.allele_a } else { a.allele_b };
    let allele_b = if rng.gen_bool(0.5) { b.allele_a } else { b.allele_b };

    // Mutation: small random perturbation
    let mutate = |v: f32| -> f32 {
        if rng.gen_bool(0.05) {
            (v + rng.gen_range(-0.1..0.1)).clamp(0.0, 1.0)
        } else {
            v
        }
    };

    Gene {
        allele_a: mutate(allele_a),
        allele_b: mutate(allele_b),
        dominance: a.dominance.clone(), // Dominance pattern is inherited, not mutated
    }
}
```

**Task Goal**: Implement `PlantGenotype`, `Gene`, gene expression, and crossover in `crates/botany/src/genetics.rs`. Include unit tests for Mendelian inheritance ratios and mutation rates.

## Phenotype Expression

### Genotype → Visual Parameters

```rust
// crates/botany/src/phenotype.rs

/// All visual parameters derived from genotype
#[derive(Debug, Clone)]
pub struct PlantPhenotype {
    // L-system parameters
    pub axiom_complexity: u32,        // Derivation steps (1–6)
    pub branch_angle: f32,            // Degrees (15–60)
    pub branch_length: f32,           // World units (0.1–2.0)
    pub branch_thickness: f32,        // Radius (0.01–0.1)
    pub branching_factor: u32,        // Branches per node (1–4)

    // Stem parameters — the shape of the swept axis itself (added by #21)
    pub cross_section_index: usize,   // Round, square, triangular, fluted
    pub taper_curve: TaperCurve,      // How fast a stem narrows per segment
    pub tropism_elasticity: f32,      // How far a segment gives to gravity
    pub axis_curvature: f32,          // Degrees of pitch per drawn segment

    // Leaf parameters
    pub leaf_mesh_index: usize,       // Index into the organ surface library
    pub leaf_scale: f32,
    pub leaf_color: PlantColor,
    pub leaves_per_segment: u32,

    // Flower parameters
    pub produces_flowers: bool,
    pub petal_count: u32,
    pub petal_mesh_index: usize,
    pub petal_color: PlantColor,
    pub petal_scale: f32,

    // Fruit parameters
    pub produces_fruit: bool,
    pub fruit_mesh_index: usize,
    pub fruit_color: PlantColor,
    pub fruit_scale: f32,

    /// Which quality tier to build at. Not genetic — the caller sets it.
    pub lod_tier: LodTier,
}

pub fn express_phenotype(genotype: &PlantGenotype) -> PlantPhenotype {
    PlantPhenotype {
        axiom_complexity: map_range(genotype.branching_density.express(), 0.0, 1.0, 1, 6),
        branch_angle: map_range(genotype.branching_angle.express(), 0.0, 1.0, 15.0, 60.0),
        branch_length: map_range(genotype.internode_length.express(), 0.0, 1.0, 0.1, 2.0),
        // ... map all genes to visual parameters
    }
}
```

**Task Goal**: Implement phenotype expression as a pure function from genotype to visual parameters. This function is deterministic — same genotype always produces same phenotype.

The four stem traits #21 added have no gene of their own: widening
`PlantGenotype` would invalidate every save and every stored breeding pair, so
each rides the gene nearest it in meaning. The cross-section profile and the
tropism elasticity ride `stem_thickness` — a thin stem is a floppy one — the
taper curve rides `internode_length`, and the axis curvature rides
`stem_height`, the one morphology gene the earlier phenotype never expressed.
`express_phenotype` documents each mapping at the call site.

## L-System Procedural Plant Generation

### Background

The L-system engine is inspired by **vlab** and **L-studio** (algorithmic botany tools from the University of Calgary). The core concepts:

1. **Alphabet**: Symbols representing plant parts (`F` = forward/stem, `+`/`-` = turn, `[`/`]` = push/pop, `L` = leaf, `W` = flower, `R` = fruit)
2. **Axiom**: Starting string
3. **Production Rules**: Rewriting rules applied iteratively
4. **Turtle Interpretation**: Convert final string to 3D geometry — done by
   `crates/plantgl` since Phase E of the PlantGL port; see below.

### L-System Engine

```rust
// crates/botany/src/lsystem.rs

#[derive(Debug, Clone)]
pub enum LSymbol {
    Forward(f32),       // F(length) — grow stem segment
    TurnLeft(f32),      // +(angle)
    TurnRight(f32),     // -(angle)
    PitchUp(f32),       // ^(angle)
    PitchDown(f32),     // &(angle)
    RollLeft(f32),      // /(angle)
    RollRight(f32),     // \(angle)
    Push,               // [ — save state
    Pop,                // ] — restore state
    Leaf,               // L — a leaf of the phenotype's own shape (pre-#21)
    Flower,             // W — a flower of the phenotype's own petal count
    Fruit,              // R — a fruit of the phenotype's own shape
    Width(f32),         // !(width) — set stem width
    Apex,               // A — growth apex (replaced by rules)

    // Added by Phase E of the PlantGL port (#21). The rewriting engine below
    // is unchanged: these are symbols the *interpreter* acts on.
    StartGC,                   // open a swept axis
    StopGC,                    // close it and draw it
    SetCrossSection(usize),    // the profile to sweep; 0 is the round default
    SetTropism(f32),           // elasticity towards gravity
    Surface(SurfaceId, f32),   // a named organ template, scaled
}

#[derive(Debug)]
pub struct ProductionRule {
    pub predecessor: LSymbol,
    pub successor: Vec<LSymbol>,
    pub probability: f32,  // Stochastic rules (0.0–1.0)
}

pub struct LSystem {
    pub axiom: Vec<LSymbol>,
    pub rules: Vec<ProductionRule>,
}

impl LSystem {
    /// Build an L-system from phenotype parameters
    pub fn from_phenotype(phenotype: &PlantPhenotype) -> Self {
        let angle = phenotype.branch_angle;
        let length = phenotype.branch_length;
        // 180° for a two-ranked plant, then 120°, 90°, 72°.
        let divergence = 360.0 / (phenotype.branching_factor + 1) as f32;

        // The fertile rules come FIRST. `find_matching_rule` accumulates the
        // matching rules' probabilities in order, so a rule behind one of
        // probability 1.0 can never be reached — which is why no plant grew a
        // flower before #21. Each also ends in an `Apex`, so a node that
        // flowers is still a node that grows.
        let rules = vec![
            ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![
                    // A flower on a short stalk, then carry on growing.
                    LSymbol::Push,
                    LSymbol::TurnLeft(PEDUNCLE_ANGLE),
                    LSymbol::Forward(length * 0.45),
                    /* petal_count × [ roll, pitch, Surface(Petal) ] */
                    LSymbol::Pop,
                    LSymbol::Apex,
                ],
                probability: 0.18,
            },
            /* the fruit rule, the same shape */
            ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![
                    LSymbol::Forward(length),
                    /* leaves_per_segment × [ roll, pitch, Surface(Leaf) ] */
                    LSymbol::Push,
                    LSymbol::RollLeft(divergence),
                    LSymbol::TurnLeft(angle),
                    LSymbol::PitchUp(angle * 0.5),
                    LSymbol::Apex,
                    LSymbol::Pop,
                    // The axis CONTINUES. This is what joins consecutive
                    // internodes into one sweep; two pushed branches would
                    // leave every `Forward` alone between a `Push` and a `Pop`.
                    LSymbol::RollLeft(divergence),
                    LSymbol::Apex,
                ],
                probability: 1.0 - 0.18 - 0.12,
            },
        ];

        LSystem {
            // The profile and the tropism are set before the sweep opens: a
            // generalized cylinder is drawn with the parameters it was
            // *opened* with.
            axiom: vec![
                LSymbol::SetCrossSection(phenotype.cross_section_index),
                LSymbol::SetTropism(phenotype.tropism_elasticity),
                LSymbol::Width(phenotype.branch_thickness),
                LSymbol::StartGC,
                LSymbol::Apex,
                LSymbol::StopGC,
            ],
            rules,
        }
    }

    /// Apply production rules n times
    pub fn derive(&self, iterations: u32, rng: &mut impl Rng) -> Vec<LSymbol> {
        let mut current = self.axiom.clone();
        for _ in 0..iterations {
            current = self.apply_rules(&current, rng);
        }
        current
    }

    fn apply_rules(&self, input: &[LSymbol], rng: &mut impl Rng) -> Vec<LSymbol> {
        let mut output = Vec::new();
        for symbol in input {
            if let Some(rule) = self.find_matching_rule(symbol, rng) {
                output.extend(rule.successor.clone());
            } else {
                output.push(symbol.clone());
            }
        }
        output
    }
}
```

**Task Goal**: Implement the L-system string rewriting engine. Support parameterized symbols, stochastic rules, and context-sensitive rules (for vlab compatibility). Must be deterministic given the same RNG seed.

`derive` draws from the RNG only for a symbol some rule matches, so the
derivation is reproducible from a seed and a capped derivation is a prefix of a
deeper one. That is what the LOD tiers and the save format both rest on.

### Turtle Interpretation → 3D Geometry

Interpretation does **not** live in `crates/botany` any more. Phase E of the
PlantGL port (#21) deleted `crates/botany/src/turtle.rs` — its `Vec3`, its
axis-angle rotation and its `TurtleInterpreter` — and moved the whole job onto
`crates/plantgl`, the Rust translation of
[openalea/plantgl](https://github.com/openalea/plantgl). See
`docs/design/08-plantgl-port.md` for the port itself.

The division of labour is the L-Py ↔ PlantGL boundary upstream draws:

| Crate | Knows | Licence |
|---|---|---|
| `crates/botany` | genetics, phenotype, the alphabet, the production rules, which organ goes where | MIT |
| `crates/plantgl` | the frame, the stack, sweeps, patches, discretisation, tessellation, measurement, OBJ | CeCILL-C |

`crates/botany/src/interpret.rs` is the `match` between them: it walks a
derived string and calls `plantgl::modelling::Turtle`. Nothing translated from
PlantGL may be copied across that line — that is what keeps `botany` MIT.

```rust
// crates/botany/src/interpret.rs

pub fn interpret<D: TurtleDrawer>(
    turtle: &mut Turtle<D>,
    symbols: &[LSymbol],
    phenotype: &PlantPhenotype,
) -> Result<PlantStats> {
    for symbol in symbols {
        match symbol {
            // Inside a sweep an `F` records a ring rather than drawing a tube,
            // and narrows the axis by the phenotype's taper ratio.
            LSymbol::Forward(length) => {
                turtle.forward_tapered(*length, turtle.width() * taper)?;
                turtle.down(phenotype.axis_curvature);
            }
            LSymbol::TurnLeft(angle) => turtle.left(*angle),
            LSymbol::Push => turtle.push(),
            LSymbol::Pop => turtle.pop()?,
            LSymbol::StartGC => turtle.start_gc(),
            LSymbol::StopGC => turtle.stop_gc()?,
            LSymbol::Surface(id, scale) => turtle.surface(id.name(), *scale)?,
            /* … */
        }
    }
}
```

Being generic over `TurtleDrawer` means one derived string can produce three
different things without re-deriving it:

- `SceneDrawer` → a `plantgl::Scene` of parametric shapes. Inspectable,
  exportable, re-tessellatable at another density. This is what
  `PlantModel::scene` holds.
- `MeshDrawer` → one merged `TriangleSet` per appearance, skipping the scene
  graph. The renderer's path.
- `MeasureDrawer` → surface area, volume, bounding box and segment count,
  allocating almost nothing. **Harvest yield without building a mesh**, which
  is what ties the reward to the phenotype the player can see.

### Generalized cylinders: why the stems changed shape

The old interpreter emitted one `StemSegment` per internode and the old mesh
builder turned each into a two-ring cylinder. Consecutive internodes therefore
met at a hard seam, with no mitre and no shared width, and a plant read as a
stack of cans.

`LSystem::from_phenotype` now wraps the whole plant in `StartGC` … `StopGC`,
and the growth rule *continues* its axis with a bare `Apex` instead of ending
in two pushed branches. Between `StartGC` and `StopGC` a `Forward` records a
point, a `left` vector and a width rather than drawing anything; the turtle
sweeps the accumulated run as one `plantgl::Extrusion` under
rotation-minimising frames, and splits the sweep by itself at every
`Push`/`Pop`. One branch is one continuous, mitred, tapering surface.

Sweeps do not nest — a second `StartGC` inside an open one discards the axis so
far — which is why there is exactly one pair, in the axiom, and why the
cross-section and the tropism are set before it: a sweep is drawn with the
parameters it was *opened* with.

### Organ surfaces

Leaves, petals and fruit are no longer marker instances to be swapped for art
assets later. `crates/botany/src/surfaces.rs` builds them procedurally into a
`plantgl::SurfaceLibrary`, and the L-system places them with
`LSymbol::Surface(SurfaceId, scale)`:

| Kind | Shape | Indexed by |
|---|---|---|
| Leaf | 4×4 Bézier patch, cupped across and drooping along | `leaf_mesh_index` (5 outlines) |
| Petal | the same, shorter and cupped harder | `petal_mesh_index` (3 outlines) |
| Fruit | a scaled sphere of revolution | `fruit_mesh_index` (4 bodies) |

Each is modelled in the turtle's local frame — running from `z = 0` at its
attachment to `z = 1` at its tip — which is the convention upstream's default
`"l"` leaf uses, so a surface written here drops into a cpfg program unchanged.

Because a patch is parametric, the same library at a coarser LOD tier is the
same shape sampled less finely, not a different mesh.

### Mesh Construction

`PlantMeshData` is gone. `crates/botany/src/mesh_gen.rs` is a re-export facade
over `interpret`, and the type a caller gets back is `PlantModel`:

```rust
// crates/botany/src/interpret.rs

pub struct PlantModel {
    pub scene: plantgl::Scene,      // one shape per swept axis and per organ
    pub phenotype: PlantPhenotype,  // tier included
    pub stats: PlantStats,          // segments, leaves, petals, fruit
    pub symbols: Vec<LSymbol>,      // kept, so it can be measured or re-drawn
}

impl PlantModel {
    pub fn batches(&self) -> Result<Vec<MeshBatch>>;   // merged by appearance
    pub fn triangle_count(&self) -> Result<usize>;
    pub fn measures(&self) -> Result<Measures>;        // area, volume, bbox
    pub fn to_obj(&self, mtl: &str) -> Result<ObjFiles>;
}
```

Shapes are merged by appearance **before** conversion, so a plant reaches the
renderer as at most four draw calls — stem, leaf, petal, fruit — rather than
one per leaf. `crates/botany/src/fyrox_bridge.rs` does the conversion:
`TriangleSet` is already structure-of-arrays and `plantgl`'s `real_t` is `f32`,
so it is a repack into Fyrox's interleaved `StaticVertex` plus an index map,
with no numeric conversion.

Measurement goes back to the turtle rather than to the mesh, and is computed on
first use. That is not an optimisation: a turtle's tubes are drawn open —
upstream never caps them — so their meshes enclose no volume at all, while the
volume of the stem is exactly what a yield model wants. `MeasureDrawer` sums it
as though the ends were closed.

### Level of detail

`crates/botany/src/lod.rs` defines three tiers, each fixing four things at
once: how many derivation steps the L-system takes, how many facets a swept
stem has, how finely an organ patch is sampled, and whether organs are drawn at
all.

| Tier | Iterations | Section | Patch | Organs | Triangle budget |
|---|---|---|---|---|---|
| `Hub` | 6 | 10 | 5×4 | yes | < 12 000 |
| `Distant` | 5 | 5 | 3×2 | yes | < 1 500 |
| `Icon` | 3 | 3 | 2×2 | no | < 400 |

Capping the derivation is the only lever with real leverage at the low end —
no tessellation density takes a 300-leaf plant under 400 triangles. Because
`LSystem::derive` re-derives from the axiom and consumes the RNG in iteration
order, a capped derivation is the *prefix* of the uncapped one for the same
seed: the icon is a smaller version of the same plant, not a different plant.

`crates/botany/tests/budget.rs` enforces the budgets, and
`crates/botany/tests/golden.rs` snapshots the OBJ output for five seeds, with
the pre-port baseline kept alongside under `tests/golden/pre-plantgl/`.

### Where this leaves vlab/L-studio

The original plan was to port the pieces of vlab/L-studio the game needed. It
went to PlantGL instead, which is the same research lineage — CIRAD/INRIA/INRA
rather than Calgary — but is CeCILL-C rather than unlicensed, is still
maintained, and ships the parts that are hard to get right. `08-plantgl-port.md`
records why. What that table used to promise now maps like this:

| vlab/L-studio component | Where it lives | Notes |
|---|---|---|
| `LEngine` (string rewriting) | `crates/botany/src/lsystem.rs` | Parametric and stochastic rules, unchanged by the port |
| `Turtle` (3D interpretation) | `plantgl::modelling::turtle`, driven by `crates/botany/src/interpret.rs` | The full cpfg/L-studio command set |
| `Surface` (mesh generation) | `plantgl::Extrusion` for stems, `plantgl::BezierPatch` for organs | Generalized cylinders under rotation-minimising frames |
| `Environment` (tropisms) | `Turtle::set_tropism` / `set_elasticity`, from `PlantPhenotype::tropism_elasticity` | Gravitropism today; a light direction is the same call |

## Genotype → Alchemy Effect Mapping

### How Genetics Map to Gameplay Effects

The player never sees gene values. They discover effects by crafting and using potions.

```rust
// crates/botany/src/stat_mapping.rs

/// Map a plant's hidden genetics to alchemy effects
pub fn genetics_to_effects(genotype: &PlantGenotype) -> Vec<AlchemyEffect> {
    let mut effects = Vec::new();

    // Primary effect: healing vs damage
    let healing = genotype.healing_affinity.express();
    let potency = genotype.potency.express();
    let stat_target_value = genotype.stat_target.express();
    let duration = map_range(genotype.duration_gene.express(), 0.0, 1.0, 1, 5);
    let toxicity = genotype.toxicity.express();

    // Determine primary stat target
    let target_stat = match (stat_target_value * 6.0) as u32 {
        0 => AttributeType::Strength,
        1 => AttributeType::Dexterity,
        2 => AttributeType::Constitution,
        3 => AttributeType::Intelligence,
        4 => AttributeType::Wisdom,
        _ => AttributeType::Charisma,
    };

    if healing > 0.5 {
        // Healing plant
        let heal_amount = map_range(potency, 0.0, 1.0, 5, 30);
        effects.push(AlchemyEffect::Heal(heal_amount));

        if potency > 0.7 {
            let boost = map_range(potency, 0.7, 1.0, 1, 4);
            effects.push(AlchemyEffect::StatBuff {
                stat: target_stat,
                amount: boost,
                turns: duration,
            });
        }
    } else {
        // Harmful plant (poisons, debuffs for enemy use)
        let damage = map_range(potency, 0.0, 1.0, 3, 20);
        effects.push(AlchemyEffect::Damage(damage));
        effects.push(AlchemyEffect::StatDebuff {
            stat: target_stat,
            amount: map_range(potency, 0.0, 1.0, 1, 3),
            turns: duration,
        });
    }

    // Toxicity = side effect on user
    if toxicity > 0.6 {
        effects.push(AlchemyEffect::SideEffect {
            damage: map_range(toxicity, 0.6, 1.0, 1, 10),
        });
    }

    effects
}
```

**Task Goal**: Implement the mapping from genetics to alchemy effects. The system should be configurable via data files so designers can tune the mapping curves.

## Crafting / Alchemy System

### Recipe Structure

```rust
// crates/inventory/src/crafting.rs

#[derive(Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub category: RecipeCategory,
    pub ingredients: Vec<IngredientSlot>,
    pub result_type: ResultType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RecipeCategory {
    HealingPotion,
    BuffPotion,
    Poison,
    Medicine,      // Cures status effects
    Fertilizer,    // For garden use
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngredientSlot {
    pub slot_type: IngredientType,
    pub required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IngredientType {
    AnyPlant,
    PlantWithTrait { min_healing: Option<f32>, min_potency: Option<f32> },
    SpecificItem(String),  // e.g., "empty_vial", "purified_water"
    Catalyst,              // Reagent that modifies the recipe
}
```

### Crafting Flow

```
Player opens crafting UI
         │
         ▼
Select recipe from known recipes
         │
         ▼
Fill ingredient slots from inventory
         │
         ▼
"Brew" button → resolve recipe
         │
         ▼
For each plant ingredient:
    - Read hidden genetics
    - Map genetics → alchemy effects
    - Combine effects based on recipe type
         │
         ▼
Generate result item with:
    - Name (generated from effect profile)
    - Combined effects
    - Quality rating (based on ingredient potency alignment)
    - Visual indicator (color of liquid based on primary effect)
         │
         ▼
Add to inventory, consume ingredients
```

**Task Goal**: Implement the crafting system in `crates/inventory/src/crafting.rs`. Recipe resolution combines plant genetics from multiple ingredients, weighted by the recipe type.

## Garden System

### Garden Layout

The garden is a hub sub-location with a grid of **plots**. Each plot can hold one plant.

```rust
// crates/garden/src/plots.rs

pub struct Garden {
    pub plots: Vec<GardenPlot>,
    pub max_plots: usize,       // Upgradeable
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GardenPlot {
    pub index: usize,
    pub state: PlotState,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PlotState {
    Empty,
    Planted {
        plant: PlantInstance,
        growth_stage: f32,      // 0.0 = seed, 1.0 = mature
        watered: bool,
        health: f32,            // 0.0 = dead, 1.0 = perfect
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlantInstance {
    pub id: Uuid,
    pub genotype: PlantGenotype,
    pub species_name: String,       // Player-assigned or auto-generated
    pub generation: u32,            // 0 = wild, 1+ = bred
    pub parent_ids: Option<(Uuid, Uuid)>,
}
```

### Growth Simulation

```
Each dungeon run advances garden time by 1 "cycle"

Per cycle per plot:
    if planted and watered:
        growth_stage += growth_rate (affected by genetics, soil quality)
        health affected by:
            - Was it watered? (+0.1 if yes, -0.2 if no)
            - Random pest chance (5%, mitigated by garden upgrades)
            - Genetic vigor (constitution-analog gene)

    if growth_stage >= 1.0:
        Plant is mature → can be harvested, bred, or left to produce seeds
```

### Breeding Flow

```
Player selects two mature plants in adjacent plots
         │
         ▼
"Cross-pollinate" action
         │
         ▼
crossover(parent_a.genotype, parent_b.genotype, rng) → child genotype
         │
         ▼
Child seed added to inventory
Player can plant it in an empty plot
         │
         ▼
Child grows with its own phenotype (player observes differences)
```

**Task Goal**: Implement garden state management, growth simulation, and breeding in `crates/garden/`. The player's goal is to selectively breed plants with desired (but hidden) alchemy properties by observing phenotype changes across generations.

## Inventory System

```rust
// crates/inventory/src/container.rs

pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStack {
    pub item: Item,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: Uuid,
    pub template_id: String,
    pub name: String,
    pub item_type: ItemType,
    pub icon_path: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemType {
    PlantSample { genotype: PlantGenotype },
    Potion { effects: Vec<AlchemyEffect> },
    Medicine { cures: Vec<StatusEffectType> },
    Ingredient { ingredient_type: IngredientType },
    Equipment(EquipmentData),
    QuestItem,
    Seed { genotype: PlantGenotype },
    Gold(u32),
}
```

**Task Goal**: Implement the inventory system with generic containers usable for player inventory, party shared inventory, shop inventories, and loot containers.

## Key Implementation Files

| File | Purpose |
|------|---------|
| `crates/botany/src/genetics.rs` | Genotype, genes, crossover, mutation |
| `crates/botany/src/phenotype.rs` | Genotype → visual parameter mapping |
| `crates/botany/src/lsystem.rs` | L-system string rewriting engine |
| `crates/botany/src/interpret.rs` | L-symbol → `plantgl` turtle dispatch |
| `crates/botany/src/surfaces.rs` | Procedural leaf/petal/fruit templates and stem profiles |
| `crates/botany/src/lod.rs` | Quality tiers and their triangle budgets |
| `crates/botany/src/fyrox_bridge.rs` | `plantgl` geometry → Fyrox scene nodes (feature `fyrox`) |
| `crates/botany/src/mesh_gen.rs` | Re-export facade over `interpret` |
| `crates/plantgl/` | Geometry and turtle modelling, ported from PlantGL (CeCILL-C) |
| `crates/botany/src/stat_mapping.rs` | Genetics → alchemy effect mapping |
| `crates/inventory/src/container.rs` | Inventory containers |
| `crates/inventory/src/crafting.rs` | Recipe resolution, potion creation |
| `crates/inventory/src/items.rs` | Item type definitions |
| `crates/inventory/src/interaction.rs` | Pickup, use, give mechanics |
| `crates/garden/src/plots.rs` | Garden plot state |
| `crates/garden/src/growth.rs` | Growth simulation |
| `crates/garden/src/breeding.rs` | Cross-pollination |
| `data/recipes.ron` | Crafting recipe definitions |
| `data/plant_genetics.ron` | Base genetic parameter ranges per species |
