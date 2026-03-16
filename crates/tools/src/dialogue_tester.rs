use apothecarys_dialogue::parser::YarnValue;
use apothecarys_dialogue::runner::{DialogueRunner, DialogueState};

/// A log entry for commands executed during dialogue.
#[derive(Debug, Clone)]
pub struct CommandLogEntry {
    pub command: String,
    pub args: Vec<String>,
}

/// State for the dialogue tester tool.
pub struct DialogueTesterState {
    runner: DialogueRunner,
    /// Log of executed commands.
    pub command_log: Vec<CommandLogEntry>,
    /// History of dialogue lines shown (for scrollback).
    pub line_history: Vec<DialogueLine>,
    /// Path to the loaded yarn file, if any.
    pub loaded_file: Option<String>,
}

impl std::fmt::Debug for DialogueTesterState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DialogueTesterState")
            .field("command_log", &self.command_log)
            .field("line_history", &self.line_history)
            .field("loaded_file", &self.loaded_file)
            .finish()
    }
}

/// A line of dialogue for display in the tester.
#[derive(Debug, Clone)]
pub struct DialogueLine {
    pub speaker: Option<String>,
    pub text: String,
}

impl DialogueTesterState {
    pub fn new() -> Self {
        Self {
            runner: DialogueRunner::new(),
            command_log: Vec::new(),
            line_history: Vec::new(),
            loaded_file: None,
        }
    }

    /// Load a yarn source string. Returns Ok with node titles or Err with parse error.
    pub fn load_yarn(&mut self, source: &str) -> Result<Vec<String>, String> {
        self.runner = DialogueRunner::new();
        self.command_log.clear();
        self.line_history.clear();

        self.runner
            .load_yarn(source)
            .map_err(|e| e.to_string())?;

        let titles: Vec<String> = self
            .runner
            .node_titles()
            .iter()
            .map(|s| s.to_string())
            .collect();
        Ok(titles)
    }

    /// Load a yarn file from disk.
    pub fn load_yarn_file(&mut self, path: &str) -> Result<Vec<String>, String> {
        let contents =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {e}"))?;
        self.loaded_file = Some(path.to_string());
        self.load_yarn(&contents)
    }

    /// Start a dialogue node by title.
    pub fn start_node(&mut self, title: &str) -> bool {
        self.command_log.clear();
        self.line_history.clear();
        self.runner.start_node(title)
    }

    /// Advance the dialogue. Returns the new state.
    pub fn advance(&mut self) -> DialogueState {
        let state = self.runner.advance();

        match &state {
            DialogueState::ShowingLine { speaker, text } => {
                self.line_history.push(DialogueLine {
                    speaker: speaker.clone(),
                    text: text.clone(),
                });
            }
            DialogueState::ExecutingCommand { command, args } => {
                self.command_log.push(CommandLogEntry {
                    command: command.clone(),
                    args: args.clone(),
                });
            }
            _ => {}
        }

        state
    }

    /// Select a choice by index.
    pub fn select_choice(&mut self, index: usize) -> bool {
        self.runner.select_choice(index)
    }

    /// Get the current state.
    pub fn state(&self) -> &DialogueState {
        self.runner.state()
    }

    /// Get a variable value.
    pub fn get_variable(&self, name: &str) -> Option<&YarnValue> {
        self.runner.get_variable(name)
    }

    /// Set a variable value (for testing purposes).
    pub fn set_variable(&mut self, name: &str, value: YarnValue) {
        self.runner.set_variable(name, value);
    }

    /// Get all variables and their values.
    pub fn variables(&self) -> Vec<(String, YarnValue)> {
        self.runner
            .variables()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Get the list of node titles loaded.
    pub fn node_titles(&self) -> Vec<String> {
        self.runner
            .node_titles()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Whether the dialogue is currently active.
    pub fn is_active(&self) -> bool {
        self.runner.is_active()
    }

    /// Format a YarnValue as a string for display.
    pub fn format_value(value: &YarnValue) -> String {
        match value {
            YarnValue::Bool(b) => b.to_string(),
            YarnValue::Number(n) => {
                if *n == (*n as i64) as f64 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            YarnValue::String(s) => format!("\"{s}\""),
        }
    }
}

impl Default for DialogueTesterState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_YARN: &str = r#"title: Greeting
---
NPC: Hello, traveler!
NPC: How can I help you?
-> Ask about potions.
    NPC: I have healing potions and mana potions.
    <<set $asked_potions to true>>
-> Leave.
    NPC: Goodbye!
===

title: Shop
---
NPC: Welcome to my shop!
<<give_item "health_potion" 1>>
===
"#;

    #[test]
    fn test_load_yarn() {
        let mut state = DialogueTesterState::new();
        let titles = state.load_yarn(TEST_YARN).unwrap();
        assert!(titles.contains(&"Greeting".to_string()));
        assert!(titles.contains(&"Shop".to_string()));
    }

    #[test]
    fn test_load_invalid_yarn() {
        let mut state = DialogueTesterState::new();
        let result = state.load_yarn("not valid yarn at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_start_and_advance() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        assert!(state.start_node("Greeting"));

        let s = state.advance();
        assert!(matches!(s, DialogueState::ShowingLine { ref text, .. } if text == "Hello, traveler!"));
        assert_eq!(state.line_history.len(), 1);
        assert_eq!(state.line_history[0].text, "Hello, traveler!");
    }

    #[test]
    fn test_advance_through_choices() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        state.start_node("Greeting");

        state.advance(); // line 1
        state.advance(); // line 2
        let s = state.advance(); // choices

        match s {
            DialogueState::WaitingForChoice { choices } => {
                assert_eq!(choices.len(), 2);
                assert_eq!(choices[0].text, "Ask about potions.");
            }
            other => panic!("Expected WaitingForChoice, got {other:?}"),
        }

        assert!(state.select_choice(0));
        let s = state.advance();
        assert!(matches!(s, DialogueState::ShowingLine { ref text, .. } if text.contains("healing potions")));
    }

    #[test]
    fn test_command_logging() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        state.start_node("Shop");

        state.advance(); // line
        let s = state.advance(); // command

        assert!(matches!(s, DialogueState::ExecutingCommand { .. }));
        assert_eq!(state.command_log.len(), 1);
        assert_eq!(state.command_log[0].command, "give_item");
        assert_eq!(state.command_log[0].args, vec!["health_potion", "1"]);
    }

    #[test]
    fn test_variable_tracking() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        state.start_node("Greeting");

        state.advance(); // line 1
        state.advance(); // line 2
        state.advance(); // choices
        state.select_choice(0); // "Ask about potions"
        state.advance(); // NPC response line

        // The set command is processed internally, advance past it
        state.advance();

        assert_eq!(
            state.get_variable("asked_potions"),
            Some(&YarnValue::Bool(true))
        );
    }

    #[test]
    fn test_set_variable() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();

        state.set_variable("test_var", YarnValue::Number(42.0));
        assert_eq!(
            state.get_variable("test_var"),
            Some(&YarnValue::Number(42.0))
        );
    }

    #[test]
    fn test_variables_list() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();

        state.set_variable("gold", YarnValue::Number(100.0));
        state.set_variable("name", YarnValue::String("Alchemist".to_string()));

        let vars = state.variables();
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_format_value() {
        assert_eq!(DialogueTesterState::format_value(&YarnValue::Bool(true)), "true");
        assert_eq!(DialogueTesterState::format_value(&YarnValue::Number(42.0)), "42");
        assert_eq!(DialogueTesterState::format_value(&YarnValue::Number(3.125)), "3.125");
        assert_eq!(
            DialogueTesterState::format_value(&YarnValue::String("hello".to_string())),
            "\"hello\""
        );
    }

    #[test]
    fn test_start_nonexistent_node() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        assert!(!state.start_node("NonexistentNode"));
    }

    #[test]
    fn test_line_history() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        state.start_node("Greeting");

        state.advance(); // line 1
        state.advance(); // line 2

        assert_eq!(state.line_history.len(), 2);
        assert_eq!(state.line_history[0].speaker.as_deref(), Some("NPC"));
        assert_eq!(state.line_history[0].text, "Hello, traveler!");
        assert_eq!(state.line_history[1].text, "How can I help you?");
    }

    #[test]
    fn test_node_titles() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        let titles = state.node_titles();
        assert!(titles.contains(&"Greeting".to_string()));
        assert!(titles.contains(&"Shop".to_string()));
    }

    #[test]
    fn test_is_active() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        assert!(!state.is_active());

        state.start_node("Greeting");
        assert!(state.is_active());
    }

    #[test]
    fn test_reload_clears_state() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        state.start_node("Shop");
        state.advance();
        state.advance();

        // Reload should clear everything
        state.load_yarn(TEST_YARN).unwrap();
        assert!(state.command_log.is_empty());
        assert!(state.line_history.is_empty());
    }

    #[test]
    fn test_select_invalid_choice() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();
        state.start_node("Greeting");
        state.advance(); // line 1
        state.advance(); // line 2
        state.advance(); // choices

        assert!(!state.select_choice(99));
    }

    #[test]
    fn test_debug_impl() {
        let state = DialogueTesterState::new();
        let debug_str = format!("{state:?}");
        assert!(debug_str.contains("DialogueTesterState"));
        assert!(debug_str.contains("command_log"));
    }

    #[test]
    fn test_default_trait() {
        let state = DialogueTesterState::default();
        assert!(!state.is_active());
        assert!(state.command_log.is_empty());
        assert!(state.line_history.is_empty());
        assert!(state.loaded_file.is_none());
    }

    #[test]
    fn test_load_file_nonexistent() {
        let mut state = DialogueTesterState::new();
        let result = state.load_yarn_file("/tmp/nonexistent_apothecarys_test.yarn");
        assert!(result.is_err());
    }

    #[test]
    fn test_format_value_integers() {
        assert_eq!(DialogueTesterState::format_value(&YarnValue::Number(0.0)), "0");
        assert_eq!(DialogueTesterState::format_value(&YarnValue::Number(1.0)), "1");
        assert_eq!(DialogueTesterState::format_value(&YarnValue::Number(-5.0)), "-5");
    }

    #[test]
    fn test_format_value_booleans() {
        assert_eq!(DialogueTesterState::format_value(&YarnValue::Bool(false)), "false");
    }

    #[test]
    fn test_multiple_nodes_navigation() {
        let mut state = DialogueTesterState::new();
        state.load_yarn(TEST_YARN).unwrap();

        // Start Greeting node
        state.start_node("Greeting");
        assert!(state.is_active());

        // Start a different node
        state.start_node("Shop");
        assert!(state.is_active());

        // Advance through shop
        let s = state.advance();
        assert!(matches!(s, DialogueState::ShowingLine { .. }));
    }

    #[test]
    fn test_line_history_speaker_none() {
        let yarn = r#"title: Test
---
This is a narration line without a speaker.
===
"#;
        let mut state = DialogueTesterState::new();
        state.load_yarn(yarn).unwrap();
        state.start_node("Test");
        state.advance();

        assert_eq!(state.line_history.len(), 1);
        assert!(state.line_history[0].speaker.is_none());
        assert_eq!(
            state.line_history[0].text,
            "This is a narration line without a speaker."
        );
    }
}
