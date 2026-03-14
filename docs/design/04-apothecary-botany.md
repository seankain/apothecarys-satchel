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

    // Leaf parameters
    pub leaf_mesh_index: usize,       // Index into leaf template meshes
    pub leaf_scale: Vector3<f32>,
    pub leaf_color: Color,
    pub leaves_per_segment: u32,

    // Flower parameters
    pub produces_flowers: bool,
    pub petal_count: u32,
    pub petal_mesh_index: usize,
    pub petal_color: Color,
    pub petal_scale: f32,

    // Fruit parameters
    pub produces_fruit: bool,
    pub fruit_mesh_index: usize,
    pub fruit_color: Color,
    pub fruit_scale: f32,
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

## L-System Procedural Plant Generation

### Background

The L-system engine is inspired by **vlab** and **L-studio** (algorithmic botany tools from the University of Calgary). The core concepts:

1. **Alphabet**: Symbols representing plant parts (`F` = forward/stem, `+`/`-` = turn, `[`/`]` = push/pop, `L` = leaf, `W` = flower, `R` = fruit)
2. **Axiom**: Starting string
3. **Production Rules**: Rewriting rules applied iteratively
4. **Turtle Interpretation**: Convert final string to 3D geometry

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
    Leaf,               // L — place leaf
    Flower,             // W — place flower
    Fruit,              // R — place fruit
    Width(f32),         // !(width) — set stem width
    Apex,               // A — growth apex (replaced by rules)
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
        // Generate rules parameterized by phenotype values
        let angle = phenotype.branch_angle;
        let length = phenotype.branch_length;

        let rules = vec![
            // Apex grows into a stem segment and branches
            ProductionRule {
                predecessor: LSymbol::Apex,
                successor: vec![
                    LSymbol::Width(phenotype.branch_thickness),
                    LSymbol::Forward(length),
                    LSymbol::Push,
                    LSymbol::TurnLeft(angle),
                    LSymbol::Leaf,
                    LSymbol::Apex,
                    LSymbol::Pop,
                    LSymbol::Push,
                    LSymbol::TurnRight(angle),
                    LSymbol::Leaf,
                    LSymbol::Apex,
                    LSymbol::Pop,
                ],
                probability: 1.0,
            },
            // Additional rules for flowers, fruit, branching variants...
        ];

        LSystem {
            axiom: vec![LSymbol::Apex],
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

### Turtle Interpretation → 3D Mesh

```rust
// crates/botany/src/turtle.rs

pub struct TurtleState {
    pub position: Vector3<f32>,
    pub heading: Vector3<f32>,      // Forward direction
    pub left: Vector3<f32>,         // Left direction
    pub up: Vector3<f32>,           // Up direction
    pub width: f32,                 // Current stem width
}

pub struct TurtleInterpreter {
    state: TurtleState,
    stack: Vec<TurtleState>,
    mesh_builder: PlantMeshBuilder,
}

impl TurtleInterpreter {
    /// Interpret L-system string into mesh data
    pub fn interpret(
        &mut self,
        symbols: &[LSymbol],
        phenotype: &PlantPhenotype,
    ) -> PlantMeshData {
        for symbol in symbols {
            match symbol {
                LSymbol::Forward(len) => {
                    let start = self.state.position;
                    self.state.position += self.state.heading * len;
                    self.mesh_builder.add_stem_segment(
                        start,
                        self.state.position,
                        self.state.width,
                    );
                }
                LSymbol::TurnLeft(angle) => self.rotate_heading(*angle, self.state.up),
                LSymbol::TurnRight(angle) => self.rotate_heading(-angle, self.state.up),
                LSymbol::Push => self.stack.push(self.state.clone()),
                LSymbol::Pop => self.state = self.stack.pop().unwrap(),
                LSymbol::Leaf => {
                    self.mesh_builder.add_leaf_instance(
                        self.state.position,
                        self.state.heading,
                        phenotype.leaf_mesh_index,
                        phenotype.leaf_scale,
                        phenotype.leaf_color,
                    );
                }
                LSymbol::Flower if phenotype.produces_flowers => {
                    self.mesh_builder.add_flower_instance(
                        self.state.position,
                        phenotype.petal_count,
                        phenotype.petal_color,
                        phenotype.petal_scale,
                    );
                }
                LSymbol::Fruit if phenotype.produces_fruit => {
                    self.mesh_builder.add_fruit_instance(
                        self.state.position,
                        phenotype.fruit_mesh_index,
                        phenotype.fruit_color,
                        phenotype.fruit_scale,
                    );
                }
                _ => {}
            }
        }
        self.mesh_builder.build()
    }
}
```

### Mesh Construction

```rust
// crates/botany/src/mesh_gen.rs

pub struct PlantMeshData {
    pub stem_vertices: Vec<Vertex>,
    pub stem_indices: Vec<u32>,
    pub leaf_instances: Vec<MeshInstance>,    // Instanced rendering of leaf template
    pub flower_instances: Vec<MeshInstance>,
    pub fruit_instances: Vec<MeshInstance>,
}

pub struct MeshInstance {
    pub template_index: usize,    // Which base mesh to instance
    pub transform: Matrix4<f32>,  // Position, rotation, scale
    pub color: Color,             // Tint color
}

impl PlantMeshData {
    /// Convert to Fyrox scene nodes
    pub fn to_scene_nodes(&self, scene: &mut Scene, templates: &PlantTemplates) -> Handle<Node> {
        // Create parent node for the plant
        // Add stem mesh (custom geometry from vertices/indices)
        // Add instanced leaf/flower/fruit meshes using templates
    }
}
```

**Task Goal**: Implement turtle interpretation and mesh construction. Stem segments are generated as cylinder geometry between turtle positions. Leaves, flowers, and fruits are placed as instanced copies of template meshes loaded from `assets/models/plants/`.

### Porting from vlab/L-studio

Key C++ components to port:

| vlab/L-studio Component | Rust Equivalent | Notes |
|--------------------------|-----------------|-------|
| `LEngine` (string rewriting) | `crates/botany/src/lsystem.rs` | Core rewriting; support parametric & stochastic rules |
| `Turtle` (3D interpretation) | `crates/botany/src/turtle.rs` | 3D turtle with heading/up/left vectors |
| `Surface` (mesh generation) | `crates/botany/src/mesh_gen.rs` | Generalized cylinders for stems |
| `Environment` (tropisms) | Optional: gravitropism, phototropism | Bend stems toward/away from direction vectors |

The port does **not** need to be complete — focus on:
1. Parametric L-systems with stochastic rules
2. 3D turtle interpretation
3. Generalized cylinder stem generation
4. Instanced organ placement (leaves, flowers, fruit)

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
| `crates/botany/src/turtle.rs` | 3D turtle interpreter |
| `crates/botany/src/mesh_gen.rs` | Plant mesh construction |
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
