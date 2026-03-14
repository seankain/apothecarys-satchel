# 05 — Dialogue, Scripting & Persistence

## Scope

YarnSpinner dialogue file parsing and runtime, Lua scripting integration, save/load system with versioning.

---

## Dialogue System (YarnSpinner)

### YarnSpinner File Format

Yarn files (`.yarn`) use a simple markup format for dialogue trees:

```yarn
title: Herbalist_Greeting
tags:
---
Herbalist: Welcome to my shop, apothecary. What brings you here today?
-> I'm looking for rare seeds.
    Herbalist: Ah, I may have a few. Let me check my stock.
    <<set $visited_herbalist to true>>
    <<jump Herbalist_SeedShop>>
-> Just browsing.
    Herbalist: Take your time. Mind the nightshade on the left shelf.
-> [if $has_moonpetal] I found this moonpetal in the caves.
    Herbalist: Remarkable! Those haven't been seen in years.
    <<give_item "moonpetal_seeds" 3>>
    <<set $moonpetal_quest_complete to true>>
===
```

### Parser

```rust
// crates/dialogue/src/parser.rs

#[derive(Debug)]
pub struct YarnNode {
    pub title: String,
    pub tags: Vec<String>,
    pub body: Vec<YarnStatement>,
}

#[derive(Debug)]
pub enum YarnStatement {
    /// Plain dialogue line: "Speaker: Text"
    Line {
        speaker: Option<String>,
        text: String,
    },
    /// Player choice: "-> Choice text"
    Choice {
        text: String,
        condition: Option<YarnExpression>,   // [if $var] guard
        body: Vec<YarnStatement>,            // Indented content after choice
    },
    /// Command: <<command args>>
    Command {
        name: String,
        args: Vec<String>,
    },
    /// Jump to another node: <<jump NodeTitle>>
    Jump(String),
    /// Set variable: <<set $var to value>>
    SetVariable {
        name: String,
        value: YarnValue,
    },
    /// Conditional block: <<if $condition>> ... <<endif>>
    Conditional {
        condition: YarnExpression,
        then_body: Vec<YarnStatement>,
        else_body: Option<Vec<YarnStatement>>,
    },
}

#[derive(Debug)]
pub enum YarnValue {
    Bool(bool),
    Number(f64),
    String(String),
}

#[derive(Debug)]
pub enum YarnExpression {
    Variable(String),
    Literal(YarnValue),
    Comparison(Box<YarnExpression>, ComparisonOp, Box<YarnExpression>),
    Not(Box<YarnExpression>),
    And(Box<YarnExpression>, Box<YarnExpression>),
    Or(Box<YarnExpression>, Box<YarnExpression>),
}

pub fn parse_yarn_file(source: &str) -> Result<Vec<YarnNode>>;
```

**Task Goal**: Implement a YarnSpinner parser that handles:
1. Node headers (`title:`, `tags:`, `---`, `===`)
2. Dialogue lines with optional speaker prefix
3. Player choices with optional conditions
4. Commands (`<<set>>`, `<<jump>>`, custom commands)
5. Inline conditionals (`<<if>>`, `<<else>>`, `<<endif>>`)
6. Variable interpolation in text (`{$variable_name}`)

Consider using `nom` or `pest` for parser implementation.

### Dialogue Runner

```rust
// crates/dialogue/src/runner.rs

pub struct DialogueRunner {
    nodes: HashMap<String, YarnNode>,
    variables: HashMap<String, YarnValue>,
    current_node: Option<String>,
    position: usize,            // Current statement index within node
    state: DialogueState,
}

pub enum DialogueState {
    Idle,
    ShowingLine { speaker: Option<String>, text: String },
    WaitingForChoice { choices: Vec<AvailableChoice> },
    ExecutingCommand { command: String, args: Vec<String> },
    Finished,
}

pub struct AvailableChoice {
    pub index: usize,
    pub text: String,
    pub enabled: bool,  // false if condition not met (still shown, greyed out)
}

impl DialogueRunner {
    pub fn load_yarn_files(&mut self, paths: &[&str]) -> Result<()>;
    pub fn start_node(&mut self, title: &str);
    pub fn advance(&mut self) -> DialogueState;
    pub fn select_choice(&mut self, index: usize);
    pub fn get_variable(&self, name: &str) -> Option<&YarnValue>;
    pub fn set_variable(&mut self, name: &str, value: YarnValue);
}
```

### Custom Yarn Commands

Commands like `<<give_item>>` bridge dialogue to game systems:

```rust
// crates/dialogue/src/commands.rs

pub type CommandHandler = Box<dyn Fn(&[String], &mut GameContext) -> Result<()>>;

pub struct CommandRegistry {
    handlers: HashMap<String, CommandHandler>,
}

impl CommandRegistry {
    pub fn register(&mut self, name: &str, handler: CommandHandler);

    // Built-in commands:
    // <<give_item "item_id" count>>     → Add item to player inventory
    // <<take_item "item_id" count>>     → Remove item from inventory
    // <<set_flag "flag_name">>          → Set global game flag
    // <<start_quest "quest_id">>        → Begin quest
    // <<add_recruit "template">>        → Add party member to recruit pool
    // <<play_animation "anim_name">>    → Play NPC animation
    // <<open_shop "shop_id">>           → Open shop UI
}
```

**Task Goal**: Implement the dialogue runner with variable tracking, choice evaluation, and a command dispatch system. Commands registered by game systems at startup.

---

## Scripting System (Lua)

### Why Lua

- `mlua` crate: well-maintained, safe bindings, supports Lua 5.4 and LuaJIT
- Lightweight VM, fast startup
- Widely known scripting language
- Easy to sandbox for modding safety

### Architecture

```rust
// crates/scripting/src/vm.rs

use mlua::prelude::*;

pub struct ScriptingEngine {
    lua: Lua,
}

impl ScriptingEngine {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        // Sandbox: remove dangerous modules
        lua.globals().set("os", mlua::Value::Nil)?;
        lua.globals().set("io", mlua::Value::Nil)?;
        lua.globals().set("debug", mlua::Value::Nil)?;
        Ok(Self { lua })
    }

    pub fn register_api(&self) -> Result<()> {
        // Register game API functions accessible from Lua
        self.register_entity_api()?;
        self.register_combat_api()?;
        self.register_inventory_api()?;
        self.register_dialogue_api()?;
        self.register_world_api()?;
        Ok(())
    }

    pub fn run_script(&self, path: &str) -> Result<()>;
    pub fn call_function(&self, name: &str, args: impl IntoLuaMulti) -> Result<()>;
}
```

### Lua API Surface

```rust
// crates/scripting/src/bindings.rs

// Entity API
// game.get_player() → Player table
// game.get_party_member(id) → PartyMember table
// game.get_entity(name) → Entity table
// game.spawn_entity(template, position) → Entity
// game.destroy_entity(entity)

// Combat API
// combat.start_encounter(enemy_group_id)
// combat.is_in_combat() → bool
// combat.get_combatant(id) → Combatant table
// combat.apply_damage(target, amount)
// combat.apply_status(target, effect_name, duration)

// Inventory API
// inventory.add_item(item_id, count)
// inventory.remove_item(item_id, count)
// inventory.has_item(item_id) → bool
// inventory.get_count(item_id) → number

// World API
// world.transition_to(location_id, spawn_name)
// world.get_current_location() → string
// world.set_flag(name, value)
// world.get_flag(name) → value

// Dialogue API
// dialogue.start(node_title)
// dialogue.is_active() → bool

// UI API
// ui.show_notification(text, duration)
// ui.show_tooltip(text)
```

### Script Usage Examples

```lua
-- scripts/encounters/forest_ambush.lua
function on_enter_zone(zone_name)
    if zone_name == "dark_clearing" and not world.get_flag("forest_ambush_done") then
        ui.show_notification("Something stirs in the undergrowth...", 3.0)
        combat.start_encounter("forest_goblins_group_1")
        world.set_flag("forest_ambush_done", true)
    end
end

-- scripts/garden/special_events.lua
function on_plant_mature(plot_index, plant)
    if plant.generation >= 5 then
        ui.show_notification("This plant has evolved remarkably over generations!", 5.0)
        -- Unlock special recipe
        inventory.add_item("recipe_greater_elixir", 1)
    end
end
```

### Script Hot-Reloading

```rust
// crates/scripting/src/hot_reload.rs

pub struct ScriptWatcher {
    watcher: RecommendedWatcher,  // notify crate
    modified: HashSet<PathBuf>,
}

impl ScriptWatcher {
    /// In dev mode: watch scripts/ directory for changes
    pub fn new(scripts_dir: &Path) -> Result<Self>;

    /// Called each frame; reloads modified scripts
    pub fn poll_and_reload(&mut self, engine: &ScriptingEngine) -> Result<Vec<PathBuf>>;
}
```

**Task Goal**: Implement Lua integration in `crates/scripting/`:
1. Sandboxed Lua VM with game API bindings
2. Script loading from `assets/scripts/`
3. Event hooks (on_enter_zone, on_combat_start, on_plant_mature, etc.)
4. Hot-reloading in dev builds via filesystem watcher

---

## Save/Load System

### Serialization Strategy

| Aspect | Choice |
|--------|--------|
| Format | MessagePack (binary, compact, fast) |
| Library | `rmp-serde` |
| Schema | Rust structs with `#[derive(Serialize, Deserialize)]` |
| Versioning | Integer version number + migration functions |

### Save Data Structure

```rust
// crates/persistence/src/save.rs

pub const SAVE_VERSION: u32 = 1;
pub const SAVE_MAGIC: [u8; 4] = *b"APOT";

#[derive(Serialize, Deserialize)]
pub struct SaveFile {
    pub magic: [u8; 4],
    pub version: u32,
    pub timestamp: u64,           // Unix timestamp
    pub play_time_seconds: f64,
    pub data: SaveData,
    pub checksum: u32,            // CRC32 of serialized data
}

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    // World state
    pub current_location: String,
    pub player_position: [f32; 3],
    pub world_flags: HashMap<String, YarnValue>,
    pub visited_locations: HashSet<String>,

    // Player
    pub player: PlayerSaveData,

    // Party
    pub active_party: Vec<PartyMemberSaveData>,
    pub recruitment_pool: Vec<PartyMemberSaveData>,
    pub dead_members: Vec<String>,   // Names for memorial/log

    // Inventory
    pub inventory: InventorySaveData,
    pub known_recipes: Vec<String>,

    // Garden
    pub garden: GardenSaveData,

    // Dialogue state
    pub dialogue_variables: HashMap<String, YarnValue>,

    // Dungeon state (if saved mid-dungeon)
    pub dungeon_state: Option<DungeonSaveData>,
}

#[derive(Serialize, Deserialize)]
pub struct PlayerSaveData {
    pub name: String,
    pub level: u32,
    pub xp: u32,
    pub attributes: Attributes,
    pub derived: DerivedStats,
}

#[derive(Serialize, Deserialize)]
pub struct GardenSaveData {
    pub plots: Vec<PlotSaveData>,
    pub max_plots: usize,
    pub unlocked_upgrades: Vec<String>,
}

// ... other save data structs
```

### Save Flow

```
Player triggers save (manual or autosave)
         │
         ▼
Collect state from all systems:
    - World system → current location, flags
    - Party system → member data
    - Inventory → item stacks
    - Garden → plot states, plant genotypes
    - Dialogue → variable state
    - Dungeon → room states, enemy positions (if mid-dungeon)
         │
         ▼
Construct SaveData struct
         │
         ▼
Serialize with rmp_serde::to_vec()
         │
         ▼
Calculate CRC32 checksum
         │
         ▼
Write SaveFile (magic + version + data + checksum)
         │
         ▼
Write to disk: saves/slot_{n}.sav
```

### Load Flow

```
Player selects save slot
         │
         ▼
Read file from disk
         │
         ▼
Verify magic bytes
         │
         ▼
Check version number
    │                    │
    ▼ (current)          ▼ (older)
Deserialize directly    Run migration chain:
                        v1 → v2 → ... → current
         │
         ▼
Verify CRC32 checksum
         │
         ▼
Distribute state to all systems:
    - Load scene for current_location
    - Restore player position
    - Rebuild party from save data
    - Restore inventory
    - Restore garden state + regenerate plant meshes
    - Set dialogue variables
    - Restore dungeon state if applicable
```

### Version Migration

```rust
// crates/persistence/src/versioning.rs

pub type MigrationFn = fn(Vec<u8>) -> Result<Vec<u8>>;

pub struct MigrationChain {
    migrations: BTreeMap<u32, MigrationFn>,
}

impl MigrationChain {
    pub fn register(&mut self, from_version: u32, migrate: MigrationFn);

    /// Apply all migrations from saved version to current
    pub fn migrate(&self, data: Vec<u8>, from: u32, to: u32) -> Result<Vec<u8>> {
        let mut current = data;
        for version in from..to {
            if let Some(migration) = self.migrations.get(&version) {
                current = migration(current)?;
            }
        }
        Ok(current)
    }
}
```

### Autosave Strategy

- Autosave on location transition (entering/leaving dungeon)
- Autosave after combat resolution
- Autosave on garden actions
- Keep 1 autosave slot + 3 manual save slots
- Display save timestamp and location thumbnail in load UI

**Task Goal**: Implement save/load in `crates/persistence/`:
1. `SaveData` collection from all game systems (each system implements a `Saveable` trait)
2. MessagePack serialization with CRC32 integrity check
3. Version migration framework
4. File I/O with error handling (corrupted saves, disk full, etc.)
5. Autosave triggers at appropriate game moments

### Saveable Trait

```rust
pub trait Saveable {
    type SaveData: Serialize + DeserializeOwned;
    fn save(&self) -> Self::SaveData;
    fn load(&mut self, data: Self::SaveData) -> Result<()>;
}
```

Each game system (`Party`, `Inventory`, `Garden`, `World`, `DialogueRunner`) implements `Saveable`.

## Key Implementation Files

| File | Purpose |
|------|---------|
| `crates/dialogue/src/parser.rs` | YarnSpinner file parser |
| `crates/dialogue/src/runner.rs` | Dialogue state machine |
| `crates/dialogue/src/commands.rs` | Custom command registry |
| `crates/scripting/src/vm.rs` | Lua VM setup and sandboxing |
| `crates/scripting/src/bindings.rs` | Game API → Lua bindings |
| `crates/scripting/src/hot_reload.rs` | Dev-mode script hot-reload |
| `crates/persistence/src/save.rs` | Save file structure and serialization |
| `crates/persistence/src/load.rs` | Deserialization and state restoration |
| `crates/persistence/src/versioning.rs` | Save version migration chain |
