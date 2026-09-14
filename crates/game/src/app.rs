use fyrox::{
    core::{
        log::{Log, MessageKind},
        pool::Handle,
        reflect::prelude::*,
        visitor::prelude::*,
    },
    event::{ElementState, Event, WindowEvent},
    gui::{button::ButtonMessage, message::UiMessage},
    keyboard::PhysicalKey,
    plugin::{Plugin, PluginContext, PluginRegistrationContext},
    scene::Scene,
};

use apothecarys_garden::plots::Garden;

use crate::camera::{IsoCameraConfig, IsometricCamera};
use crate::garden_scene::{build_garden_scene, GardenLayout};
use crate::hub_scene::build_hub_scene;
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

    #[visit(skip)]
    #[reflect(hidden)]
    camera: Option<IsometricCamera>,

    /// True while the menu overlay is visible during gameplay (Escape toggle).
    #[visit(skip)]
    #[reflect(hidden)]
    menu_open: bool,

    /// Where the plots sit and how far away a plant drops to the coarser tier.
    #[visit(skip)]
    #[reflect(hidden)]
    garden_layout: GardenLayout,

    /// The plant node for each plot, in plot order; `Handle::NONE` for an
    /// empty plot. Kept so a plot can be rebuilt on its own after planting or
    /// harvesting rather than rebuilding the whole garden.
    #[visit(skip)]
    #[reflect(hidden)]
    garden_plants: Vec<Handle<fyrox::scene::node::Node>>,
}

impl GamePlugin {
    pub fn new() -> Self {
        Self {
            state: GameState::Menu,
            scene_handle: Handle::NONE,
            main_menu: None,
            camera: None,
            menu_open: false,
            garden_layout: GardenLayout::default(),
            garden_plants: Vec::new(),
        }
    }

    fn create_empty_scene(context: &mut PluginContext) -> Handle<Scene> {
        let scene = Scene::new();
        context.scenes.add(scene)
    }

    /// Transition into the Hub state: replace the current scene with the hub
    /// blockout and set up the isometric camera.
    fn enter_hub(&mut self, context: &mut PluginContext) {
        // Remove old scene if any.
        if self.scene_handle.is_some() {
            context.scenes.remove(self.scene_handle);
        }

        let mut scene = build_hub_scene();

        // Add the isometric camera to the hub scene.
        let iso_camera = IsometricCamera::new(&mut scene, IsoCameraConfig::default());
        self.camera = Some(iso_camera);

        self.scene_handle = context.scenes.add(scene);
        self.state = GameState::Hub;
        self.garden_plants.clear();

        Log::writeln(MessageKind::Information, "Entered Hub state");
    }

    /// Transition into the Garden state: build the plot grid and grow every
    /// planted plot through `plantgl`.
    ///
    /// Plants are generated here, once, rather than per frame: a full garden
    /// costs a few milliseconds (`crates/botany/tests/budget.rs` holds it to
    /// 15 ms a plant), which belongs in the transition and not in `update`.
    pub fn enter_garden(&mut self, garden: &Garden, context: &mut PluginContext) {
        if self.scene_handle.is_some() {
            context.scenes.remove(self.scene_handle);
        }

        let config = IsoCameraConfig::default();
        // The camera looks at the middle of the grid, so "near" is measured
        // from where the player actually is.
        let viewer = self.garden_layout.plot_position(garden.plots.len() / 2);
        let (mut scene, plants) = build_garden_scene(garden, &self.garden_layout, viewer);

        let iso_camera = IsometricCamera::new(&mut scene, config);
        self.camera = Some(iso_camera);
        self.garden_plants = plants;

        self.scene_handle = context.scenes.add(scene);
        self.state = GameState::Garden;

        Log::writeln(
            MessageKind::Information,
            format!(
                "Entered Garden state: {} plots, {} planted",
                garden.plots.len(),
                self.garden_plants.iter().filter(|h| h.is_some()).count()
            ),
        );
    }

    /// The plant node for a plot, if it has one.
    pub fn plant_node(&self, plot: usize) -> Option<Handle<fyrox::scene::node::Node>> {
        self.garden_plants
            .get(plot)
            .copied()
            .filter(|handle| handle.is_some())
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

    fn update(&mut self, context: &mut PluginContext) {
        // Update the isometric camera each frame when in a gameplay state.
        if self.state != GameState::Menu {
            if let Some(camera) = &mut self.camera {
                if let Some(scene) = context.scenes.try_get_mut(self.scene_handle) {
                    camera.update(scene, context.dt);
                }
            }
        }
    }

    fn on_os_event(&mut self, event: &Event<()>, context: PluginContext) {
        // Toggle main menu with Escape when not on the title-screen menu.
        if let Event::WindowEvent {
            event: WindowEvent::KeyboardInput { event, .. },
            ..
        } = event
        {
            if matches!(
                event.physical_key,
                PhysicalKey::Code(fyrox::keyboard::KeyCode::Escape)
            ) && event.state == ElementState::Pressed
                && !event.repeat
                && self.state != GameState::Menu
            {
                self.menu_open = !self.menu_open;
                if let Some(ref menu) = self.main_menu {
                    menu.set_visible(context.user_interfaces.first_mut(), self.menu_open);
                }
                Log::writeln(
                    MessageKind::Information,
                    format!(
                        "Menu toggled: {}",
                        if self.menu_open { "open" } else { "closed" }
                    ),
                );
            }
        }
    }

    fn on_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        let Some(ref menu) = self.main_menu else {
            return;
        };

        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == menu.start_button {
                Log::writeln(MessageKind::Information, "Start Game clicked");
                menu.set_visible(context.user_interfaces.first_mut(), false);
                self.menu_open = false;
                self.enter_hub(context);
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
