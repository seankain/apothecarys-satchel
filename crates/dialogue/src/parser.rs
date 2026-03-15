use serde::{Deserialize, Serialize};

/// A parsed Yarn dialogue node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct YarnNode {
    pub title: String,
    pub tags: Vec<String>,
    pub body: Vec<YarnStatement>,
}

/// A single statement within a Yarn node body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum YarnStatement {
    Line {
        speaker: Option<String>,
        text: String,
    },
    Choice {
        text: String,
        condition: Option<YarnExpression>,
        body: Vec<YarnStatement>,
    },
    Command {
        name: String,
        args: Vec<String>,
    },
    Jump(String),
    SetVariable {
        name: String,
        value: YarnExpression,
    },
    Conditional {
        condition: YarnExpression,
        then_body: Vec<YarnStatement>,
        else_body: Option<Vec<YarnStatement>>,
    },
}

/// A value literal in Yarn scripts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum YarnValue {
    Bool(bool),
    Number(f64),
    String(String),
}

/// An expression in Yarn scripts used for conditions and assignments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum YarnExpression {
    Variable(String),
    Literal(YarnValue),
    Comparison(Box<YarnExpression>, ComparisonOp, Box<YarnExpression>),
    Not(Box<YarnExpression>),
    And(Box<YarnExpression>, Box<YarnExpression>),
    Or(Box<YarnExpression>, Box<YarnExpression>),
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComparisonOp {
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
}

/// Errors that can occur during parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Line {line}: {message}")]
    SyntaxError { line: usize, message: String },
    #[error("Unexpected end of file at line {line}")]
    UnexpectedEof { line: usize },
    #[error("No nodes found in file")]
    NoNodes,
}

/// Parse a Yarn file source string into a list of nodes.
pub fn parse_yarn_file(source: &str) -> Result<Vec<YarnNode>, ParseError> {
    let mut nodes = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut pos = 0;

    while pos < lines.len() {
        // Skip blank lines outside nodes
        if lines[pos].trim().is_empty() {
            pos += 1;
            continue;
        }

        // Look for node start (title: line)
        if lines[pos].trim().starts_with("title:") {
            let (node, next_pos) = parse_node(&lines, pos)?;
            nodes.push(node);
            pos = next_pos;
        } else {
            pos += 1;
        }
    }

    if nodes.is_empty() {
        return Err(ParseError::NoNodes);
    }

    Ok(nodes)
}

fn parse_node(lines: &[&str], start: usize) -> Result<(YarnNode, usize), ParseError> {
    let mut pos = start;

    // Parse title
    let title_line = lines[pos].trim();
    let title = title_line
        .strip_prefix("title:")
        .ok_or(ParseError::SyntaxError {
            line: pos + 1,
            message: "Expected 'title:' header".to_string(),
        })?
        .trim()
        .to_string();
    pos += 1;

    // Parse optional tags
    let mut tags = Vec::new();
    while pos < lines.len() {
        let line = lines[pos].trim();
        if line == "---" {
            break;
        }
        if line.starts_with("tags:") {
            let tag_str = line.strip_prefix("tags:").unwrap().trim();
            if !tag_str.is_empty() {
                tags = tag_str.split_whitespace().map(|s| s.to_string()).collect();
            }
        }
        // Skip other header fields
        pos += 1;
    }

    // Expect ---
    if pos >= lines.len() || lines[pos].trim() != "---" {
        return Err(ParseError::UnexpectedEof { line: pos + 1 });
    }
    pos += 1;

    // Parse body until ===
    let (body, next_pos) = parse_body(lines, pos, 0)?;
    pos = next_pos;

    // Expect ===
    if pos >= lines.len() || lines[pos].trim() != "===" {
        return Err(ParseError::UnexpectedEof { line: pos + 1 });
    }
    pos += 1;

    Ok((YarnNode { title, tags, body }, pos))
}

fn parse_body(
    lines: &[&str],
    start: usize,
    indent_level: usize,
) -> Result<(Vec<YarnStatement>, usize), ParseError> {
    let mut stmts = Vec::new();
    let mut pos = start;

    while pos < lines.len() {
        let line = lines[pos];
        let trimmed = line.trim();

        // End of node
        if trimmed == "===" {
            break;
        }

        // Skip empty lines
        if trimmed.is_empty() {
            pos += 1;
            continue;
        }

        // Calculate indentation (for choice bodies)
        let current_indent = count_indent(line);
        if indent_level > 0 && current_indent < indent_level {
            break;
        }

        // Choice line
        if trimmed.starts_with("->") {
            let (choice, next_pos) = parse_choice(lines, pos)?;
            stmts.push(choice);
            pos = next_pos;
            continue;
        }

        // Command line <<...>>
        if trimmed.starts_with("<<") {
            let stmt = parse_command_line(trimmed, pos + 1)?;
            match stmt {
                // Handle <<if>> conditionals that span multiple lines
                CommandParsed::If(condition) => {
                    let (cond_stmt, next_pos) =
                        parse_conditional(lines, pos, condition, indent_level)?;
                    stmts.push(cond_stmt);
                    pos = next_pos;
                    continue;
                }
                CommandParsed::EndIf | CommandParsed::Else => {
                    // These are handled by parse_conditional
                    break;
                }
                CommandParsed::Statement(stmt) => {
                    stmts.push(stmt);
                }
            }
            pos += 1;
            continue;
        }

        // Regular dialogue line
        stmts.push(parse_dialogue_line(trimmed));
        pos += 1;
    }

    Ok((stmts, pos))
}

fn count_indent(line: &str) -> usize {
    let spaces: usize = line.len() - line.trim_start().len();
    spaces
}

fn parse_choice(
    lines: &[&str],
    start: usize,
) -> Result<(YarnStatement, usize), ParseError> {
    let line = lines[start].trim();
    let after_arrow = line.strip_prefix("->").unwrap().trim();

    // Check for condition: [if $var] text
    let (text, condition) = if after_arrow.starts_with('[') {
        if let Some(end_bracket) = after_arrow.find(']') {
            let cond_str = &after_arrow[1..end_bracket];
            let text = after_arrow[end_bracket + 1..].trim().to_string();
            let condition = parse_condition_string(cond_str.trim())?;
            (text, Some(condition))
        } else {
            (after_arrow.to_string(), None)
        }
    } else {
        (after_arrow.to_string(), None)
    };

    let text = interpolate_variables(&text);
    let choice_indent = count_indent(lines[start]);
    let body_indent = choice_indent + 4; // expect 4-space indent for body

    // Parse indented body
    let mut body = Vec::new();
    let mut pos = start + 1;

    while pos < lines.len() {
        let current_line = lines[pos];
        let trimmed = current_line.trim();

        if trimmed.is_empty() {
            pos += 1;
            continue;
        }

        // Stop if we're back at choice level or less
        let current_indent = count_indent(current_line);
        if current_indent < body_indent && !trimmed.is_empty() {
            break;
        }

        // Parse as body content
        if trimmed.starts_with("<<") {
            let cmd = parse_command_line(trimmed, pos + 1)?;
            match cmd {
                CommandParsed::If(condition) => {
                    let (cond_stmt, next_pos) =
                        parse_conditional(lines, pos, condition, body_indent)?;
                    body.push(cond_stmt);
                    pos = next_pos;
                    continue;
                }
                CommandParsed::EndIf | CommandParsed::Else => break,
                CommandParsed::Statement(stmt) => body.push(stmt),
            }
        } else if trimmed.starts_with("->") {
            let (sub_choice, next_pos) = parse_choice(lines, pos)?;
            body.push(sub_choice);
            pos = next_pos;
            continue;
        } else {
            body.push(parse_dialogue_line(trimmed));
        }
        pos += 1;
    }

    Ok((
        YarnStatement::Choice {
            text,
            condition,
            body,
        },
        pos,
    ))
}

enum CommandParsed {
    Statement(YarnStatement),
    If(YarnExpression),
    Else,
    EndIf,
}

fn parse_command_line(trimmed: &str, line_num: usize) -> Result<CommandParsed, ParseError> {
    // Extract content between << and >>
    let content = trimmed
        .strip_prefix("<<")
        .and_then(|s| s.strip_suffix(">>"))
        .ok_or(ParseError::SyntaxError {
            line: line_num,
            message: "Command must be enclosed in << >>".to_string(),
        })?
        .trim();

    // Parse specific commands
    if content.starts_with("set ") {
        return parse_set_command(content, line_num).map(CommandParsed::Statement);
    }
    if content.starts_with("jump ") {
        let target = content.strip_prefix("jump ").unwrap().trim().to_string();
        return Ok(CommandParsed::Statement(YarnStatement::Jump(target)));
    }
    if content.starts_with("if ") {
        let cond_str = content.strip_prefix("if ").unwrap().trim();
        let condition = parse_expression(cond_str)?;
        return Ok(CommandParsed::If(condition));
    }
    if content == "else" {
        return Ok(CommandParsed::Else);
    }
    if content == "endif" {
        return Ok(CommandParsed::EndIf);
    }

    // Generic command
    let parts: Vec<String> = parse_command_args(content);
    if parts.is_empty() {
        return Err(ParseError::SyntaxError {
            line: line_num,
            message: "Empty command".to_string(),
        });
    }

    Ok(CommandParsed::Statement(YarnStatement::Command {
        name: parts[0].clone(),
        args: parts[1..].to_vec(),
    }))
}

fn parse_command_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ' ' if !in_quotes => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }

    args
}

fn parse_set_command(content: &str, _line_num: usize) -> Result<YarnStatement, ParseError> {
    // <<set $var_name to value>>
    let rest = content.strip_prefix("set ").unwrap().trim();

    // Find "to" keyword
    if let Some(to_pos) = rest.find(" to ") {
        let var_name = rest[..to_pos].trim().trim_start_matches('$').to_string();
        let value_str = rest[to_pos + 4..].trim();
        let value = parse_expression(value_str)?;
        Ok(YarnStatement::SetVariable {
            name: var_name,
            value,
        })
    } else if let Some(eq_pos) = rest.find(" = ") {
        // Also support <<set $var = value>> syntax
        let var_name = rest[..eq_pos].trim().trim_start_matches('$').to_string();
        let value_str = rest[eq_pos + 3..].trim();
        let value = parse_expression(value_str)?;
        Ok(YarnStatement::SetVariable {
            name: var_name,
            value,
        })
    } else {
        Err(ParseError::SyntaxError {
            line: _line_num,
            message: format!("Invalid set command: {content}"),
        })
    }
}

fn parse_conditional(
    lines: &[&str],
    start: usize,
    condition: YarnExpression,
    indent_level: usize,
) -> Result<(YarnStatement, usize), ParseError> {
    let mut pos = start + 1;

    // Parse then-body
    let (then_body, next_pos) = parse_body(lines, pos, indent_level)?;
    pos = next_pos;

    // Check for <<else>>
    let else_body = if pos < lines.len() && lines[pos].trim() == "<<else>>" {
        pos += 1;
        let (body, next_pos) = parse_body(lines, pos, indent_level)?;
        pos = next_pos;
        Some(body)
    } else {
        None
    };

    // Expect <<endif>>
    if pos < lines.len() && lines[pos].trim() == "<<endif>>" {
        pos += 1;
    }

    Ok((
        YarnStatement::Conditional {
            condition,
            then_body,
            else_body,
        },
        pos,
    ))
}

fn parse_dialogue_line(trimmed: &str) -> YarnStatement {
    // Check for Speaker: Text format
    if let Some(colon_pos) = trimmed.find(':') {
        let potential_speaker = &trimmed[..colon_pos];
        // Speaker should be a simple name (no spaces at start, no special chars)
        if !potential_speaker.is_empty()
            && !potential_speaker.starts_with(' ')
            && !potential_speaker.contains('{')
            && !potential_speaker.contains('<')
            && potential_speaker
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == ' ')
        {
            let text = trimmed[colon_pos + 1..].trim().to_string();
            let text = interpolate_variables(&text);
            return YarnStatement::Line {
                speaker: Some(potential_speaker.to_string()),
                text,
            };
        }
    }

    let text = interpolate_variables(trimmed);
    YarnStatement::Line {
        speaker: None,
        text,
    }
}

fn interpolate_variables(text: &str) -> String {
    // Replace {$var_name} with a placeholder marker.
    // Actual interpolation happens at runtime in the dialogue runner.
    // For now, keep the syntax as-is for the runner to process.
    text.to_string()
}

fn parse_condition_string(cond_str: &str) -> Result<YarnExpression, ParseError> {
    // Handles "if $var" style conditions in choice guards
    let cond_str = cond_str.strip_prefix("if ").unwrap_or(cond_str).trim();
    parse_expression(cond_str)
}

fn parse_expression(expr: &str) -> Result<YarnExpression, ParseError> {
    let expr = expr.trim();

    // Handle "not" prefix
    if let Some(rest) = expr.strip_prefix("not ") {
        let inner = parse_expression(rest)?;
        return Ok(YarnExpression::Not(Box::new(inner)));
    }
    if let Some(rest) = expr.strip_prefix('!') {
        let inner = parse_expression(rest)?;
        return Ok(YarnExpression::Not(Box::new(inner)));
    }

    // Handle "and" / "or" (lowest precedence, left-to-right)
    if let Some(pos) = find_logical_op(expr, " and ") {
        let left = parse_expression(&expr[..pos])?;
        let right = parse_expression(&expr[pos + 5..])?;
        return Ok(YarnExpression::And(Box::new(left), Box::new(right)));
    }
    if let Some(pos) = find_logical_op(expr, " or ") {
        let left = parse_expression(&expr[..pos])?;
        let right = parse_expression(&expr[pos + 4..])?;
        return Ok(YarnExpression::Or(Box::new(left), Box::new(right)));
    }

    // Handle comparison operators
    for (op_str, op) in &[
        ("==", ComparisonOp::Equal),
        ("!=", ComparisonOp::NotEqual),
        ("<=", ComparisonOp::LessEqual),
        (">=", ComparisonOp::GreaterEqual),
        ("<", ComparisonOp::LessThan),
        (">", ComparisonOp::GreaterThan),
    ] {
        if let Some(pos) = expr.find(op_str) {
            let left = parse_expression(&expr[..pos])?;
            let right = parse_expression(&expr[pos + op_str.len()..])?;
            return Ok(YarnExpression::Comparison(
                Box::new(left),
                *op,
                Box::new(right),
            ));
        }
    }

    // Literals and variables
    parse_atom(expr)
}

fn find_logical_op(expr: &str, op: &str) -> Option<usize> {
    // Find logical operator not inside quotes
    let mut in_quotes = false;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();

    for i in 0..expr.len() {
        if bytes[i] == b'"' {
            in_quotes = !in_quotes;
        }
        if !in_quotes && i + op_bytes.len() <= bytes.len() && &bytes[i..i + op_bytes.len()] == op_bytes
        {
            return Some(i);
        }
    }
    None
}

fn parse_atom(expr: &str) -> Result<YarnExpression, ParseError> {
    let expr = expr.trim();

    if expr.is_empty() {
        return Err(ParseError::SyntaxError {
            line: 0,
            message: "Empty expression".to_string(),
        });
    }

    // Variable reference
    if let Some(var_name) = expr.strip_prefix('$') {
        return Ok(YarnExpression::Variable(var_name.to_string()));
    }

    // Boolean literals
    if expr == "true" {
        return Ok(YarnExpression::Literal(YarnValue::Bool(true)));
    }
    if expr == "false" {
        return Ok(YarnExpression::Literal(YarnValue::Bool(false)));
    }

    // Number literal
    if let Ok(n) = expr.parse::<f64>() {
        return Ok(YarnExpression::Literal(YarnValue::Number(n)));
    }

    // String literal (quoted)
    if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
        let inner = expr[1..expr.len() - 1].to_string();
        return Ok(YarnExpression::Literal(YarnValue::String(inner)));
    }

    // Bare word — treat as variable (common in condition guards)
    Ok(YarnExpression::Variable(expr.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YARN: &str = r#"title: Herbalist_Greeting
tags: greeting shop
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
"#;

    #[test]
    fn test_parse_basic_yarn() {
        let nodes = parse_yarn_file(SAMPLE_YARN).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].title, "Herbalist_Greeting");
        assert_eq!(nodes[0].tags, vec!["greeting", "shop"]);
    }

    #[test]
    fn test_parse_dialogue_line() {
        let nodes = parse_yarn_file(SAMPLE_YARN).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::Line { speaker, text } => {
                assert_eq!(speaker.as_deref(), Some("Herbalist"));
                assert!(text.contains("Welcome to my shop"));
            }
            other => panic!("Expected Line, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_choices() {
        let nodes = parse_yarn_file(SAMPLE_YARN).unwrap();
        let body = &nodes[0].body;

        // First choice (no condition)
        match &body[1] {
            YarnStatement::Choice {
                text, condition, ..
            } => {
                assert_eq!(text, "I'm looking for rare seeds.");
                assert!(condition.is_none());
            }
            other => panic!("Expected Choice, got {other:?}"),
        }

        // Third choice (with condition)
        match &body[3] {
            YarnStatement::Choice {
                text, condition, ..
            } => {
                assert_eq!(text, "I found this moonpetal in the caves.");
                assert!(condition.is_some());
                match condition.as_ref().unwrap() {
                    YarnExpression::Variable(name) => assert_eq!(name, "has_moonpetal"),
                    other => panic!("Expected Variable, got {other:?}"),
                }
            }
            other => panic!("Expected Choice, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_set_command() {
        let nodes = parse_yarn_file(SAMPLE_YARN).unwrap();
        // Find set command inside first choice body
        if let YarnStatement::Choice { body, .. } = &nodes[0].body[1] {
            match &body[1] {
                YarnStatement::SetVariable { name, value } => {
                    assert_eq!(name, "visited_herbalist");
                    assert_eq!(*value, YarnExpression::Literal(YarnValue::Bool(true)));
                }
                other => panic!("Expected SetVariable, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_parse_jump() {
        let nodes = parse_yarn_file(SAMPLE_YARN).unwrap();
        if let YarnStatement::Choice { body, .. } = &nodes[0].body[1] {
            match &body[2] {
                YarnStatement::Jump(target) => assert_eq!(target, "Herbalist_SeedShop"),
                other => panic!("Expected Jump, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_parse_custom_command() {
        let nodes = parse_yarn_file(SAMPLE_YARN).unwrap();
        if let YarnStatement::Choice { body, .. } = &nodes[0].body[3] {
            match &body[1] {
                YarnStatement::Command { name, args } => {
                    assert_eq!(name, "give_item");
                    assert_eq!(args, &["moonpetal_seeds", "3"]);
                }
                other => panic!("Expected Command, got {other:?}"),
            }
        }
    }

    #[test]
    fn test_parse_conditional() {
        let yarn = r#"title: Test
---
<<if $quest_done>>
Player: I already did that.
<<else>>
Player: I haven't done that yet.
<<endif>>
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::Conditional {
                condition,
                then_body,
                else_body,
            } => {
                assert!(matches!(condition, YarnExpression::Variable(v) if v == "quest_done"));
                assert_eq!(then_body.len(), 1);
                assert!(else_body.is_some());
                assert_eq!(else_body.as_ref().unwrap().len(), 1);
            }
            other => panic!("Expected Conditional, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_comparison_expression() {
        let expr = parse_expression("$gold >= 100").unwrap();
        match expr {
            YarnExpression::Comparison(left, op, right) => {
                assert!(matches!(*left, YarnExpression::Variable(ref v) if v == "gold"));
                assert_eq!(op, ComparisonOp::GreaterEqual);
                assert!(matches!(*right, YarnExpression::Literal(YarnValue::Number(n)) if n == 100.0));
            }
            other => panic!("Expected Comparison, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_not_expression() {
        let expr = parse_expression("not $visited").unwrap();
        match expr {
            YarnExpression::Not(inner) => {
                assert!(matches!(*inner, YarnExpression::Variable(ref v) if v == "visited"));
            }
            other => panic!("Expected Not, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_and_expression() {
        let expr = parse_expression("$has_key and $has_map").unwrap();
        assert!(matches!(expr, YarnExpression::And(_, _)));
    }

    #[test]
    fn test_parse_or_expression() {
        let expr = parse_expression("$is_warrior or $is_knight").unwrap();
        assert!(matches!(expr, YarnExpression::Or(_, _)));
    }

    #[test]
    fn test_parse_multiple_nodes() {
        let yarn = r#"title: Node1
---
Speaker: Hello from node 1.
===

title: Node2
tags: second
---
Speaker: Hello from node 2.
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].title, "Node1");
        assert_eq!(nodes[1].title, "Node2");
        assert_eq!(nodes[1].tags, vec!["second"]);
    }

    #[test]
    fn test_parse_line_without_speaker() {
        let yarn = r#"title: Test
---
This is narration without a speaker.
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::Line { speaker, text } => {
                assert!(speaker.is_none());
                assert_eq!(text, "This is narration without a speaker.");
            }
            other => panic!("Expected Line, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_variable_interpolation() {
        let yarn = r#"title: Test
---
NPC: Hello {$player_name}, welcome to {$town_name}!
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::Line { text, .. } => {
                assert!(text.contains("{$player_name}"));
                assert!(text.contains("{$town_name}"));
            }
            other => panic!("Expected Line, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_string_value_in_set() {
        let yarn = r#"title: Test
---
<<set $name to "Apothecary">>
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::SetVariable { name, value } => {
                assert_eq!(name, "name");
                assert_eq!(
                    *value,
                    YarnExpression::Literal(YarnValue::String("Apothecary".to_string()))
                );
            }
            other => panic!("Expected SetVariable, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_number_value_in_set() {
        let yarn = r#"title: Test
---
<<set $gold to 100>>
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::SetVariable { name, value } => {
                assert_eq!(name, "gold");
                assert_eq!(*value, YarnExpression::Literal(YarnValue::Number(100.0)));
            }
            other => panic!("Expected SetVariable, got {other:?}"),
        }
    }

    #[test]
    fn test_empty_file_error() {
        let result = parse_yarn_file("");
        assert!(matches!(result, Err(ParseError::NoNodes)));
    }

    #[test]
    fn test_command_with_quoted_args() {
        let yarn = r#"title: Test
---
<<give_item "healing_potion" 5>>
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::Command { name, args } => {
                assert_eq!(name, "give_item");
                assert_eq!(args, &["healing_potion", "5"]);
            }
            other => panic!("Expected Command, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_set_with_equals() {
        let yarn = r#"title: Test
---
<<set $counter = 42>>
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::SetVariable { name, value } => {
                assert_eq!(name, "counter");
                assert_eq!(*value, YarnExpression::Literal(YarnValue::Number(42.0)));
            }
            other => panic!("Expected SetVariable, got {other:?}"),
        }
    }

    #[test]
    fn test_conditional_without_else() {
        let yarn = r#"title: Test
---
<<if $has_key>>
NPC: You found the key!
<<endif>>
===
"#;
        let nodes = parse_yarn_file(yarn).unwrap();
        match &nodes[0].body[0] {
            YarnStatement::Conditional {
                then_body,
                else_body,
                ..
            } => {
                assert_eq!(then_body.len(), 1);
                assert!(else_body.is_none());
            }
            other => panic!("Expected Conditional, got {other:?}"),
        }
    }
}
