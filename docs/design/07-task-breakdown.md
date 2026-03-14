# 07 — Task Breakdown & Dependency Graph

## Overview

Tasks are organized into **phases** reflecting natural dependency order. Within each phase, tasks are numbered and can be worked on in parallel where dependencies allow. Each task has a clear **deliverable** and **acceptance criteria**.

## Dependency Graph (Visual)

```
Phase 1: Foundation
  T1.1 Workspace Setup ──────────────┐
  T1.2 Core Types ───────────────────┤
  T1.3 Fyrox Game Plugin ───────────┤
                                     │
Phase 2: Core Systems               │
  T2.1 Isometric Camera ◄───────────┤
  T2.2 Navigation Mesh ◄────────────┤
  T2.3 Player Movement ◄─── T2.1, T2.2
  T2.4 Interaction System ◄── T2.1, T1.2
  T2.5 Stat System ◄──────── T1.2
  T2.6 Inventory System ◄─── T1.2
                                     │
Phase 3: Content Systems             │
  T3.1 World Graph ◄──────── T1.2   │
  T3.2 Scene Transitions ◄── T3.1, T2.1
  T3.3 YarnSpinner Parser ◄─ (standalone)
  T3.4 Dialogue Runner ◄──── T3.3
  T3.5 Lua Scripting ◄────── T1.2
                                     │
Phase 4: Gameplay Systems            │
  T4.1 Party Generation ◄─── T2.5   │
  T4.2 Combat Turn Manager ◄─ T2.5, T4.1
  T4.3 Combat AI ◄────────── T4.2
  T4.4 Player Combat Actions ◄ T4.2, T2.6
  T4.5 Crafting System ◄──── T2.6
                                     │
Phase 5: Botany                      │
  T5.1 Genetics System ◄──── (standalone)
  T5.2 Phenotype Expression ◄ T5.1
  T5.3 L-System Engine ◄──── (standalone)
  T5.4 Turtle Interpreter ◄── T5.3
  T5.5 Plant Mesh Gen ◄───── T5.2, T5.4
  T5.6 Stat Mapping ◄─────── T5.1, T2.5
  T5.7 Garden System ◄────── T5.1, T5.5
                                     │
Phase 6: Persistence & Polish        │
  T6.1 Save System ◄──────── All Phase 4, T5.7
  T6.2 Load System ◄──────── T6.1
  T6.3 Hub Location ◄─────── T3.2, T4.1, T5.7
  T6.4 Game UI / HUD ◄────── T2.6, T4.2
                                     │
Phase 7: Tooling                     │
  T7.1 Map Editor ◄───────── T2.1, T3.1
  T7.2 Connection Editor ◄── T3.1
  T7.3 Animation Viewer ◄─── T1.3
  T7.4 Dialogue Tester ◄──── T3.4
  T7.5 Plant Previewer ◄──── T5.5
```

---

## Phase 1: Foundation

### T1.1 — Workspace Setup

**Deliverable**: Rust workspace compiles with all crate stubs.

**Steps**:
1. Create root `Cargo.toml` with workspace members
2. Create all crate directories with stub `Cargo.toml` and `lib.rs`/`main.rs`
3. Add Fyrox dependency to `game` crate
4. Add `serde`, `serde_derive` to `core` crate
5. Verify `cargo build` succeeds (with stubs)
6. Set up `assets/`, `data/`, `docs/` directory structure

**Acceptance**: `cargo build --workspace` succeeds. All crate stubs compile.

**Dependencies**: None

**Estimated Complexity**: Low

---

### T1.2 — Core Types & Components

**Deliverable**: Shared type definitions used across all crates.

**Steps**:
1. Define `Attributes`, `DerivedStats`, `DamageDice` in `crates/core/src/stats.rs`
2. Define `Interactable`, `InteractionType` components in `crates/core/src/components.rs`
3. Define base `Item`, `ItemType`, `ItemStack` in `crates/core/src/items.rs`
4. Define `GameConfig` in `crates/core/src/config.rs`
5. Add `Serialize`/`Deserialize` derives to all types
6. Write unit tests for stat modifier calculations

**Acceptance**: All types compile, serialize/deserialize round-trip tests pass.

**Dependencies**: T1.1

**Estimated Complexity**: Low

**Key Reference**: [03-combat-party.md § Character Stats](03-combat-party.md#character-stats-dnd-analogous), [04-apothecary-botany.md § Inventory System](04-apothecary-botany.md#inventory-system)

---

### T1.3 — Fyrox Game Plugin Shell

**Deliverable**: Game launches a Fyrox window with an empty scene.

**Steps**:
1. Implement `GamePlugin` in `crates/game/src/app.rs`
2. Define `GameState` enum: `Menu`, `Hub`, `Dungeon`, `Combat`, `Garden`
3. Implement `main.rs` that initializes Fyrox engine with `GamePlugin`
4. Create empty scene on startup
5. Verify window opens and renders a blank scene

**Acceptance**: `cargo run --bin game` opens a window with Fyrox rendering. No crashes. Logs show game state initialization.

**Dependencies**: T1.1

**Estimated Complexity**: Low

**Key Reference**: [01-engine-rendering.md § Fyrox Integration](01-engine-rendering.md#fyrox-integration)

---

## Phase 2: Core Systems

### T2.1 — Isometric Camera

**Deliverable**: Orthographic isometric camera that follows the player with zoom.

**Steps**:
1. Create `crates/game/src/camera.rs`
2. Implement orthographic camera with 45° Y-rotation, 30° downward pitch
3. Add smooth follow (lerp toward player position)
4. Add scroll-wheel zoom (adjust orthographic `vertical_size`)
5. Add map bounds clamping
6. Test with a placeholder cube as "player"

**Acceptance**: Camera shows correct isometric angle. Follows a moving object smoothly. Zoom works. Stays within defined bounds.

**Dependencies**: T1.3

**Estimated Complexity**: Medium

**Key Reference**: [01-engine-rendering.md § Isometric Camera](01-engine-rendering.md#isometric-camera)

---

### T2.2 — Navigation Mesh

**Deliverable**: Navmesh loading and A* pathfinding.

**Steps**:
1. Create `crates/navigation/src/navmesh.rs`
2. Implement navmesh loading from Fyrox scene node (named "navmesh")
3. Implement A* pathfinding over navmesh polygons
4. Implement string-pulling path smoothing (funnel algorithm)
5. Implement `is_walkable(point)` ground query
6. Write unit tests with simple test navmeshes

**Acceptance**: Given a navmesh and two points, produces a smooth walkable path. `is_walkable` correctly rejects points off the mesh.

**Dependencies**: T1.3

**Estimated Complexity**: High

**Key Reference**: [02-world-navigation.md § Navigation Mesh](02-world-navigation.md#navigation-mesh)

---

### T2.3 — Player Movement (WASD + Click-to-Move)

**Deliverable**: Player character moves via keyboard and mouse click.

**Steps**:
1. Create `crates/navigation/src/input.rs`
2. Implement `MovementCommand` enum (`NavigateTo`, `DirectMove`, `Stop`)
3. Implement click-to-move: raycast → navmesh check → pathfind → follow path
4. Implement WASD: camera-relative direction → direct velocity → navmesh clamping
5. Implement input mode switching (click cancels WASD, WASD cancels click path)
6. Hook into animation system (idle/walk/run based on velocity)
7. Test with a character model on a simple navmesh

**Acceptance**: Player moves smoothly with WASD. Click-to-move pathfinds correctly. Inputs override each other cleanly. Character doesn't walk through walls or off navmesh.

**Dependencies**: T2.1, T2.2

**Estimated Complexity**: High

**Key Reference**: [02-world-navigation.md § Player Movement](02-world-navigation.md#player-movement)

---

### T2.4 — Interaction System (Hover + Click)

**Deliverable**: Mouse-over highlights interactable objects; clicking triggers interaction.

**Steps**:
1. Create `crates/navigation/src/interaction.rs`
2. Implement per-frame mouse raycast through orthographic camera
3. Implement `Interactable` component query on hit nodes
4. Implement material-swap highlighting (emission boost on hover)
5. Implement click handler with range check (navigate if too far, interact if close)
6. Implement interaction dispatch (pickup, examine, exit, NPC, garden_plot)
7. Add tooltip rendering for hovered object names

**Acceptance**: Hovering over an interactable object highlights it and shows tooltip. Clicking interacts (or navigates then interacts). Non-interactable objects are ignored.

**Dependencies**: T2.1, T1.2

**Estimated Complexity**: Medium

**Key Reference**: [01-engine-rendering.md § Interactable Object Highlighting](01-engine-rendering.md#interactable-object-highlighting), [02-world-navigation.md § Item Interaction](02-world-navigation.md#item-interaction)

---

### T2.5 — Stat System Implementation

**Deliverable**: Full stat calculation, modifier system, and status effects.

**Steps**:
1. Implement `Attributes::modifier()` in `crates/core/src/stats.rs`
2. Implement `DerivedStats::calculate(attributes, level, class)`
3. Implement `StatusEffect` enum with all buff/debuff variants
4. Implement `ActiveStatusEffect` with duration tracking
5. Implement `tick_effects()` — decrement durations, apply per-turn effects, remove expired
6. Implement `apply_effect()` and `remove_effect()` on derived stats
7. Write comprehensive unit tests for stat calculations and effect stacking

**Acceptance**: Stats calculate correctly from attributes. Status effects apply, tick, and expire. Stacking rules work (same effect type refreshes duration, doesn't double-stack amount).

**Dependencies**: T1.2

**Estimated Complexity**: Medium

**Key Reference**: [03-combat-party.md § Character Stats](03-combat-party.md#character-stats-dnd-analogous), [03-combat-party.md § Status Effects](03-combat-party.md#status-effects)

---

### T2.6 — Inventory System

**Deliverable**: Generic inventory container with add/remove/query operations.

**Steps**:
1. Implement `Inventory` in `crates/inventory/src/container.rs`
2. Implement `add_item()`, `remove_item()`, `has_item()`, `get_count()`
3. Implement stack merging for stackable items
4. Implement max slot enforcement
5. Implement `ItemType` enum with all variants (PlantSample, Potion, Medicine, etc.)
6. Write unit tests for all inventory operations

**Acceptance**: Items can be added, removed, stacked, and queried. Slot limits enforced. Serializes correctly.

**Dependencies**: T1.2

**Estimated Complexity**: Low

**Key Reference**: [04-apothecary-botany.md § Inventory System](04-apothecary-botany.md#inventory-system)

---

## Phase 3: Content Systems

### T3.1 — World Graph & Location System

**Deliverable**: World graph loads from RON and provides location/exit queries.

**Steps**:
1. Implement `LocationDef`, `ExitDef`, `SpawnPoint` in `crates/world/src/location.rs`
2. Implement `WorldGraph` in `crates/world/src/map_graph.rs`
3. Implement RON loading with `ron` crate
4. Implement validation (no dangling refs, all exits have matching spawns)
5. Create sample `data/locations.ron` with hub + garden + 2 dungeon floors
6. Write unit tests for graph queries and validation

**Acceptance**: World graph loads from RON, validates successfully. Queries return correct locations and exits. Validation catches intentional errors.

**Dependencies**: T1.2

**Estimated Complexity**: Low

**Key Reference**: [02-world-navigation.md § World Graph](02-world-navigation.md#world-graph)

---

### T3.2 — Scene Transitions

**Deliverable**: Player can move between locations via exits.

**Steps**:
1. Implement `SceneTransitionManager` in `crates/world/src/transitions.rs`
2. Implement exit trigger detection (player enters trigger volume)
3. Implement async scene loading with fade overlay
4. Implement player/party state transfer between scenes
5. Implement spawn point placement at target location
6. Implement entity spawning from placement data in `crates/world/src/spawning.rs`
7. Test full transition flow: hub → dungeon → hub

**Acceptance**: Walking into an exit triggers scene transition. New scene loads with correct player placement. Party state persists across transitions. Fade in/out is smooth.

**Dependencies**: T3.1, T2.1

**Estimated Complexity**: High

**Key Reference**: [02-world-navigation.md § Scene Transitions](02-world-navigation.md#scene-transitions)

---

### T3.3 — YarnSpinner Parser

**Deliverable**: Parser that reads `.yarn` files into structured AST.

**Steps**:
1. Create `crates/dialogue/src/parser.rs`
2. Define AST types: `YarnNode`, `YarnStatement`, `YarnExpression`, `YarnValue`
3. Implement parser (consider `nom` or `pest` — pest recommended for readability)
4. Handle: node headers, dialogue lines, choices with conditions, commands, jumps, set, if/else/endif
5. Handle variable interpolation `{$var_name}` in text
6. Write tests with sample `.yarn` files covering all syntax features

**Acceptance**: Parses official YarnSpinner syntax. All test `.yarn` files parse correctly. Parser errors include line numbers and descriptions.

**Dependencies**: None (standalone crate)

**Estimated Complexity**: High

**Key Reference**: [05-dialogue-scripting-persistence.md § Parser](05-dialogue-scripting-persistence.md#parser)

---

### T3.4 — Dialogue Runner

**Deliverable**: State machine that executes parsed Yarn dialogue trees.

**Steps**:
1. Implement `DialogueRunner` in `crates/dialogue/src/runner.rs`
2. Implement state machine: Idle → ShowingLine → WaitingForChoice → ExecutingCommand → Finished
3. Implement variable storage with get/set
4. Implement choice filtering (evaluate conditions, disable unavailable choices)
5. Implement `CommandRegistry` in `crates/dialogue/src/commands.rs`
6. Register built-in commands: give_item, take_item, set_flag, jump
7. Write integration tests: load yarn → run through dialogue → verify variable state

**Acceptance**: Dialogue advances correctly through lines and choices. Variables track state. Commands dispatch. Conditional choices are filtered correctly.

**Dependencies**: T3.3

**Estimated Complexity**: Medium

**Key Reference**: [05-dialogue-scripting-persistence.md § Dialogue Runner](05-dialogue-scripting-persistence.md#dialogue-runner)

---

### T3.5 — Lua Scripting Integration

**Deliverable**: Sandboxed Lua VM with game API bindings and hot-reload.

**Steps**:
1. Add `mlua` dependency to `crates/scripting/Cargo.toml`
2. Implement `ScriptingEngine` in `crates/scripting/src/vm.rs`
3. Sandbox: remove `os`, `io`, `debug` modules
4. Implement game API bindings in `crates/scripting/src/bindings.rs`:
   - Entity API (get_player, get_entity, spawn, destroy)
   - Combat API (start_encounter, apply_damage, apply_status)
   - Inventory API (add_item, remove_item, has_item)
   - World API (transition_to, get_location, set_flag, get_flag)
   - Dialogue API (start_dialogue, is_active)
   - UI API (show_notification, show_tooltip)
5. Implement event hook system (on_enter_zone, on_combat_start, on_plant_mature, etc.)
6. Implement hot-reload with `notify` crate in `crates/scripting/src/hot_reload.rs`
7. Write sample scripts and test API calls

**Acceptance**: Lua scripts load and execute. API calls affect game state. Sandboxing prevents dangerous operations. Hot-reload picks up file changes in dev mode.

**Dependencies**: T1.2

**Estimated Complexity**: High

**Key Reference**: [05-dialogue-scripting-persistence.md § Scripting System](05-dialogue-scripting-persistence.md#scripting-system-lua)

---

## Phase 4: Gameplay Systems

### T4.1 — Party Generation & Management

**Deliverable**: Procedural party member creation, recruitment, and permadeath.

**Steps**:
1. Implement `generate_party_member()` in `crates/party/src/generation.rs`
   - Name from tables, class random, stats `3d6` with class minimums
   - Personality random (aggression, caution, team_focus, item_affinity)
   - Appearance data selection
   - Backstory template assembly
2. Implement recruitment pool (3–6 members) in `crates/party/src/recruitment.rs`
3. Implement active roster management in `crates/party/src/roster.rs`
4. Implement permadeath handling in `crates/party/src/permadeath.rs`
5. Implement pool refresh after dungeon runs
6. Write unit tests for generation distributions and roster operations

**Acceptance**: Generated party members have valid stats and varied personalities. Recruitment pool works. Dead members are permanently removed. Equipment transfers to inventory on death.

**Dependencies**: T2.5

**Estimated Complexity**: Medium

**Key Reference**: [03-combat-party.md § Party Generation](03-combat-party.md#party-generation), [03-combat-party.md § Permadeath](03-combat-party.md#permadeath)

---

### T4.2 — Combat Turn Manager

**Deliverable**: Turn-based combat engine with initiative, phases, and resolution.

**Steps**:
1. Implement `CombatState` and `CombatPhase` in `crates/combat/src/turn_manager.rs`
2. Implement initiative rolling (d20 + DEX_mod)
3. Implement turn ordering (sorted by initiative, ties broken by DEX)
4. Implement phase transitions: RollInitiative → TurnStart → ActionSelection → ActionExecution → TurnEnd → RoundEnd
5. Implement status effect ticking at TurnEnd
6. Implement death checking (HP ≤ 0)
7. Implement victory (all enemies dead) and defeat (all party dead) detection
8. Write tests for turn ordering, phase flow, and victory/defeat conditions

**Acceptance**: Combat proceeds through correct turn order. Phases transition correctly. Status effects tick. Death is detected. Victory/defeat resolves.

**Dependencies**: T2.5, T4.1

**Estimated Complexity**: High

**Key Reference**: [03-combat-party.md § Turn Structure](03-combat-party.md#turn-structure)

---

### T4.3 — Combat AI (Party Members + Enemies)

**Deliverable**: Autonomous action selection for party members and enemies.

**Steps**:
1. Implement `select_action()` in `crates/combat/src/ai.rs`
2. Implement danger evaluation (self HP%, ally HP%, enemy threat)
3. Implement personality-weighted decision tree:
   - High danger + cautious → defend/heal self
   - Ally in danger + team-focused → support
   - High aggression → strongest attack
   - Default → balanced
4. Implement class-specific action pools (warrior can't cast spells, etc.)
5. Implement enemy AI from template data (weighted action selection + targeting rules)
6. Write tests verifying AI selects reasonable actions for given states

**Acceptance**: Party members make contextually appropriate decisions. Personality affects behavior noticeably. Enemies follow their template priorities. No invalid actions selected (class respects ability pool).

**Dependencies**: T4.2

**Estimated Complexity**: High

**Key Reference**: [03-combat-party.md § Party Member AI](03-combat-party.md#party-member-ai)

---

### T4.4 — Player Combat Actions (Apothecary)

**Deliverable**: Player can use/give items and examine enemies during combat.

**Steps**:
1. Implement `PlayerAction` enum in `crates/combat/src/actions.rs`
2. Implement `UseItem`: select from inventory → select target → apply item effects via status system
3. Implement `GiveItem`: transfer consumable to party member
4. Implement `Examine`: INT check → reveal enemy stats/weaknesses
5. Implement `Wait`: skip turn
6. Implement combat UI for action/target selection
7. Test all player actions with various items and targets

**Acceptance**: Player can use potions to heal allies, throw harmful potions at enemies, give items to party members, examine enemies (with success/fail based on INT), and wait. Effects apply correctly.

**Dependencies**: T4.2, T2.6

**Estimated Complexity**: Medium

**Key Reference**: [03-combat-party.md § Player Actions](03-combat-party.md#player-actions-apothecary)

---

### T4.5 — Crafting System

**Deliverable**: Recipe-based potion/medicine crafting from plant ingredients.

**Steps**:
1. Implement `Recipe` and `RecipeCategory` in `crates/inventory/src/crafting.rs`
2. Implement recipe loading from `data/recipes.ron`
3. Implement recipe resolution:
   - Match ingredients to slots
   - Extract genetics from plant ingredients
   - Map genetics to alchemy effects (via `botany::stat_mapping`)
   - Combine effects based on recipe type
   - Generate result item with name, effects, quality
4. Implement known-recipe tracking (player discovers recipes)
5. Write tests for recipe resolution with various ingredient combinations

**Acceptance**: Crafting consumes ingredients and produces items with correct effects derived from plant genetics. Unknown recipes are not available. Quality varies based on ingredient potency.

**Dependencies**: T2.6 (T5.1 and T5.6 needed for full genetics integration but can stub initially)

**Estimated Complexity**: Medium

**Key Reference**: [04-apothecary-botany.md § Crafting / Alchemy System](04-apothecary-botany.md#crafting--alchemy-system)

---

## Phase 5: Botany

### T5.1 — Genetics System

**Deliverable**: Plant genotype representation with Mendelian genetics and crossover.

**Steps**:
1. Implement `Gene`, `Dominance`, `PlantGenotype` in `crates/botany/src/genetics.rs`
2. Implement `Gene::express()` for each dominance type
3. Implement `crossover()` between two parent genotypes
4. Implement mutation (5% per allele, small perturbation)
5. Implement random genotype generation for wild plants
6. Write unit tests:
   - Expression correctness for each dominance type
   - Mendelian ratios over large sample crossovers
   - Mutation rate verification
   - Serialization round-trip

**Acceptance**: Genes express correctly. Crossover follows Mendelian inheritance. Mutation rate is ~5%. Wild plants have reasonable random distributions.

**Dependencies**: None (standalone)

**Estimated Complexity**: Medium

**Key Reference**: [04-apothecary-botany.md § Plant Genetics System](04-apothecary-botany.md#plant-genetics-system)

---

### T5.2 — Phenotype Expression

**Deliverable**: Pure function mapping genotype to visual/structural parameters.

**Steps**:
1. Implement `PlantPhenotype` struct in `crates/botany/src/phenotype.rs`
2. Implement `express_phenotype(genotype) → PlantPhenotype`
3. Map each gene to its visual parameter with appropriate range mapping
4. Handle discrete mappings (leaf shape index, petal count: 3/4/5/6/8)
5. Handle color via HSV with hue/saturation from genes
6. Write tests: same genotype → same phenotype (determinism)

**Acceptance**: Phenotype is deterministic from genotype. All parameters fall within valid ranges. Visual variation is apparent across different genotypes.

**Dependencies**: T5.1

**Estimated Complexity**: Low

**Key Reference**: [04-apothecary-botany.md § Phenotype Expression](04-apothecary-botany.md#phenotype-expression)

---

### T5.3 — L-System Engine

**Deliverable**: Parametric, stochastic L-system string rewriting engine.

**Steps**:
1. Implement `LSymbol` enum in `crates/botany/src/lsystem.rs`
2. Implement `ProductionRule` with predecessor/successor/probability
3. Implement `LSystem` with axiom and rules
4. Implement `derive(iterations, rng)` — iterative rule application
5. Implement `from_phenotype()` — build L-system rules from phenotype parameters
6. Support stochastic rules (probability < 1.0 → apply randomly)
7. Support parameterized symbols (angle/length values embedded in symbols)
8. Write tests:
   - Simple known L-system (Sierpinski, fractal plant) produces expected strings
   - Determinism with same RNG seed
   - Iteration count affects string length correctly

**Acceptance**: L-system produces correct strings for known grammars. Stochastic rules apply probabilistically. Phenotype parameters correctly parameterize the rules.

**Dependencies**: None (standalone)

**Estimated Complexity**: High

**Key Reference**: [04-apothecary-botany.md § L-System Engine](04-apothecary-botany.md#l-system-procedural-plant-generation)

---

### T5.4 — Turtle Interpreter

**Deliverable**: 3D turtle that interprets L-system strings into mesh placement data.

**Steps**:
1. Implement `TurtleState` in `crates/botany/src/turtle.rs`
2. Implement `TurtleInterpreter` with state stack (push/pop for branching)
3. Implement 3D rotations (turn left/right, pitch up/down, roll)
4. Implement `Forward` → record stem segment (start, end, width)
5. Implement `Leaf`, `Flower`, `Fruit` → record instance placement
6. Implement `Width` → adjust current stem width
7. Write tests with known L-system inputs → verify vertex positions

**Acceptance**: Turtle correctly interprets all symbol types. Push/pop correctly saves/restores state. Stem segments connect correctly. Organ placements are at correct positions and orientations.

**Dependencies**: T5.3

**Estimated Complexity**: Medium

**Key Reference**: [04-apothecary-botany.md § Turtle Interpretation](04-apothecary-botany.md#turtle-interpretation--3d-mesh)

---

### T5.5 — Plant Mesh Generation

**Deliverable**: Complete pipeline from genotype → L-system → turtle → Fyrox scene nodes.

**Steps**:
1. Implement `PlantMeshBuilder` in `crates/botany/src/mesh_gen.rs`
2. Implement `add_stem_segment()` — generalized cylinder between two points
3. Implement `add_leaf_instance()` — instanced mesh with transform and color
4. Implement `add_flower_instance()` — petal arrangement (radial placement)
5. Implement `add_fruit_instance()` — instanced mesh placement
6. Implement `build() → PlantMeshData`
7. Implement `to_scene_nodes()` — convert to Fyrox nodes with materials
8. Create base template meshes in `assets/models/plants/` (simple leaf, petal, fruit shapes)
9. Integration test: genotype → express → L-system → turtle → mesh → render

**Acceptance**: End-to-end pipeline produces visible 3D plants. Different genotypes produce visibly different plants. Plants render correctly in Fyrox scene.

**Dependencies**: T5.2, T5.4

**Estimated Complexity**: High

**Key Reference**: [04-apothecary-botany.md § Mesh Construction](04-apothecary-botany.md#mesh-construction)

---

### T5.6 — Genetics → Stat Mapping

**Deliverable**: Mapping from hidden plant genetics to alchemy/gameplay effects.

**Steps**:
1. Implement `genetics_to_effects()` in `crates/botany/src/stat_mapping.rs`
2. Map `healing_affinity` → heal vs damage
3. Map `potency` → effect strength
4. Map `stat_target` → which attribute is affected
5. Map `duration_gene` → effect duration (1–5 turns)
6. Map `toxicity` → side effect severity
7. Make mapping configurable via `data/plant_genetics.ron`
8. Write tests verifying effect distributions across random genotypes

**Acceptance**: Plants with high healing_affinity produce healing effects. Potency scales effect amounts. Stat target selects the correct attribute. Toxicity adds side effects. Mapping is data-driven.

**Dependencies**: T5.1, T2.5

**Estimated Complexity**: Low

**Key Reference**: [04-apothecary-botany.md § Genotype → Alchemy Effect Mapping](04-apothecary-botany.md#genotype--alchemy-effect-mapping)

---

### T5.7 — Garden System

**Deliverable**: Garden plot management with growth simulation and breeding.

**Steps**:
1. Implement `Garden`, `GardenPlot`, `PlotState` in `crates/garden/src/plots.rs`
2. Implement growth simulation in `crates/garden/src/growth.rs`:
   - Growth rate per cycle (affected by genetics, watering)
   - Health tracking (watering, pests, vigor)
   - Maturity detection
3. Implement breeding in `crates/garden/src/breeding.rs`:
   - Select two adjacent mature plants
   - Call `genetics::crossover()` to produce child genotype
   - Produce seed item with child genotype
4. Implement harvesting (mature plant → plant sample item with genotype)
5. Implement garden plot unlocking/upgrading
6. Write tests for growth simulation and breeding flows

**Acceptance**: Plants grow over cycles. Watering and genetics affect growth. Mature plants can be harvested or bred. Bred offspring have genotypes that follow Mendelian inheritance from parents.

**Dependencies**: T5.1, T5.5

**Estimated Complexity**: Medium

**Key Reference**: [04-apothecary-botany.md § Garden System](04-apothecary-botany.md#garden-system)

---

## Phase 6: Persistence & Integration

### T6.1 — Save System

**Deliverable**: Game state serialized to MessagePack save files.

**Steps**:
1. Define `SaveFile`, `SaveData`, and sub-structs in `crates/persistence/src/save.rs`
2. Define `Saveable` trait
3. Implement `Saveable` for all game systems (Party, Inventory, Garden, World, DialogueRunner)
4. Implement save collection (gather data from all systems)
5. Implement MessagePack serialization via `rmp-serde`
6. Implement CRC32 checksum
7. Implement file I/O with error handling
8. Write tests: save → file → load → compare

**Acceptance**: All game state serializes to a compact binary file. Checksum validates. File I/O handles errors gracefully.

**Dependencies**: All Phase 4 systems, T5.7

**Estimated Complexity**: Medium

**Key Reference**: [05-dialogue-scripting-persistence.md § Save/Load System](05-dialogue-scripting-persistence.md#saveload-system)

---

### T6.2 — Load System

**Deliverable**: Save files deserialize and restore full game state.

**Steps**:
1. Implement `load_save_file()` in `crates/persistence/src/load.rs`
2. Verify magic bytes and version
3. Implement `MigrationChain` in `crates/persistence/src/versioning.rs`
4. Implement state distribution to all game systems via `Saveable::load()`
5. Rebuild scenes (load location, place player, regenerate plant meshes)
6. Implement autosave triggers (location transition, combat resolution, garden actions)
7. Test full save → quit → load → resume cycle

**Acceptance**: Loading a save file restores the game to exact prior state. Version migration works for older saves. Autosaves trigger at correct moments.

**Dependencies**: T6.1

**Estimated Complexity**: Medium

---

### T6.3 — Hub Location Integration

**Deliverable**: Playable hub with recruitment, garden access, and dungeon entry.

**Steps**:
1. Create hub scene with basic environment meshes
2. Place recruitment NPC (triggers recruitment UI)
3. Place garden entrance (transitions to garden sub-scene)
4. Place dungeon entrance (transitions to first dungeon floor)
5. Place crafting station (opens crafting UI)
6. Integrate all hub interactions: recruitment → party system, garden → garden system, crafting → crafting system
7. Test full loop: hub → dungeon → combat → return → craft → garden

**Acceptance**: Player can recruit, access garden, enter dungeons, and craft from the hub. All transitions work. Full gameplay loop is functional.

**Dependencies**: T3.2, T4.1, T5.7

**Estimated Complexity**: High

---

### T6.4 — Game UI / HUD

**Deliverable**: In-game UI for inventory, party status, combat, crafting, garden, and dialogue.

**Steps**:
1. Implement HUD overlay (party HP bars, minimap indicator, notification area)
2. Implement inventory screen (grid display, item inspection, use/give)
3. Implement combat UI (turn indicator, action menu, target selection, HP bars)
4. Implement crafting UI (recipe list, ingredient slots, brew button)
5. Implement garden UI (plot grid, plant info, breed/harvest buttons)
6. Implement dialogue UI (text box, choice buttons, speaker portrait)
7. Implement recruitment UI (candidate list, stats display, recruit/dismiss)
8. Use Fyrox UI framework for all screens

**Acceptance**: All UI screens are functional and readable. Information displays correctly. Player can perform all game actions through UI.

**Dependencies**: T2.6, T4.2

**Estimated Complexity**: Very High

---

## Phase 7: Tooling

### T7.1 — Map Editor

**Deliverable**: Tool for placing meshes in location scenes.

**Steps**:
1. Create `crates/tools/src/bin/map_editor.rs`
2. Implement Fyrox application with isometric viewport
3. Implement asset browser (scan `assets/models/` recursively)
4. Implement drag-and-drop mesh placement with ground-plane raycasting
5. Implement transform gizmos (translate, rotate, scale)
6. Implement scene hierarchy panel
7. Implement properties panel with component editing
8. Implement grid snapping
9. Implement undo/redo (command pattern)
10. Implement `.placement.ron` save/load
11. Test: place objects → save → load → verify positions match

**Acceptance**: Can place, move, rotate, and delete objects. Can add components (Interactable, SpawnPoint). Saves and loads correctly. Undo/redo works.

**Dependencies**: T2.1, T3.1

**Estimated Complexity**: Very High

**Key Reference**: [06-editor-tooling.md § Map Editor](06-editor-tooling.md#map-editor)

---

### T7.2 — Connection Editor

**Deliverable**: 2D graph editor for world location connectivity.

**Steps**:
1. Create `crates/tools/src/bin/connection_editor.rs`
2. Implement 2D canvas with pan/zoom
3. Implement node rendering (location boxes)
4. Implement edge rendering (connection lines with arrows)
5. Implement drag-to-create connections
6. Implement property panel for location and exit editing
7. Implement validation (highlight errors)
8. Implement RON save/load of `data/locations.ron`

**Acceptance**: Can create, connect, and configure locations visually. Validation catches errors. Saves valid RON files that the game loads correctly.

**Dependencies**: T3.1

**Estimated Complexity**: High

**Key Reference**: [06-editor-tooling.md § Connection Editor](06-editor-tooling.md#connection-editor)

---

### T7.3 — Animation Viewer

**Deliverable**: Preview tool for character/creature animations.

**Steps**:
1. Create `crates/tools/src/bin/animation_viewer.rs`
2. Implement model loading (file open dialog → load `.glb`/`.fbx`)
3. Implement animation clip enumeration from loaded model
4. Implement clip list panel with play-on-select
5. Implement playback controls (play, pause, stop, scrub, speed, loop toggle)
6. Implement orbit camera for model inspection
7. Optional: skeleton/bone debug rendering

**Acceptance**: Can load any model file and play its embedded animations. Playback controls work. Can inspect model from all angles.

**Dependencies**: T1.3

**Estimated Complexity**: Medium

**Key Reference**: [06-editor-tooling.md § Animation Viewer](06-editor-tooling.md#animation-viewer)

---

### T7.4 — Dialogue Tester

**Deliverable**: Interactive dialogue testing tool.

**Steps**:
1. Create `crates/tools/src/bin/dialogue_tester.rs`
2. Implement yarn file loading
3. Implement node browser panel
4. Implement dialogue display (lines with speaker, choices as buttons)
5. Implement variable inspector (view and edit all Yarn variables)
6. Implement command log panel
7. Test with sample dialogue files

**Acceptance**: Can load `.yarn` files and step through dialogue interactively. Choices work. Variables display and can be edited. Commands are logged.

**Dependencies**: T3.4

**Estimated Complexity**: Medium

**Key Reference**: [06-editor-tooling.md § Dialogue Tester](06-editor-tooling.md#dialogue-tester)

---

### T7.5 — Plant Previewer

**Deliverable**: Real-time plant generation previewer with gene parameter sliders.

**Steps**:
1. Create `crates/tools/src/bin/plant_previewer.rs`
2. Implement gene slider panel (all allele values adjustable)
3. Implement real-time phenotype recalculation on slider change
4. Implement L-system → turtle → mesh regeneration on change
5. Implement 3D viewport with orbit camera showing generated plant
6. Implement phenotype readout panel
7. Implement alchemy effect readout panel
8. Implement genotype RON export
9. Optional: breeding simulation (load two parents, view offspring range)

**Acceptance**: Adjusting gene sliders produces visible changes in the 3D plant. Phenotype and alchemy readouts update. Export produces valid RON.

**Dependencies**: T5.5

**Estimated Complexity**: High

**Key Reference**: [06-editor-tooling.md § Plant Previewer](06-editor-tooling.md#plant-previewer)

---

## Summary Table

| Task | Phase | Complexity | Dependencies | Parallelizable With |
|------|-------|-----------|--------------|---------------------|
| T1.1 Workspace Setup | 1 | Low | — | — |
| T1.2 Core Types | 1 | Low | T1.1 | T1.3 |
| T1.3 Fyrox Plugin | 1 | Low | T1.1 | T1.2 |
| T2.1 Isometric Camera | 2 | Med | T1.3 | T2.2, T2.5, T2.6 |
| T2.2 Navigation Mesh | 2 | High | T1.3 | T2.1, T2.5, T2.6 |
| T2.3 Player Movement | 2 | High | T2.1, T2.2 | T2.4 |
| T2.4 Interaction System | 2 | Med | T2.1, T1.2 | T2.3 |
| T2.5 Stat System | 2 | Med | T1.2 | T2.1, T2.2, T2.6 |
| T2.6 Inventory System | 2 | Low | T1.2 | T2.1, T2.2, T2.5 |
| T3.1 World Graph | 3 | Low | T1.2 | T3.3, T3.5 |
| T3.2 Scene Transitions | 3 | High | T3.1, T2.1 | T3.4 |
| T3.3 YarnSpinner Parser | 3 | High | — | T3.1, T3.5, T5.1, T5.3 |
| T3.4 Dialogue Runner | 3 | Med | T3.3 | T3.2 |
| T3.5 Lua Scripting | 3 | High | T1.2 | T3.1, T3.3 |
| T4.1 Party Generation | 4 | Med | T2.5 | T4.5 |
| T4.2 Combat Turn Manager | 4 | High | T2.5, T4.1 | — |
| T4.3 Combat AI | 4 | High | T4.2 | T4.4 |
| T4.4 Player Combat Actions | 4 | Med | T4.2, T2.6 | T4.3 |
| T4.5 Crafting System | 4 | Med | T2.6 | T4.1 |
| T5.1 Genetics System | 5 | Med | — | T5.3, T3.3, T3.5 |
| T5.2 Phenotype Expression | 5 | Low | T5.1 | T5.4 |
| T5.3 L-System Engine | 5 | High | — | T5.1, T3.3, T3.5 |
| T5.4 Turtle Interpreter | 5 | Med | T5.3 | T5.2 |
| T5.5 Plant Mesh Gen | 5 | High | T5.2, T5.4 | — |
| T5.6 Stat Mapping | 5 | Low | T5.1, T2.5 | T5.5 |
| T5.7 Garden System | 5 | Med | T5.1, T5.5 | — |
| T6.1 Save System | 6 | Med | Phase 4, T5.7 | — |
| T6.2 Load System | 6 | Med | T6.1 | T6.3 |
| T6.3 Hub Integration | 6 | High | T3.2, T4.1, T5.7 | T6.2 |
| T6.4 Game UI / HUD | 6 | V.High | T2.6, T4.2 | T6.1 |
| T7.1 Map Editor | 7 | V.High | T2.1, T3.1 | T7.2, T7.3, T7.4 |
| T7.2 Connection Editor | 7 | High | T3.1 | T7.1, T7.3, T7.4 |
| T7.3 Animation Viewer | 7 | Med | T1.3 | T7.1, T7.2, T7.4 |
| T7.4 Dialogue Tester | 7 | Med | T3.4 | T7.1, T7.2, T7.3 |
| T7.5 Plant Previewer | 7 | High | T5.5 | T7.1–T7.4 |

## Recommended Execution Order for Maximum Parallelism

### Sprint 1 (Foundation)
- T1.1 → T1.2 + T1.3 (parallel)

### Sprint 2 (Core — maximum parallel)
- T2.1 + T2.2 + T2.5 + T2.6 (all parallel)
- T3.3 + T5.1 + T5.3 (standalone, can start immediately)

### Sprint 3 (Integration)
- T2.3 (needs T2.1 + T2.2)
- T2.4 (needs T2.1)
- T3.1 + T3.5 (parallel, need T1.2)
- T5.2 (needs T5.1), T5.4 (needs T5.3)
- T3.4 (needs T3.3)

### Sprint 4 (Gameplay)
- T4.1 + T4.5 (parallel, need T2.5/T2.6)
- T3.2 (needs T3.1, T2.1)
- T5.5 (needs T5.2, T5.4)
- T5.6 (needs T5.1, T2.5)

### Sprint 5 (Combat + Garden)
- T4.2 (needs T2.5, T4.1)
- T5.7 (needs T5.1, T5.5)
- T7.3 + T7.4 (tooling, can start early)

### Sprint 6 (Combat Completion + Persistence)
- T4.3 + T4.4 (parallel, need T4.2)
- T6.1 (needs Phase 4 + T5.7)
- T6.4 (UI, needs T2.6, T4.2)

### Sprint 7 (Integration + Tooling)
- T6.2 + T6.3 (parallel)
- T7.1 + T7.2 + T7.5 (tooling, parallel)
