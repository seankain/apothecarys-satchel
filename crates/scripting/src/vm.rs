use mlua::prelude::*;
use std::path::Path;

/// Errors that can occur in the scripting engine.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Script not found: {path}")]
    NotFound { path: String },
}

/// A sandboxed Lua scripting engine for game logic.
pub struct ScriptingEngine {
    lua: Lua,
}

impl ScriptingEngine {
    /// Create a new sandboxed scripting engine.
    pub fn new() -> Result<Self, ScriptError> {
        let lua = Lua::new();

        // Sandbox: remove dangerous modules
        {
            let globals = lua.globals();
            globals.set("os", mlua::Value::Nil)?;
            globals.set("io", mlua::Value::Nil)?;
            globals.set("debug", mlua::Value::Nil)?;
            globals.set("loadfile", mlua::Value::Nil)?;
            globals.set("dofile", mlua::Value::Nil)?;
        }

        Ok(Self { lua })
    }

    /// Get a reference to the underlying Lua state.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Run a Lua script from a string.
    pub fn run_string(&self, source: &str) -> Result<(), ScriptError> {
        self.lua.load(source).exec()?;
        Ok(())
    }

    /// Run a Lua script from a file path.
    pub fn run_file(&self, path: impl AsRef<Path>) -> Result<(), ScriptError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ScriptError::NotFound {
                path: path.display().to_string(),
            });
        }
        let source = std::fs::read_to_string(path)?;
        self.run_string(&source)
    }

    /// Call a global Lua function by name with no arguments.
    pub fn call_function(&self, name: &str) -> Result<(), ScriptError> {
        let func: LuaFunction = self.lua.globals().get(name)?;
        func.call::<()>(())?;
        Ok(())
    }

    /// Call a global Lua function with string arguments.
    pub fn call_function_with_args(
        &self,
        name: &str,
        args: &[&str],
    ) -> Result<(), ScriptError> {
        let func: LuaFunction = self.lua.globals().get(name)?;
        let lua_args: Vec<LuaValue> = args
            .iter()
            .map(|s| LuaValue::String(self.lua.create_string(s).unwrap()))
            .collect();
        let multi = LuaMultiValue::from_vec(lua_args);
        func.call::<()>(multi)?;
        Ok(())
    }

    /// Set a global string variable.
    pub fn set_global_string(&self, name: &str, value: &str) -> Result<(), ScriptError> {
        self.lua.globals().set(name, value)?;
        Ok(())
    }

    /// Set a global number variable.
    pub fn set_global_number(&self, name: &str, value: f64) -> Result<(), ScriptError> {
        self.lua.globals().set(name, value)?;
        Ok(())
    }

    /// Set a global boolean variable.
    pub fn set_global_bool(&self, name: &str, value: bool) -> Result<(), ScriptError> {
        self.lua.globals().set(name, value)?;
        Ok(())
    }

    /// Get a global string variable.
    pub fn get_global_string(&self, name: &str) -> Result<Option<String>, ScriptError> {
        let val: LuaValue = self.lua.globals().get(name)?;
        match val {
            LuaValue::String(s) => Ok(Some(s.to_str()?.to_string())),
            LuaValue::Nil => Ok(None),
            _ => Ok(None),
        }
    }

    /// Get a global number variable.
    pub fn get_global_number(&self, name: &str) -> Result<Option<f64>, ScriptError> {
        let val: LuaValue = self.lua.globals().get(name)?;
        match val {
            LuaValue::Number(n) => Ok(Some(n)),
            LuaValue::Integer(n) => Ok(Some(n as f64)),
            LuaValue::Nil => Ok(None),
            _ => Ok(None),
        }
    }

    /// Get a global boolean variable.
    pub fn get_global_bool(&self, name: &str) -> Result<Option<bool>, ScriptError> {
        let val: LuaValue = self.lua.globals().get(name)?;
        match val {
            LuaValue::Boolean(b) => Ok(Some(b)),
            LuaValue::Nil => Ok(None),
            _ => Ok(None),
        }
    }

    /// Check if a global function exists.
    pub fn has_function(&self, name: &str) -> bool {
        self.lua
            .globals()
            .get::<LuaValue>(name)
            .map(|v| matches!(v, LuaValue::Function(_)))
            .unwrap_or(false)
    }
}

impl Default for ScriptingEngine {
    fn default() -> Self {
        Self::new().expect("Failed to create Lua VM")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_engine() {
        let engine = ScriptingEngine::new().unwrap();
        assert!(engine.lua().globals().get::<LuaValue>("os").unwrap() == LuaValue::Nil);
        assert!(engine.lua().globals().get::<LuaValue>("io").unwrap() == LuaValue::Nil);
        assert!(engine.lua().globals().get::<LuaValue>("debug").unwrap() == LuaValue::Nil);
    }

    #[test]
    fn test_run_string() {
        let engine = ScriptingEngine::new().unwrap();
        engine.run_string("x = 42").unwrap();
        let x = engine.get_global_number("x").unwrap();
        assert_eq!(x, Some(42.0));
    }

    #[test]
    fn test_sandboxing_blocks_os() {
        let engine = ScriptingEngine::new().unwrap();
        let result = engine.run_string("os.execute('echo hello')");
        assert!(result.is_err());
    }

    #[test]
    fn test_sandboxing_blocks_io() {
        let engine = ScriptingEngine::new().unwrap();
        let result = engine.run_string("io.open('/etc/passwd')");
        assert!(result.is_err());
    }

    #[test]
    fn test_call_function() {
        let engine = ScriptingEngine::new().unwrap();
        engine
            .run_string(
                r#"
            result = false
            function my_func()
                result = true
            end
        "#,
            )
            .unwrap();

        engine.call_function("my_func").unwrap();
        let result = engine.get_global_bool("result").unwrap();
        assert_eq!(result, Some(true));
    }

    #[test]
    fn test_call_function_with_args() {
        let engine = ScriptingEngine::new().unwrap();
        engine
            .run_string(
                r#"
            captured_zone = ""
            function on_enter_zone(zone_name)
                captured_zone = zone_name
            end
        "#,
            )
            .unwrap();

        engine
            .call_function_with_args("on_enter_zone", &["dark_clearing"])
            .unwrap();
        let zone = engine.get_global_string("captured_zone").unwrap();
        assert_eq!(zone, Some("dark_clearing".to_string()));
    }

    #[test]
    fn test_global_variables() {
        let engine = ScriptingEngine::new().unwrap();

        engine.set_global_string("name", "Alchemist").unwrap();
        engine.set_global_number("gold", 100.0).unwrap();
        engine.set_global_bool("has_key", true).unwrap();

        assert_eq!(
            engine.get_global_string("name").unwrap(),
            Some("Alchemist".to_string())
        );
        assert_eq!(engine.get_global_number("gold").unwrap(), Some(100.0));
        assert_eq!(engine.get_global_bool("has_key").unwrap(), Some(true));
    }

    #[test]
    fn test_nonexistent_globals() {
        let engine = ScriptingEngine::new().unwrap();
        assert_eq!(engine.get_global_string("missing").unwrap(), None);
        assert_eq!(engine.get_global_number("missing").unwrap(), None);
        assert_eq!(engine.get_global_bool("missing").unwrap(), None);
    }

    #[test]
    fn test_has_function() {
        let engine = ScriptingEngine::new().unwrap();
        engine.run_string("function foo() end").unwrap();
        assert!(engine.has_function("foo"));
        assert!(!engine.has_function("bar"));
    }

    #[test]
    fn test_run_nonexistent_file() {
        let engine = ScriptingEngine::new().unwrap();
        let result = engine.run_file("/nonexistent/path.lua");
        assert!(matches!(result, Err(ScriptError::NotFound { .. })));
    }

    #[test]
    fn test_lua_table_operations() {
        let engine = ScriptingEngine::new().unwrap();
        engine
            .run_string(
                r#"
            data = {
                name = "test",
                value = 42
            }
            result_name = data.name
            result_value = data.value
        "#,
            )
            .unwrap();

        assert_eq!(
            engine.get_global_string("result_name").unwrap(),
            Some("test".to_string())
        );
        assert_eq!(engine.get_global_number("result_value").unwrap(), Some(42.0));
    }

    #[test]
    fn test_lua_math_operations() {
        let engine = ScriptingEngine::new().unwrap();
        // math module should still be available
        engine
            .run_string("result = math.floor(3.7)")
            .unwrap();
        assert_eq!(engine.get_global_number("result").unwrap(), Some(3.0));
    }

    #[test]
    fn test_lua_string_operations() {
        let engine = ScriptingEngine::new().unwrap();
        engine
            .run_string(r#"result = string.upper("hello")"#)
            .unwrap();
        assert_eq!(
            engine.get_global_string("result").unwrap(),
            Some("HELLO".to_string())
        );
    }
}
