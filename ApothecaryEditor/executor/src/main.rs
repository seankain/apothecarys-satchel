//! Executor with your game connected to it as a plugin.
use fyrox::engine::executor::Executor;
use fyrox::event_loop::EventLoop;
use fyrox::core::log::Log;

fn main() {
    Log::set_file_name("ApothecaryEditor.log");

    let mut executor = Executor::new(Some(EventLoop::new().unwrap()));

    // Dynamic linking with hot reloading.
    #[cfg(feature = "dylib")]
    {
        #[cfg(target_os = "windows")]
        let file_name = "game_dylib.dll";
        #[cfg(target_os = "linux")]
        let file_name = "libgame_dylib.so";
        #[cfg(target_os = "macos")]
        let file_name = "libgame_dylib.dylib";
        executor.add_dynamic_plugin(file_name, true, true).unwrap();
    }

    // Static linking.
    #[cfg(not(feature = "dylib"))]
    {
        use ApothecaryEditor::Game;
        executor.add_plugin(Game::default());
    }

    executor.run()
}