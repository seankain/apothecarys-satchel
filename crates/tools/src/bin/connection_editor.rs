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
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        Orientation, Thickness, UiNode,
    },
    plugin::{Plugin, PluginContext, PluginRegistrationContext},
    window::WindowAttributes,
};

use apothecarys_tools::connection_editor::ConnectionEditorState;
use apothecarys_world::location::LocationType;

/// Plugin for the connection editor tool.
#[derive(Default, Visit, Reflect, Debug)]
struct ConnectionEditorPlugin {
    #[visit(skip)]
    #[reflect(hidden)]
    editor_state: Option<ConnectionEditorState>,

    #[visit(skip)]
    #[reflect(hidden)]
    file_path: Option<String>,

    // UI handles
    #[visit(skip)]
    #[reflect(hidden)]
    location_list: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    add_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    remove_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    connect_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    validate_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    save_button: Handle<UiNode>,

    #[visit(skip)]
    #[reflect(hidden)]
    status_text: Handle<UiNode>,
}

impl ConnectionEditorPlugin {
    fn new(file_path: Option<String>) -> Self {
        Self {
            file_path,
            ..Default::default()
        }
    }

    fn build_ui(&mut self, context: &mut PluginContext) {
        let ctx = &mut context.user_interfaces.first_mut().build_ctx();

        self.location_list = ListViewBuilder::new(
            WidgetBuilder::new()
                .with_width(250.0)
                .with_height(350.0),
        )
        .build(ctx);

        self.add_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(70.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Add")
        .build(ctx);

        self.remove_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(70.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Remove")
        .build(ctx);

        self.connect_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(70.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Connect")
        .build(ctx);

        self.validate_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(70.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Validate")
        .build(ctx);

        self.save_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(70.0)
                .with_height(28.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text("Save")
        .build(ctx);

        let button_panel = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(self.add_button)
                .with_child(self.remove_button)
                .with_child(self.connect_button)
                .with_child(self.validate_button)
                .with_child(self.save_button),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        self.status_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(24.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Connection Editor - Ready")
        .with_wrap(WrapMode::NoWrap)
        .build(ctx);

        let content = GridBuilder::new(
            WidgetBuilder::new()
                .with_child(self.location_list)
                .with_child(button_panel)
                .with_child(self.status_text),
        )
        .add_row(Row::stretch())
        .add_row(Row::strict(32.0))
        .add_row(Row::strict(28.0))
        .add_column(Column::stretch())
        .build(ctx);

        WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(400.0)
                .with_height(450.0)
                .with_desired_position(Vector2::new(10.0, 10.0)),
        )
        .with_title(WindowTitle::text("World Graph"))
        .with_content(content)
        .open(false)
        .build(ctx);
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

impl Plugin for ConnectionEditorPlugin {
    fn register(&self, _context: PluginRegistrationContext) {
        Log::writeln(
            MessageKind::Information,
            "Connection Editor tool registered",
        );
    }

    fn init(&mut self, _scene_path: Option<&str>, mut context: PluginContext) {
        let mut state = ConnectionEditorState::new();

        if let Some(path) = &self.file_path {
            match state.load_from_file(path) {
                Ok(()) => {
                    Log::writeln(
                        MessageKind::Information,
                        format!(
                            "Loaded {} locations from {path}",
                            state.node_count()
                        ),
                    );
                }
                Err(e) => {
                    Log::writeln(
                        MessageKind::Warning,
                        format!("Could not load {path}: {e}"),
                    );
                }
            }
        }

        self.editor_state = Some(state);
        self.build_ui(&mut context);
    }

    fn update(&mut self, _context: &mut PluginContext) {}

    fn on_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.validate_button {
                if let Some(state) = &self.editor_state {
                    let errors = state.validate();
                    if errors.is_empty() {
                        self.update_status(context, "Validation passed!");
                        Log::writeln(MessageKind::Information, "Validation passed");
                    } else {
                        let msg = format!("{} error(s) found", errors.len());
                        self.update_status(context, &msg);
                        for err in &errors {
                            Log::writeln(MessageKind::Warning, format!("Validation: {err}"));
                        }
                    }
                }
            } else if message.destination() == self.save_button {
                if let Some(state) = &self.editor_state {
                    let path = self
                        .file_path
                        .clone()
                        .unwrap_or_else(|| "data/locations.ron".to_string());
                    match state.save_to_file(&path) {
                        Ok(()) => {
                            self.update_status(context, &format!("Saved to {path}"));
                            Log::writeln(
                                MessageKind::Information,
                                format!("Saved to {path}"),
                            );
                        }
                        Err(e) => {
                            self.update_status(context, "Save failed!");
                            Log::writeln(
                                MessageKind::Error,
                                format!("Save failed: {e}"),
                            );
                        }
                    }
                }
            } else if message.destination() == self.add_button {
                if let Some(state) = &mut self.editor_state {
                    let count = state.node_count();
                    let id = format!("location_{count}");
                    let name = format!("New Location {count}");
                    let pos = apothecarys_tools::connection_editor::NodePosition::new(
                        (count % 4) as f32 * 250.0 + 50.0,
                        (count / 4) as f32 * 200.0 + 50.0,
                    );
                    state.add_node(&id, &name, LocationType::Town, pos);
                    self.update_status(context, &format!("Added {name}"));
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).cloned();

    println!("Connection Editor");
    if let Some(path) = &file_path {
        println!("Loading: {path}");
    }

    let mut window_attributes = WindowAttributes::default();
    window_attributes.title = "Connection Editor - Apothecary's Satchel".to_string();
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

    executor.add_plugin(ConnectionEditorPlugin::new(file_path));
    executor.run();
}
