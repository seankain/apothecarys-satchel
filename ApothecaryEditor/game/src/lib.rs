//! Game project.
#[allow(unused_imports)]
use fyrox::graph::prelude::*;
use fyrox::{
    core::pool::Handle, core::visitor::prelude::*, core::reflect::prelude::*,
    event::Event,
    gui::{message::UiMessage, UserInterface},
    plugin::{Plugin, PluginContext, PluginRegistrationContext, error::GameResult},
    scene::Scene,
};
use std::path::Path;

// Re-export the engine.
pub use fyrox;

#[derive(Default, Visit, Reflect, Debug)]
#[reflect(non_cloneable)]
pub struct Game {
    scene: Handle<Scene>,
}

impl Plugin for Game {
    fn register(&self, _context: PluginRegistrationContext) -> GameResult {
        // Register your scripts here.
        Ok(())
    }

    fn init(&mut self, scene_path: Option<&str>, context: PluginContext) -> GameResult {
        context
            .async_scene_loader
            .request(scene_path.unwrap_or("data/scene.rgs"));
        Ok(())
    }

    fn on_deinit(&mut self, _context: PluginContext) -> GameResult {
        // Do a cleanup here.
        Ok(())
    }

    fn update(&mut self, _context: &mut PluginContext) -> GameResult {
        // Add your global update code here.
        Ok(())
    }

    fn on_os_event(
        &mut self,
        _event: &Event<()>,
        _context: PluginContext,
    ) -> GameResult {
        // Do something on OS event here.
        Ok(())
    }

    fn on_ui_message(
        &mut self,
        _context: &mut PluginContext,
        _message: &UiMessage,
        _ui_handle: Handle<UserInterface>
    ) -> GameResult {
        // Handle UI events here.
        Ok(())
    }

    fn on_scene_begin_loading(&mut self, _path: &Path, ctx: &mut PluginContext) -> GameResult {
        if self.scene.is_some() {
            ctx.scenes.remove(self.scene);
        }
        Ok(())
    }

    fn on_scene_loaded(
        &mut self,
        _path: &Path,
        scene: Handle<Scene>,
        _data: &[u8],
        _context: &mut PluginContext,
    ) -> GameResult {
        self.scene = scene;
        Ok(())
    }
}
