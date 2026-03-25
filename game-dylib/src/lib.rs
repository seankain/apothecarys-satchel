//! Wrapper for hot-reloadable plugin.
use apothecarys_satchel::{fyrox::plugin::Plugin, GamePlugin};

#[no_mangle]
pub fn fyrox_plugin() -> Box<dyn Plugin> {
    Box::new(GamePlugin::default())
}
