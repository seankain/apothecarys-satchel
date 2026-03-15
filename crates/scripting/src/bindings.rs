use mlua::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::vm::ScriptingEngine;

/// Shared game state that Lua scripts can read and modify.
/// This acts as a bridge between the Lua VM and the game engine.
#[derive(Debug, Clone, Default)]
pub struct GameBridge {
    /// World flags (key-value pairs for game state).
    pub flags: Arc<Mutex<HashMap<String, FlagValue>>>,
    /// Inventory operations log (for testing/verification).
    pub inventory_log: Arc<Mutex<Vec<InventoryOp>>>,
    /// Combat operations log.
    pub combat_log: Arc<Mutex<Vec<CombatOp>>>,
    /// World operations log.
    pub world_log: Arc<Mutex<Vec<WorldOp>>>,
    /// Dialogue operations log.
    pub dialogue_log: Arc<Mutex<Vec<DialogueOp>>>,
    /// UI notifications log.
    pub ui_log: Arc<Mutex<Vec<UiOp>>>,
    /// Current location name.
    pub current_location: Arc<Mutex<String>>,
    /// Whether combat is active.
    pub in_combat: Arc<Mutex<bool>>,
}

/// A stored flag value.
#[derive(Debug, Clone, PartialEq)]
pub enum FlagValue {
    Bool(bool),
    Number(f64),
    String(String),
}

/// Logged inventory operation.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryOp {
    pub action: String,
    pub item_id: String,
    pub count: i64,
}

/// Logged combat operation.
#[derive(Debug, Clone, PartialEq)]
pub struct CombatOp {
    pub action: String,
    pub args: Vec<String>,
}

/// Logged world operation.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldOp {
    pub action: String,
    pub args: Vec<String>,
}

/// Logged dialogue operation.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueOp {
    pub action: String,
    pub node: String,
}

/// Logged UI operation.
#[derive(Debug, Clone, PartialEq)]
pub struct UiOp {
    pub action: String,
    pub text: String,
    pub duration: Option<f64>,
}

/// Register all game API bindings into the scripting engine.
pub fn register_all_apis(
    engine: &ScriptingEngine,
    bridge: &GameBridge,
) -> Result<(), mlua::Error> {
    let lua = engine.lua();

    register_world_api(lua, bridge)?;
    register_inventory_api(lua, bridge)?;
    register_combat_api(lua, bridge)?;
    register_dialogue_api(lua, bridge)?;
    register_ui_api(lua, bridge)?;

    Ok(())
}

fn register_world_api(lua: &Lua, bridge: &GameBridge) -> Result<(), mlua::Error> {
    let world_table = lua.create_table()?;

    // world.set_flag(name, value)
    let flags = bridge.flags.clone();
    world_table.set(
        "set_flag",
        lua.create_function(move |_lua, (name, value): (String, LuaValue)| {
            let flag_val = match value {
                LuaValue::Boolean(b) => FlagValue::Bool(b),
                LuaValue::Number(n) => FlagValue::Number(n),
                LuaValue::Integer(n) => FlagValue::Number(n as f64),
                LuaValue::String(s) => FlagValue::String(s.to_str()?.to_string()),
                _ => FlagValue::Bool(true),
            };
            flags.lock().unwrap().insert(name, flag_val);
            Ok(())
        })?,
    )?;

    // world.get_flag(name) -> value
    let flags = bridge.flags.clone();
    world_table.set(
        "get_flag",
        lua.create_function(move |lua, name: String| {
            let flags = flags.lock().unwrap();
            match flags.get(&name) {
                Some(FlagValue::Bool(b)) => Ok(LuaValue::Boolean(*b)),
                Some(FlagValue::Number(n)) => Ok(LuaValue::Number(*n)),
                Some(FlagValue::String(s)) => Ok(LuaValue::String(lua.create_string(s)?)),
                None => Ok(LuaValue::Nil),
            }
        })?,
    )?;

    // world.get_current_location() -> string
    let current_location = bridge.current_location.clone();
    world_table.set(
        "get_current_location",
        lua.create_function(move |_lua, ()| {
            let loc = current_location.lock().unwrap().clone();
            Ok(loc)
        })?,
    )?;

    // world.transition_to(location_id, spawn_name)
    let world_log = bridge.world_log.clone();
    world_table.set(
        "transition_to",
        lua.create_function(move |_lua, (location, spawn): (String, String)| {
            world_log.lock().unwrap().push(WorldOp {
                action: "transition_to".to_string(),
                args: vec![location, spawn],
            });
            Ok(())
        })?,
    )?;

    lua.globals().set("world", world_table)?;
    Ok(())
}

fn register_inventory_api(lua: &Lua, bridge: &GameBridge) -> Result<(), mlua::Error> {
    let inventory_table = lua.create_table()?;

    // inventory.add_item(item_id, count)
    let inv_log = bridge.inventory_log.clone();
    inventory_table.set(
        "add_item",
        lua.create_function(move |_lua, (item_id, count): (String, i64)| {
            inv_log.lock().unwrap().push(InventoryOp {
                action: "add".to_string(),
                item_id,
                count,
            });
            Ok(())
        })?,
    )?;

    // inventory.remove_item(item_id, count)
    let inv_log = bridge.inventory_log.clone();
    inventory_table.set(
        "remove_item",
        lua.create_function(move |_lua, (item_id, count): (String, i64)| {
            inv_log.lock().unwrap().push(InventoryOp {
                action: "remove".to_string(),
                item_id,
                count,
            });
            Ok(())
        })?,
    )?;

    // inventory.has_item(item_id) -> bool
    // For now, check the flags as a proxy
    let flags = bridge.flags.clone();
    inventory_table.set(
        "has_item",
        lua.create_function(move |_lua, item_id: String| {
            let key = format!("has_item_{item_id}");
            let flags = flags.lock().unwrap();
            match flags.get(&key) {
                Some(FlagValue::Bool(b)) => Ok(*b),
                _ => Ok(false),
            }
        })?,
    )?;

    // inventory.get_count(item_id) -> number
    let flags = bridge.flags.clone();
    inventory_table.set(
        "get_count",
        lua.create_function(move |_lua, item_id: String| {
            let key = format!("count_{item_id}");
            let flags = flags.lock().unwrap();
            match flags.get(&key) {
                Some(FlagValue::Number(n)) => Ok(*n),
                _ => Ok(0.0),
            }
        })?,
    )?;

    lua.globals().set("inventory", inventory_table)?;
    Ok(())
}

fn register_combat_api(lua: &Lua, bridge: &GameBridge) -> Result<(), mlua::Error> {
    let combat_table = lua.create_table()?;

    // combat.start_encounter(enemy_group_id)
    let combat_log = bridge.combat_log.clone();
    combat_table.set(
        "start_encounter",
        lua.create_function(move |_lua, group_id: String| {
            combat_log.lock().unwrap().push(CombatOp {
                action: "start_encounter".to_string(),
                args: vec![group_id],
            });
            Ok(())
        })?,
    )?;

    // combat.is_in_combat() -> bool
    let in_combat = bridge.in_combat.clone();
    combat_table.set(
        "is_in_combat",
        lua.create_function(move |_lua, ()| {
            let val = *in_combat.lock().unwrap();
            Ok(val)
        })?,
    )?;

    // combat.apply_damage(target_id, amount)
    let combat_log = bridge.combat_log.clone();
    combat_table.set(
        "apply_damage",
        lua.create_function(move |_lua, (target, amount): (String, f64)| {
            combat_log.lock().unwrap().push(CombatOp {
                action: "apply_damage".to_string(),
                args: vec![target, amount.to_string()],
            });
            Ok(())
        })?,
    )?;

    // combat.apply_status(target_id, effect_name, duration)
    let combat_log = bridge.combat_log.clone();
    combat_table.set(
        "apply_status",
        lua.create_function(
            move |_lua, (target, effect, duration): (String, String, i64)| {
                combat_log.lock().unwrap().push(CombatOp {
                    action: "apply_status".to_string(),
                    args: vec![target, effect, duration.to_string()],
                });
                Ok(())
            },
        )?,
    )?;

    lua.globals().set("combat", combat_table)?;
    Ok(())
}

fn register_dialogue_api(lua: &Lua, bridge: &GameBridge) -> Result<(), mlua::Error> {
    let dialogue_table = lua.create_table()?;

    // dialogue.start(node_title)
    let dialogue_log = bridge.dialogue_log.clone();
    dialogue_table.set(
        "start",
        lua.create_function(move |_lua, node: String| {
            dialogue_log.lock().unwrap().push(DialogueOp {
                action: "start".to_string(),
                node,
            });
            Ok(())
        })?,
    )?;

    // dialogue.is_active() -> bool
    let dialogue_log = bridge.dialogue_log.clone();
    dialogue_table.set(
        "is_active",
        lua.create_function(move |_lua, ()| {
            let active = !dialogue_log.lock().unwrap().is_empty();
            Ok(active)
        })?,
    )?;

    lua.globals().set("dialogue", dialogue_table)?;
    Ok(())
}

fn register_ui_api(lua: &Lua, bridge: &GameBridge) -> Result<(), mlua::Error> {
    let ui_table = lua.create_table()?;

    // ui.show_notification(text, duration)
    let ui_log = bridge.ui_log.clone();
    ui_table.set(
        "show_notification",
        lua.create_function(move |_lua, (text, duration): (String, f64)| {
            ui_log.lock().unwrap().push(UiOp {
                action: "notification".to_string(),
                text,
                duration: Some(duration),
            });
            Ok(())
        })?,
    )?;

    // ui.show_tooltip(text)
    let ui_log = bridge.ui_log.clone();
    ui_table.set(
        "show_tooltip",
        lua.create_function(move |_lua, text: String| {
            ui_log.lock().unwrap().push(UiOp {
                action: "tooltip".to_string(),
                text,
                duration: None,
            });
            Ok(())
        })?,
    )?;

    lua.globals().set("ui", ui_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (ScriptingEngine, GameBridge) {
        let engine = ScriptingEngine::new().unwrap();
        let bridge = GameBridge::default();
        register_all_apis(&engine, &bridge).unwrap();
        (engine, bridge)
    }

    #[test]
    fn test_world_flags() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"world.set_flag("quest_done", true)"#)
            .unwrap();

        let flags = bridge.flags.lock().unwrap();
        assert_eq!(flags.get("quest_done"), Some(&FlagValue::Bool(true)));
    }

    #[test]
    fn test_world_get_flag() {
        let (engine, bridge) = setup();
        bridge
            .flags
            .lock()
            .unwrap()
            .insert("gold".to_string(), FlagValue::Number(42.0));

        engine
            .run_string(r#"result = world.get_flag("gold")"#)
            .unwrap();
        assert_eq!(engine.get_global_number("result").unwrap(), Some(42.0));
    }

    #[test]
    fn test_world_get_flag_nil() {
        let (engine, _bridge) = setup();
        engine
            .run_string(r#"result = world.get_flag("nonexistent")"#)
            .unwrap();
        // Nil check
        let val: LuaValue = engine.lua().globals().get("result").unwrap();
        assert_eq!(val, LuaValue::Nil);
    }

    #[test]
    fn test_world_transition() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"world.transition_to("dungeon_f1", "entrance")"#)
            .unwrap();

        let log = bridge.world_log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].action, "transition_to");
        assert_eq!(log[0].args, vec!["dungeon_f1", "entrance"]);
    }

    #[test]
    fn test_world_get_current_location() {
        let (engine, bridge) = setup();
        *bridge.current_location.lock().unwrap() = "hub_town".to_string();

        engine
            .run_string(r#"result = world.get_current_location()"#)
            .unwrap();
        assert_eq!(
            engine.get_global_string("result").unwrap(),
            Some("hub_town".to_string())
        );
    }

    #[test]
    fn test_inventory_add_item() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"inventory.add_item("healing_potion", 3)"#)
            .unwrap();

        let log = bridge.inventory_log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(
            log[0],
            InventoryOp {
                action: "add".to_string(),
                item_id: "healing_potion".to_string(),
                count: 3,
            }
        );
    }

    #[test]
    fn test_inventory_remove_item() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"inventory.remove_item("herb", 1)"#)
            .unwrap();

        let log = bridge.inventory_log.lock().unwrap();
        assert_eq!(log[0].action, "remove");
    }

    #[test]
    fn test_inventory_has_item() {
        let (engine, bridge) = setup();
        bridge
            .flags
            .lock()
            .unwrap()
            .insert("has_item_key".to_string(), FlagValue::Bool(true));

        engine
            .run_string(r#"result = inventory.has_item("key")"#)
            .unwrap();
        assert_eq!(engine.get_global_bool("result").unwrap(), Some(true));
    }

    #[test]
    fn test_combat_start_encounter() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"combat.start_encounter("goblin_group_1")"#)
            .unwrap();

        let log = bridge.combat_log.lock().unwrap();
        assert_eq!(log[0].action, "start_encounter");
        assert_eq!(log[0].args, vec!["goblin_group_1"]);
    }

    #[test]
    fn test_combat_is_in_combat() {
        let (engine, bridge) = setup();
        *bridge.in_combat.lock().unwrap() = true;

        engine
            .run_string("result = combat.is_in_combat()")
            .unwrap();
        assert_eq!(engine.get_global_bool("result").unwrap(), Some(true));
    }

    #[test]
    fn test_combat_apply_damage() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"combat.apply_damage("enemy_1", 25)"#)
            .unwrap();

        let log = bridge.combat_log.lock().unwrap();
        assert_eq!(log[0].action, "apply_damage");
    }

    #[test]
    fn test_combat_apply_status() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"combat.apply_status("enemy_1", "poisoned", 3)"#)
            .unwrap();

        let log = bridge.combat_log.lock().unwrap();
        assert_eq!(log[0].action, "apply_status");
        assert_eq!(log[0].args, vec!["enemy_1", "poisoned", "3"]);
    }

    #[test]
    fn test_dialogue_start() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"dialogue.start("Herbalist_Greeting")"#)
            .unwrap();

        let log = bridge.dialogue_log.lock().unwrap();
        assert_eq!(log[0].node, "Herbalist_Greeting");
    }

    #[test]
    fn test_ui_notification() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"ui.show_notification("Something stirs...", 3.0)"#)
            .unwrap();

        let log = bridge.ui_log.lock().unwrap();
        assert_eq!(log[0].text, "Something stirs...");
        assert_eq!(log[0].duration, Some(3.0));
    }

    #[test]
    fn test_ui_tooltip() {
        let (engine, bridge) = setup();
        engine
            .run_string(r#"ui.show_tooltip("Healing Herb")"#)
            .unwrap();

        let log = bridge.ui_log.lock().unwrap();
        assert_eq!(log[0].action, "tooltip");
        assert_eq!(log[0].text, "Healing Herb");
    }

    #[test]
    fn test_full_script_scenario() {
        let (engine, bridge) = setup();
        engine
            .run_string(
                r#"
            function on_enter_zone(zone_name)
                if zone_name == "dark_clearing" and not world.get_flag("ambush_done") then
                    ui.show_notification("Something stirs in the undergrowth...", 3.0)
                    combat.start_encounter("forest_goblins_group_1")
                    world.set_flag("ambush_done", true)
                end
            end
        "#,
            )
            .unwrap();

        engine
            .call_function_with_args("on_enter_zone", &["dark_clearing"])
            .unwrap();

        let flags = bridge.flags.lock().unwrap();
        assert_eq!(flags.get("ambush_done"), Some(&FlagValue::Bool(true)));

        let combat_log = bridge.combat_log.lock().unwrap();
        assert_eq!(combat_log.len(), 1);

        let ui_log = bridge.ui_log.lock().unwrap();
        assert_eq!(ui_log.len(), 1);

        // Calling again should not trigger (flag is set)
        drop(flags);
        drop(combat_log);
        drop(ui_log);

        engine
            .call_function_with_args("on_enter_zone", &["dark_clearing"])
            .unwrap();

        let combat_log = bridge.combat_log.lock().unwrap();
        assert_eq!(combat_log.len(), 1); // still just 1
    }

    #[test]
    fn test_garden_event_script() {
        let (engine, bridge) = setup();
        engine
            .run_string(
                r#"
            function on_plant_mature(plot_index, generation)
                if generation >= 5 then
                    ui.show_notification("This plant has evolved remarkably!", 5.0)
                    inventory.add_item("recipe_greater_elixir", 1)
                end
            end
        "#,
            )
            .unwrap();

        // Simulate a mature generation-5 plant
        engine.lua().load(r#"on_plant_mature(0, 5)"#).exec().unwrap();

        let ui_log = bridge.ui_log.lock().unwrap();
        assert_eq!(ui_log.len(), 1);
        let inv_log = bridge.inventory_log.lock().unwrap();
        assert_eq!(inv_log.len(), 1);
        assert_eq!(inv_log[0].item_id, "recipe_greater_elixir");
    }
}
