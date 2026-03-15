use fyrox::{
    core::{
        log::{Log, MessageKind},
        pool::Handle,
        reflect::prelude::*,
        visitor::prelude::*,
    },
    plugin::{Plugin, PluginContext, PluginRegistrationContext},
    scene::Scene,
};

/// High-level game states corresponding to distinct gameplay modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Visit, Reflect)]
pub enum GameState {
    Menu,
    Hub,
    Dungeon,
    Combat,
    Garden,
}

impl Default for GameState {
    fn default() -> Self {
        Self::Menu
    }
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
}

impl GamePlugin {
    pub fn new() -> Self {
        Self {
            state: GameState::Menu,
            scene_handle: Handle::NONE,
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

        Log::writeln(
            MessageKind::Information,
            format!("Game initialized in {:?} state", self.state),
        );
    }

    fn update(&mut self, _context: &mut PluginContext) {
        // Game state update dispatch will be implemented in later phases
    }
}
