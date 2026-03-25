//! Thin wrapper crate that re-exports the game plugin for the Fyrox editor
//! infrastructure (editor, executor, game-dylib).

// Re-export the engine so downstream crates can access it.
pub use fyrox;

// Re-export the game plugin.
pub use apothecarys_game::app::GamePlugin;
