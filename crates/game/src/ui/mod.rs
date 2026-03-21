//! Game UI / HUD system for The Apothecary's Satchel.
//!
//! Provides UI state management for all game screens:
//! - HUD overlay (party HP bars, notifications)
//! - Inventory screen
//! - Combat UI (turn indicator, action menu, target selection)
//! - Crafting UI (recipe list, ingredient slots)
//! - Garden UI (plot grid, plant info)
//! - Dialogue UI (text box, choices)
//! - Recruitment UI (candidate list, stats)

pub mod combat_ui;
pub mod crafting_ui;
pub mod dialogue_ui;
pub mod garden_ui;
pub mod hud;
pub mod inventory_ui;
pub mod main_menu;
pub mod recruitment_ui;
