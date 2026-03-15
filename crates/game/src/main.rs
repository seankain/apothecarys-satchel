use fyrox::engine::executor::Executor;
use fyrox::window::WindowAttributes;
use fyrox::engine::GraphicsContextParams;
use fyrox::event_loop::EventLoop;

use apothecarys_game::app::GamePlugin;

fn main() {
    let mut window_attributes = WindowAttributes::default();
    window_attributes.title = "The Apothecary's Satchel".to_string();
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

    executor.add_plugin(GamePlugin::new());
    executor.run();
}
