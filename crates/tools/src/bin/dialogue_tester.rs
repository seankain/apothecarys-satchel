use std::env;

use fyrox::{
    core::{
        algebra::Vector2,
        log::{Log, MessageKind},
        pool::Handle,
        reflect::prelude::*,
        visitor::prelude::*,
    },
    engine::{executor::Executor, GraphicsContextParams},
    event_loop::EventLoop,
    gui::{
        button::{ButtonBuilder, ButtonMessage},
        formatted_text::WrapMode,
        grid::{Column, GridBuilder, Row},
        list_view::ListViewBuilder,
        message::{MessageDirection, UiMessage},
        text::TextBuilder,
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        Thickness, UiNode,
    },
    plugin::{Plugin, PluginContext, PluginRegistrationContext},
    window::WindowAttributes,
};

use apothecarys_dialogue::runner::DialogueState;
use apothecarys_tools::dialogue_tester::DialogueTesterState;

/// Plugin for the dialogue tester tool.
#[derive(Default, Visit, Reflect, Debug)]
struct DialogueTesterPlugin {
    #[visit(skip)]
    #[reflect(hidden)]
    tester_state: Option<DialogueTesterState>,

    #[visit(skip)]
    #[reflect(hidden)]
    file_path: Option<String>,

    // UI handles
    #[visit(skip)]
    #[reflect(hidden)]
    node_list: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    dialogue_text: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    advance_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    variable_text: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    command_log_text: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    status_text: Handle<UiNode>,
}

impl DialogueTesterPlugin {
    fn new(file_path: Option<String>) -> Self {
        Self {
            file_path,
            ..Default::default()
        }
    }

    fn build_ui(&mut self, context: &mut PluginContext) {
        let ctx = &mut context.user_interfaces.first_mut().build_ctx();

        // Node browser list
        self.node_list = ListViewBuilder::new(
            WidgetBuilder::new()
                .with_width(180.0)
                .with_height(150.0),
        )
        .build(ctx);

        // Dialogue display area
        self.dialogue_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(200.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("")
        .with_wrap(WrapMode::Word)
        .build(ctx);

        // Advance button
        self.advance_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(100.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Advance")
        .build(ctx);

        // Variable inspector
        self.variable_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(100.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Variables:\n(none)")
        .with_wrap(WrapMode::Word)
        .build(ctx);

        // Command log
        self.command_log_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(80.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Command Log:\n(empty)")
        .with_wrap(WrapMode::Word)
        .build(ctx);

        // Status text
        self.status_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(24.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Dialogue Tester - Ready")
        .with_wrap(WrapMode::NoWrap)
        .build(ctx);

        let content = GridBuilder::new(
            WidgetBuilder::new()
                .with_child(self.node_list)
                .with_child(self.dialogue_text)
                .with_child(self.advance_button)
                .with_child(self.variable_text)
                .with_child(self.command_log_text)
                .with_child(self.status_text),
        )
        .add_row(Row::strict(160.0))
        .add_row(Row::stretch())
        .add_row(Row::strict(32.0))
        .add_row(Row::strict(110.0))
        .add_row(Row::strict(90.0))
        .add_row(Row::strict(28.0))
        .add_column(Column::stretch())
        .build(ctx);

        WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(400.0)
                .with_height(600.0)
                .with_desired_position(Vector2::new(10.0, 10.0)),
        )
        .with_title(WindowTitle::text("Dialogue Tester"))
        .with_content(content)
        .open(false)
        .build(ctx);
    }

    fn update_dialogue_display(&self, context: &mut PluginContext) {
        let state = match &self.tester_state {
            Some(s) => s,
            None => return,
        };

        // Build dialogue text from history
        let mut text = String::new();
        for line in &state.line_history {
            if let Some(speaker) = &line.speaker {
                text.push_str(&format!("{speaker}: {}\n", line.text));
            } else {
                text.push_str(&format!("{}\n", line.text));
            }
        }

        // Show current state
        match state.state() {
            DialogueState::WaitingForChoice { choices } => {
                text.push_str("\n--- Choices ---\n");
                for choice in choices {
                    let enabled = if choice.enabled { "" } else { " [disabled]" };
                    text.push_str(&format!("  {}. {}{}\n", choice.index + 1, choice.text, enabled));
                }
            }
            DialogueState::Finished => {
                text.push_str("\n[Dialogue Finished]");
            }
            _ => {}
        }

        if self.dialogue_text.is_some() {
            context
                .user_interfaces
                .first_mut()
                .send_message(fyrox::gui::text::TextMessage::text(
                    self.dialogue_text,
                    MessageDirection::ToWidget,
                    text,
                ));
        }

        // Update variable display
        let vars = state.variables();
        let mut var_text = "Variables:\n".to_string();
        if vars.is_empty() {
            var_text.push_str("(none)");
        } else {
            for (name, value) in &vars {
                var_text.push_str(&format!(
                    "  ${name} = {}\n",
                    DialogueTesterState::format_value(value)
                ));
            }
        }
        if self.variable_text.is_some() {
            context
                .user_interfaces
                .first_mut()
                .send_message(fyrox::gui::text::TextMessage::text(
                    self.variable_text,
                    MessageDirection::ToWidget,
                    var_text,
                ));
        }

        // Update command log
        let mut cmd_text = "Command Log:\n".to_string();
        if state.command_log.is_empty() {
            cmd_text.push_str("(empty)");
        } else {
            for entry in &state.command_log {
                cmd_text.push_str(&format!(
                    "  {} {}\n",
                    entry.command,
                    entry.args.join(" ")
                ));
            }
        }
        if self.command_log_text.is_some() {
            context
                .user_interfaces
                .first_mut()
                .send_message(fyrox::gui::text::TextMessage::text(
                    self.command_log_text,
                    MessageDirection::ToWidget,
                    cmd_text,
                ));
        }
    }

    fn update_status(&self, context: &mut PluginContext, message: &str) {
        if self.status_text.is_some() {
            context
                .user_interfaces
                .first_mut()
                .send_message(fyrox::gui::text::TextMessage::text(
                    self.status_text,
                    MessageDirection::ToWidget,
                    message.to_string(),
                ));
        }
    }
}

impl Plugin for DialogueTesterPlugin {
    fn register(&self, _context: PluginRegistrationContext) {
        Log::writeln(
            MessageKind::Information,
            "Dialogue Tester tool registered",
        );
    }

    fn init(&mut self, _scene_path: Option<&str>, mut context: PluginContext) {
        let mut state = DialogueTesterState::new();

        if let Some(path) = &self.file_path {
            match state.load_yarn_file(path) {
                Ok(titles) => {
                    Log::writeln(
                        MessageKind::Information,
                        format!("Loaded {} nodes from {path}", titles.len()),
                    );
                    for title in &titles {
                        Log::writeln(
                            MessageKind::Information,
                            format!("  Node: {title}"),
                        );
                    }
                    // Auto-start the first node
                    if let Some(first) = titles.first() {
                        state.start_node(first);
                        Log::writeln(
                            MessageKind::Information,
                            format!("Auto-started node: {first}"),
                        );
                    }
                }
                Err(e) => {
                    Log::writeln(
                        MessageKind::Warning,
                        format!("Could not load {path}: {e}"),
                    );
                }
            }
        }

        self.tester_state = Some(state);
        self.build_ui(&mut context);
    }

    fn update(&mut self, _context: &mut PluginContext) {}

    fn on_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.advance_button {
                if let Some(state) = &mut self.tester_state {
                    let dialogue_state = state.advance();
                    let status = match &dialogue_state {
                        DialogueState::ShowingLine { .. } => "Showing line",
                        DialogueState::WaitingForChoice { choices } => {
                            if choices.is_empty() {
                                "No choices"
                            } else {
                                "Waiting for choice"
                            }
                        }
                        DialogueState::ExecutingCommand { command, .. } => {
                            Log::writeln(
                                MessageKind::Information,
                                format!("Command: {command}"),
                            );
                            "Executing command"
                        }
                        DialogueState::Finished => "Finished",
                        DialogueState::Idle => "Idle",
                    };
                    self.update_status(context, status);
                    self.update_dialogue_display(context);
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).cloned();

    println!("Dialogue Tester");
    if let Some(path) = &file_path {
        println!("Loading: {path}");
    } else {
        println!("Usage: dialogue_tester [file.yarn]");
    }

    let mut window_attributes = WindowAttributes::default();
    window_attributes.title = "Dialogue Tester - Apothecary's Satchel".to_string();
    window_attributes.resizable = true;

    let mut executor = Executor::from_params(
        EventLoop::new().unwrap(),
        GraphicsContextParams {
            window_attributes,
            vsync: true,
            msaa_sample_count: None,
            graphics_server_constructor: Default::default(),
        },
    );

    executor.add_plugin(DialogueTesterPlugin::new(file_path));
    executor.run();
}
