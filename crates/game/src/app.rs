use fyrox::{
    core::{
        log::{Log, MessageKind},
        pool::Handle,
        reflect::prelude::*,
        visitor::prelude::*,
    },
    gui::{button::ButtonMessage, message::UiMessage},
    plugin::{Plugin, PluginContext, PluginRegistrationContext},
    scene::Scene,
};

use crate::ui::main_menu::MainMenuState;

/// High-level game states corresponding to distinct gameplay modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Visit, Reflect)]
pub enum GameState {
    #[default]
    Menu,
    Hub,
    Dungeon,
    Combat,
    Garden,
}

/// The main Fyrox game plugin for The Apothecary's Satchel.
#[derive(Default, Visit, Reflect, Debug)]
pub struct GamePlugin {
    #[visit(skip)]
    #[reflect(hidden)]
    pub state: GameState,

    #[visit(skip)]
    #[reflect(hidden)]
    pub scene_handle: Handle<Scene>,

    #[visit(skip)]
    #[reflect(hidden)]
    main_menu: Option<MainMenuState>,
}

impl GamePlugin {
    pub fn new() -> Self {
        Self {
            state: GameState::Menu,
            scene_handle: Handle::NONE,
            main_menu: None,
        }
    }

    fn create_empty_scene(context: &mut PluginContext) -> Handle<Scene> {
        let scene = Scene::new();
        context.scenes.add(scene)
    }
}

impl Plugin for GamePlugin {
    fn register(&self, _context: PluginRegistrationContext) {
        Log::writeln(MessageKind::Information, "GamePlugin registered");
    }

    fn init(&mut self, _scene_path: Option<&str>, mut context: PluginContext) {
        Log::writeln(MessageKind::Information, "Initializing The Apothecary's Satchel...");

        self.scene_handle = Self::create_empty_scene(&mut context);
        self.state = GameState::Menu;

        let ui = context.user_interfaces.first_mut();
        self.main_menu = Some(MainMenuState::build(&mut ui.build_ctx()));

        Log::writeln(
            MessageKind::Information,
            format!("Game initialized in {:?} state", self.state),
        );
    }

    fn update(&mut self, _context: &mut PluginContext) {
        // Game state update dispatch will be implemented in later phases
    }

    fn on_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        let Some(ref menu) = self.main_menu else {
            return;
        };

        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == menu.start_button {
                Log::writeln(MessageKind::Information, "Start Game clicked");
                self.state = GameState::Hub;
                menu.set_visible(context.user_interfaces.first_mut(), false);
            } else if message.destination() == menu.exit_button {
                Log::writeln(MessageKind::Information, "Exit clicked");
                if let fyrox::engine::GraphicsContext::Initialized(ref gctx) =
                    *context.graphics_context
                {
                    gctx.window.set_visible(false);
                }
                std::process::exit(0);
            }
        }
    }
}
