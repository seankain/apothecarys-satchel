# 09 — Implementation Status & Outstanding Work

A verification pass over the tree against documents 00–08, taken at commit
`bd7a1d5`. Every claim below was checked against the source, not against the
commit log: "shipped" means the code exists *and* something calls it,
"library-only" means the code exists and passes its own tests but nothing in
the game binary reaches it.

The distinction matters, because it is the whole shape of what is left. Almost
every system in documents 02–06 is written and tested. Almost none of it is
plugged into `cargo run --bin game`.

## Baseline

| Check | Result |
|---|---|
| `cargo build --workspace` | clean |
| `cargo test --workspace` | 1146 tests, 0 failures |
| `cargo clippy --workspace --tests -- -D warnings` | clean |
| `cargo fmt --all --check` | **350 hunks across 90 files** |

## Status by phase

| Phase | Verdict |
|---|---|
| 1 — Foundation | Shipped. |
| 2 — Core systems | Camera shipped; navmesh, movement, interaction **library-only**. Stats and inventory shipped as libraries and consumed. |
| 3 — Content systems | World graph, transitions, Yarn parser, dialogue runner, Lua VM all **library-only**. |
| 4 — Gameplay systems | Party generation, turn manager, AI, actions, crafting all **library-only**. |
| 5 — Botany | Shipped, and the only chain that runs end to end — genetics → phenotype → L-system → `plantgl` → Fyrox, plus the browser demo. |
| 6 — Persistence & integration | Save/load and `HubState` **library-only**; 7 of 8 UI modules are headless state machines. |
| 7 — Tooling | Shipped — five Fyrox GUI binaries — except the animation viewer cannot load a model (see G2). |
| 8 — PlantGL port, A–E | Shipped. Phase F (issue #22) is optional and unstarted. |

## Outstanding work

### G1 — The game binary reaches the hub blockout and stops

`crates/game/src/app.rs` runs `Menu → Hub` and nothing else. Specifically:

- `GamePlugin::enter_garden` has no caller anywhere in the workspace, and
  `GameState::Dungeon` and `GameState::Combat` are never entered.
- `crates/game` does not depend on `apothecarys-navigation` or
  `apothecarys-scripting` at all. There is no player entity, no navmesh, no
  click-to-move and no interaction in the running game.
- `IsometricCamera::set_target` and `::zoom` are never called by the plugin.
  T2.1 step 4 (scroll-wheel zoom) has an implementation and no input path.
- `apothecarys-persistence` is a declared dependency of `crates/game` and is
  never referenced. There is no save or load in the game loop, so T6.1 and
  T6.2's acceptance criteria are unmet even though the crate round-trips in
  its own tests.
- `hub::HubState` and `hub::build_default_world()` are never instantiated.
- Only `ui/main_menu.rs` builds Fyrox widgets. `hud`, `inventory_ui`,
  `combat_ui`, `crafting_ui`, `garden_ui`, `dialogue_ui` and `recruitment_ui`
  are plain state machines with no widget construction and no plugin wiring —
  T6.4 is met at the model layer only.

### G2 — There is no asset pipeline

`assets/` does not exist. Nothing in the workspace loads a glTF or FBX through
Fyrox's resource manager. The consequences are visible in three places:

- `data/locations.ron` names `assets/scenes/hub_town.gltf` and friends; none
  of those files are in the tree.
- `crates/tools/src/bin/animation_viewer.rs:251` prints *"model loading
  requires runtime asset manager"* instead of loading the path it was given,
  so T7.3's acceptance criteria cannot be exercised.
- `crates/tools/src/bin/map_editor.rs:318` records every placement as
  `placeholder.glb`.

Document 01 §Asset Pipeline and §Animation System describe a system that has
no code behind it — there is no animation state machine in any shipped crate.

### G3 — The garden → inventory → crafting loop is not closed

This is the game's premise, and it is severed at two joints:

1. **Harvest loses the genotype.** `Garden::harvest()` returns a
   `PlantInstance` carrying its genotype, but there is no conversion from
   `PlantInstance` to an `Item`. `ItemType::PlantSample` is documented as
   *"a sample harvested from a plant, carrying its genotype data"* and
   `ItemType::Seed` as *"a seed carrying genotype data"* — neither variant
   carries any data at all, and `Item` has no genetics field.
2. **Crafting ignores genetics.** `resolve_recipe`'s `ResultType::Dynamic`
   branch (`crates/inventory/src/crafting.rs:214`) is commented *"a stub that
   will be fully implemented when the botany crate is ready"* and emits fixed
   effects by category. `crates/inventory` does not depend on
   `apothecarys-botany`, so `stat_mapping::genetics_to_effects` — which is
   written, tested, and already used by the previewer and the web demo —
   never reaches a crafted potion.

Until both are fixed, breeding a better plant cannot produce a better potion,
which is the loop documents 04 and 03 are built around.

### G4 — The navmesh has no source

T2.2 step 2 asks for navmesh loading from a Fyrox scene node named `navmesh`.
`crates/navigation` has no Fyrox dependency and no loader; `NavMesh::new`
takes hand-built vertices and polygons, which is what every test does. A*,
funnel smoothing and `is_walkable` are all implemented and correct — they just
have nothing to run on in the game.

### G5 — CI covers one crate of sixteen

`.github/workflows/pages.yml` runs `cargo test` and `cargo clippy` for
`apothecarys-web-demo` only, then builds and deploys the page. Document 00
lists CI as `cargo clippy`, `cargo test`, `cargo fmt --check`. Nothing today
would catch a workspace-wide regression in a pull request.

`cargo fmt --all --check` reports 350 hunks across 90 files, so adding the
format gate needs a reformat commit of its own rather than a flag flip.

### G6 — Data lives in Rust, not in `data/`

Document 00 lists five RON files. Only `locations.ron` is in the tree;
`items.ron`, `recipes.ron`, `plant_genetics.ron` and `party_templates.ron` are
absent and the corresponding tables are hardcoded (`RecipeBook::default`,
`party::generation`, `core::items`). `RecipeBook::load_from_ron` and
`WorldGraph::from_ron` both exist and are tested, and nothing calls either at
runtime — `locations.ron` is read only by the `connection_editor` tool.

### G7 — Stale artefacts

- Eight `crates/*/src/stub.rs` files — `combat`, `dialogue`, `navigation`,
  `party`, `persistence`, `scripting`, `tools`, `world` — hold a single
  comment line each and are declared by no `lib.rs`. Removed in this pass.
- The root `Cargo.toml` tells the reader to *"uncomment the `[patch]` section
  below"* to build without ALSA. The section is not commented out. Every
  build in this workspace already links the stub and therefore has no audio,
  and the `libasound2-dev` prerequisite in the README is unnecessary.
  Corrected in this pass.
- Document 00's crate tree omits `mcp-server`, `web-demo` and `alsa-sys-stub`,
  and lists five files that do not exist: `game/src/states/`,
  `navigation/src/pathfinding.rs`, `combat/src/status.rs`,
  `inventory/src/items.rs`, `inventory/src/interaction.rs` and
  `dialogue/src/commands.rs`. The functionality of the last three is present,
  in `core/src/stats.rs`, `inventory/src/container.rs` and
  `dialogue/src/runner.rs` respectively. Corrected in this pass.
- Issues #17–#21 (PlantGL Phases A–E) are open; their work merged in PRs
  #24–#28.

## Suggested order

The dependencies run one way. Assets unblock everything visual; the garden
loop unblocks the game's premise; neither needs the other.

1. **G5** — workspace CI. Cheapest, and it protects everything after it.
2. **G3** — close the garden → crafting loop. Pure library work, no engine,
   no assets, and it makes the botany pipeline count for something.
3. **G2** — asset pipeline and a player model. Unblocks G1 and G4.
4. **G1 + G4** — wire the systems into the plugin, in the order a player
   meets them: player and navigation, then garden, then hub interactions,
   then dungeon and combat, then save/load.
5. **G6** — move the hardcoded tables into `data/` once their shapes have
   stopped moving.

Phase 8F (issue #22) stays optional and off this path.
