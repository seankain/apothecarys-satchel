use std::collections::HashMap;

use crate::parser::{
    ComparisonOp, ParseError, YarnExpression, YarnNode, YarnStatement, YarnValue,
    parse_yarn_file,
};

/// The current state of the dialogue runner.
#[derive(Debug, Clone, PartialEq)]
pub enum DialogueState {
    Idle,
    ShowingLine {
        speaker: Option<String>,
        text: String,
    },
    WaitingForChoice {
        choices: Vec<AvailableChoice>,
    },
    ExecutingCommand {
        command: String,
        args: Vec<String>,
    },
    Finished,
}

/// A choice presented to the player.
#[derive(Debug, Clone, PartialEq)]
pub struct AvailableChoice {
    pub index: usize,
    pub text: String,
    pub enabled: bool,
}

/// Runs parsed Yarn dialogue trees, managing state and variable storage.
pub struct DialogueRunner {
    nodes: HashMap<String, YarnNode>,
    variables: HashMap<String, YarnValue>,
    execution_stack: Vec<ExecutionFrame>,
    state: DialogueState,
}

/// A frame of execution in the dialogue runner stack.
#[derive(Debug, Clone)]
struct ExecutionFrame {
    statements: Vec<YarnStatement>,
    position: usize,
}

impl DialogueRunner {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            variables: HashMap::new(),
            execution_stack: Vec::new(),
            state: DialogueState::Idle,
        }
    }

    /// Load yarn nodes from a source string.
    pub fn load_yarn(&mut self, source: &str) -> Result<(), ParseError> {
        let parsed_nodes = parse_yarn_file(source)?;
        for node in parsed_nodes {
            self.nodes.insert(node.title.clone(), node);
        }
        Ok(())
    }

    /// Start executing a dialogue node by title.
    pub fn start_node(&mut self, title: &str) -> bool {
        if let Some(node) = self.nodes.get(title).cloned() {
            self.execution_stack.clear();
            self.execution_stack.push(ExecutionFrame {
                statements: node.body,
                position: 0,
            });
            self.state = DialogueState::Idle;
            true
        } else {
            false
        }
    }

    /// Advance the dialogue to the next state. Returns the new state.
    pub fn advance(&mut self) -> DialogueState {
        loop {
            let frame = match self.execution_stack.last_mut() {
                Some(f) => f,
                None => {
                    self.state = DialogueState::Finished;
                    return self.state.clone();
                }
            };

            if frame.position >= frame.statements.len() {
                self.execution_stack.pop();
                continue;
            }

            let stmt = frame.statements[frame.position].clone();
            frame.position += 1;

            match stmt {
                YarnStatement::Line { speaker, text } => {
                    let text = self.resolve_interpolation(&text);
                    self.state = DialogueState::ShowingLine { speaker, text };
                    return self.state.clone();
                }

                YarnStatement::Choice { .. } => {
                    // Collect consecutive choices
                    let mut choices_stmts = vec![stmt];
                    let frame = self.execution_stack.last_mut().unwrap();
                    while frame.position < frame.statements.len() {
                        if matches!(
                            frame.statements[frame.position],
                            YarnStatement::Choice { .. }
                        ) {
                            choices_stmts.push(frame.statements[frame.position].clone());
                            frame.position += 1;
                        } else {
                            break;
                        }
                    }

                    let choices: Vec<AvailableChoice> = choices_stmts
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            if let YarnStatement::Choice {
                                text, condition, ..
                            } = s
                            {
                                let enabled = condition
                                    .as_ref()
                                    .map(|c| self.evaluate_bool(c))
                                    .unwrap_or(true);
                                let text = self.resolve_interpolation(text);
                                AvailableChoice {
                                    index: i,
                                    text,
                                    enabled,
                                }
                            } else {
                                unreachable!()
                            }
                        })
                        .collect();

                    // Store choice stmts for selection
                    let frame = self.execution_stack.last_mut().unwrap();
                    // Temporarily store the choice statements
                    // We'll handle selection in select_choice
                    frame.position -= choices_stmts.len(); // rewind
                    frame.position += choices_stmts.len(); // but keep position past choices

                    // Store choices data for select_choice to use
                    self.execution_stack.push(ExecutionFrame {
                        statements: choices_stmts,
                        position: 0,
                    });

                    self.state = DialogueState::WaitingForChoice { choices };
                    return self.state.clone();
                }

                YarnStatement::Command { name, args } => {
                    self.state = DialogueState::ExecutingCommand { command: name, args };
                    return self.state.clone();
                }

                YarnStatement::Jump(target) => {
                    // Clear stack and start new node
                    if !self.start_node(&target) {
                        self.state = DialogueState::Finished;
                        return self.state.clone();
                    }
                    continue;
                }

                YarnStatement::SetVariable { name, value } => {
                    let resolved = self.evaluate_value(&value);
                    self.variables.insert(name, resolved);
                    continue;
                }

                YarnStatement::Conditional {
                    condition,
                    then_body,
                    else_body,
                } => {
                    let result = self.evaluate_bool(&condition);
                    let body = if result {
                        then_body
                    } else {
                        else_body.unwrap_or_default()
                    };
                    if !body.is_empty() {
                        self.execution_stack.push(ExecutionFrame {
                            statements: body,
                            position: 0,
                        });
                    }
                    continue;
                }
            }
        }
    }

    /// Select a choice by index when in WaitingForChoice state.
    pub fn select_choice(&mut self, index: usize) -> bool {
        if !matches!(self.state, DialogueState::WaitingForChoice { .. }) {
            return false;
        }

        // Pop the choices frame
        let choices_frame = match self.execution_stack.pop() {
            Some(f) => f,
            None => return false,
        };

        if index >= choices_frame.statements.len() {
            // Push back if invalid
            self.execution_stack.push(choices_frame);
            return false;
        }

        // Get the selected choice's body
        if let YarnStatement::Choice { body, .. } = &choices_frame.statements[index] {
            if !body.is_empty() {
                self.execution_stack.push(ExecutionFrame {
                    statements: body.clone(),
                    position: 0,
                });
            }
        }

        self.state = DialogueState::Idle;
        true
    }

    /// Get a variable value.
    pub fn get_variable(&self, name: &str) -> Option<&YarnValue> {
        self.variables.get(name)
    }

    /// Set a variable value.
    pub fn set_variable(&mut self, name: &str, value: YarnValue) {
        self.variables.insert(name.to_string(), value);
    }

    /// Get the current dialogue state.
    pub fn state(&self) -> &DialogueState {
        &self.state
    }

    /// Get the titles of all loaded nodes.
    pub fn node_titles(&self) -> Vec<&str> {
        self.nodes.keys().map(|s| s.as_str()).collect()
    }

    /// Get all current variable names and values.
    pub fn variables(&self) -> &HashMap<String, YarnValue> {
        &self.variables
    }

    /// Whether dialogue is currently active (not idle and not finished).
    pub fn is_active(&self) -> bool {
        !matches!(self.state, DialogueState::Idle | DialogueState::Finished)
            || !self.execution_stack.is_empty()
    }

    fn resolve_interpolation(&self, text: &str) -> String {
        let mut result = text.to_string();
        // Replace {$var_name} with variable values
        while let Some(start) = result.find("{$") {
            if let Some(end) = result[start..].find('}') {
                let var_name = &result[start + 2..start + end];
                let value_str = match self.variables.get(var_name) {
                    Some(YarnValue::String(s)) => s.clone(),
                    Some(YarnValue::Number(n)) => {
                        if *n == (*n as i64) as f64 {
                            format!("{}", *n as i64)
                        } else {
                            format!("{n}")
                        }
                    }
                    Some(YarnValue::Bool(b)) => b.to_string(),
                    None => format!("{{${var_name}}}"),
                };
                result = format!("{}{}{}", &result[..start], value_str, &result[start + end + 1..]);
            } else {
                break;
            }
        }
        result
    }

    fn evaluate_bool(&self, expr: &YarnExpression) -> bool {
        match self.evaluate_value(expr) {
            YarnValue::Bool(b) => b,
            YarnValue::Number(n) => n != 0.0,
            YarnValue::String(s) => !s.is_empty(),
        }
    }

    fn evaluate_value(&self, expr: &YarnExpression) -> YarnValue {
        match expr {
            YarnExpression::Literal(v) => v.clone(),
            YarnExpression::Variable(name) => self
                .variables
                .get(name)
                .cloned()
                .unwrap_or(YarnValue::Bool(false)),
            YarnExpression::Not(inner) => {
                YarnValue::Bool(!self.evaluate_bool(inner))
            }
            YarnExpression::And(left, right) => {
                YarnValue::Bool(self.evaluate_bool(left) && self.evaluate_bool(right))
            }
            YarnExpression::Or(left, right) => {
                YarnValue::Bool(self.evaluate_bool(left) || self.evaluate_bool(right))
            }
            YarnExpression::Comparison(left, op, right) => {
                let lv = self.evaluate_value(left);
                let rv = self.evaluate_value(right);
                YarnValue::Bool(compare_values(&lv, op, &rv))
            }
        }
    }
}

impl Default for DialogueRunner {
    fn default() -> Self {
        Self::new()
    }
}

fn compare_values(left: &YarnValue, op: &ComparisonOp, right: &YarnValue) -> bool {
    match (left, right) {
        (YarnValue::Number(a), YarnValue::Number(b)) => match op {
            ComparisonOp::Equal => (a - b).abs() < f64::EPSILON,
            ComparisonOp::NotEqual => (a - b).abs() >= f64::EPSILON,
            ComparisonOp::LessThan => a < b,
            ComparisonOp::LessEqual => a <= b,
            ComparisonOp::GreaterThan => a > b,
            ComparisonOp::GreaterEqual => a >= b,
        },
        (YarnValue::String(a), YarnValue::String(b)) => match op {
            ComparisonOp::Equal => a == b,
            ComparisonOp::NotEqual => a != b,
            _ => false,
        },
        (YarnValue::Bool(a), YarnValue::Bool(b)) => match op {
            ComparisonOp::Equal => a == b,
            ComparisonOp::NotEqual => a != b,
            _ => false,
        },
        _ => false,
    }
}

type CommandHandler = Box<dyn Fn(&[String]) -> CommandResult>;

/// Registry for custom dialogue commands.
pub struct CommandRegistry {
    handlers: HashMap<String, CommandHandler>,
}

/// Result of executing a dialogue command.
#[derive(Debug)]
pub enum CommandResult {
    Ok,
    Error(String),
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a command handler.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        handler: impl Fn(&[String]) -> CommandResult + 'static,
    ) {
        self.handlers.insert(name.into(), Box::new(handler));
    }

    /// Execute a command by name.
    pub fn execute(&self, name: &str, args: &[String]) -> CommandResult {
        match self.handlers.get(name) {
            Some(handler) => handler(args),
            None => CommandResult::Error(format!("Unknown command: {name}")),
        }
    }

    /// Check if a command is registered.
    pub fn has_command(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_YARN: &str = r#"title: Start
---
NPC: Hello there, traveler!
NPC: What would you like to do?
-> Ask about the town.
    NPC: This is Willowmere, a peaceful place.
-> Ask about quests.
    NPC: There's a cave to the north that needs clearing.
    <<set $knows_quest to true>>
-> Leave.
    NPC: Safe travels!
===
"#;

    #[test]
    fn test_runner_basic_flow() {
        let mut runner = DialogueRunner::new();
        runner.load_yarn(TEST_YARN).unwrap();
        runner.start_node("Start");

        // First line
        let state = runner.advance();
        assert!(matches!(
            state,
            DialogueState::ShowingLine {
                ref speaker,
                ref text
            } if speaker.as_deref() == Some("NPC") && text == "Hello there, traveler!"
        ));

        // Second line
        let state = runner.advance();
        assert!(matches!(state, DialogueState::ShowingLine { .. }));

        // Choices
        let state = runner.advance();
        match state {
            DialogueState::WaitingForChoice { ref choices } => {
                assert_eq!(choices.len(), 3);
                assert!(choices[0].enabled);
                assert_eq!(choices[0].text, "Ask about the town.");
            }
            other => panic!("Expected WaitingForChoice, got {other:?}"),
        }
    }

    #[test]
    fn test_runner_choice_selection() {
        let mut runner = DialogueRunner::new();
        runner.load_yarn(TEST_YARN).unwrap();
        runner.start_node("Start");

        runner.advance(); // line 1
        runner.advance(); // line 2
        runner.advance(); // choices

        // Select first choice
        assert!(runner.select_choice(0));

        // Should get the choice body line
        let state = runner.advance();
        assert!(matches!(
            state,
            DialogueState::ShowingLine { ref text, .. } if text.contains("Willowmere")
        ));

        // Should finish
        let state = runner.advance();
        assert_eq!(state, DialogueState::Finished);
    }

    #[test]
    fn test_runner_set_variable() {
        let mut runner = DialogueRunner::new();
        runner.load_yarn(TEST_YARN).unwrap();
        runner.start_node("Start");

        runner.advance(); // line 1
        runner.advance(); // line 2
        runner.advance(); // choices

        // Select "Ask about quests" (index 1)
        runner.select_choice(1);

        // Get dialogue line
        runner.advance();

        // Should hit set command (processed internally) then finish
        // The set is processed internally, advance should skip past it
        let _state = runner.advance();

        assert_eq!(
            runner.get_variable("knows_quest"),
            Some(&YarnValue::Bool(true))
        );
    }

    #[test]
    fn test_runner_variable_interpolation() {
        let yarn = r#"title: Test
---
<<set $player_name to "Alchemist">>
NPC: Hello {$player_name}!
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        runner.start_node("Test");

        // set is processed internally, advance goes straight to the line
        let state = runner.advance();
        match state {
            DialogueState::ShowingLine { text, .. } => {
                assert_eq!(text, "Hello Alchemist!");
            }
            other => panic!("Expected ShowingLine, got {other:?}"),
        }
    }

    #[test]
    fn test_runner_conditional() {
        let yarn = r#"title: Test
---
<<set $has_key to true>>
<<if $has_key>>
NPC: You have the key!
<<else>>
NPC: You need a key.
<<endif>>
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        runner.start_node("Test");

        let state = runner.advance(); // processes set, then conditional, then shows line
        assert!(matches!(
            state,
            DialogueState::ShowingLine { ref text, .. } if text == "You have the key!"
        ));
    }

    #[test]
    fn test_runner_conditional_false_branch() {
        let yarn = r#"title: Test
---
<<if $has_key>>
NPC: You have the key!
<<else>>
NPC: You need a key.
<<endif>>
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        // Don't set $has_key — defaults to false
        runner.start_node("Test");

        let state = runner.advance();
        assert!(matches!(
            state,
            DialogueState::ShowingLine { ref text, .. } if text == "You need a key."
        ));
    }

    #[test]
    fn test_runner_jump() {
        let yarn = r#"title: Start
---
NPC: Before jump.
<<jump Other>>
===

title: Other
---
NPC: After jump.
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        runner.start_node("Start");

        let state = runner.advance();
        assert!(matches!(
            state,
            DialogueState::ShowingLine { ref text, .. } if text == "Before jump."
        ));

        // Jump is processed internally, next advance shows Other node's line
        let state = runner.advance();
        assert!(matches!(
            state,
            DialogueState::ShowingLine { ref text, .. } if text == "After jump."
        ));
    }

    #[test]
    fn test_runner_command_execution() {
        let yarn = r#"title: Test
---
<<give_item "healing_potion" 3>>
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        runner.start_node("Test");

        let state = runner.advance();
        match state {
            DialogueState::ExecutingCommand { command, args } => {
                assert_eq!(command, "give_item");
                assert_eq!(args, vec!["healing_potion", "3"]);
            }
            other => panic!("Expected ExecutingCommand, got {other:?}"),
        }
    }

    #[test]
    fn test_runner_choice_with_condition() {
        let yarn = r#"title: Test
---
NPC: What do you want?
-> Always available.
    NPC: Good choice.
-> [if $has_item] Conditional choice.
    NPC: You have the item!
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        runner.start_node("Test");

        runner.advance(); // line

        let state = runner.advance();
        match state {
            DialogueState::WaitingForChoice { choices } => {
                assert_eq!(choices.len(), 2);
                assert!(choices[0].enabled);
                assert!(!choices[1].enabled); // $has_item is not set
            }
            other => panic!("Expected WaitingForChoice, got {other:?}"),
        }
    }

    #[test]
    fn test_runner_choice_with_condition_enabled() {
        let yarn = r#"title: Test
---
NPC: What do you want?
-> Always available.
    NPC: Good choice.
-> [if $has_item] Conditional choice.
    NPC: You have the item!
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        runner.set_variable("has_item", YarnValue::Bool(true));
        runner.start_node("Test");

        runner.advance(); // line

        let state = runner.advance();
        match state {
            DialogueState::WaitingForChoice { choices } => {
                assert!(choices[1].enabled); // now enabled
            }
            other => panic!("Expected WaitingForChoice, got {other:?}"),
        }
    }

    #[test]
    fn test_command_registry() {
        let mut registry = CommandRegistry::new();
        registry.register("test_cmd", |args| {
            if args.len() >= 2 {
                CommandResult::Ok
            } else {
                CommandResult::Error("Need 2 args".to_string())
            }
        });

        assert!(registry.has_command("test_cmd"));
        assert!(!registry.has_command("unknown"));

        let result = registry.execute(
            "test_cmd",
            &["a".to_string(), "b".to_string()],
        );
        assert!(matches!(result, CommandResult::Ok));
    }

    #[test]
    fn test_runner_start_nonexistent_node() {
        let mut runner = DialogueRunner::new();
        assert!(!runner.start_node("nonexistent"));
    }

    #[test]
    fn test_runner_number_interpolation() {
        let yarn = r#"title: Test
---
<<set $gold to 42>>
NPC: You have {$gold} gold.
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        runner.start_node("Test");

        let state = runner.advance();
        match state {
            DialogueState::ShowingLine { text, .. } => {
                assert_eq!(text, "You have 42 gold.");
            }
            other => panic!("Expected ShowingLine, got {other:?}"),
        }
    }

    #[test]
    fn test_runner_comparison_condition() {
        let yarn = r#"title: Test
---
<<set $gold to 150>>
<<if $gold >= 100>>
NPC: You're rich!
<<else>>
NPC: You're poor.
<<endif>>
===
"#;
        let mut runner = DialogueRunner::new();
        runner.load_yarn(yarn).unwrap();
        runner.start_node("Test");

        let state = runner.advance();
        assert!(matches!(
            state,
            DialogueState::ShowingLine { ref text, .. } if text == "You're rich!"
        ));
    }
}
