//! Inventory screen UI state management.

use apothecarys_core::items::{Item, ItemStack, ItemType};
use apothecarys_inventory::container::Inventory;

/// State for the inventory UI screen.
pub struct InventoryUiState {
    pub visible: bool,
    pub selected_slot: Option<usize>,
    pub context_menu_open: bool,
}

impl InventoryUiState {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected_slot: None,
            context_menu_open: false,
        }
    }

    /// Toggle inventory screen visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.selected_slot = None;
            self.context_menu_open = false;
        }
    }

    /// Select a slot for inspection.
    pub fn select_slot(&mut self, index: usize) {
        self.selected_slot = Some(index);
        self.context_menu_open = false;
    }

    /// Open context menu for the selected slot.
    pub fn open_context_menu(&mut self) {
        if self.selected_slot.is_some() {
            self.context_menu_open = true;
        }
    }

    /// Close context menu.
    pub fn close_context_menu(&mut self) {
        self.context_menu_open = false;
    }

    /// Get actions available for the selected item.
    pub fn available_actions(&self, inventory: &Inventory) -> Vec<ItemAction> {
        let slot_idx = match self.selected_slot {
            Some(i) => i,
            None => return Vec::new(),
        };

        let stack = match inventory.get_slot(slot_idx) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let mut actions = vec![ItemAction::Inspect];

        match &stack.item.item_type {
            ItemType::Potion { .. } | ItemType::Medicine { .. } => {
                actions.push(ItemAction::Use);
            }
            ItemType::Equipment(_) => {
                actions.push(ItemAction::Equip);
            }
            ItemType::Seed => {
                actions.push(ItemAction::Plant);
            }
            _ => {}
        }

        actions.push(ItemAction::Drop);
        actions
    }
}

impl Default for InventoryUiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Actions the player can perform on an inventory item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAction {
    Inspect,
    Use,
    Equip,
    Plant,
    Drop,
    Give,
}

/// Format item details for display.
pub fn format_item_details(stack: &ItemStack) -> ItemDetails {
    ItemDetails {
        name: stack.item.name.clone(),
        description: stack.item.description.clone(),
        count: stack.count,
        item_type_label: item_type_label(&stack.item.item_type),
        effects: format_effects(&stack.item),
    }
}

/// Displayable item information.
#[derive(Debug, Clone)]
pub struct ItemDetails {
    pub name: String,
    pub description: String,
    pub count: u32,
    pub item_type_label: String,
    pub effects: Vec<String>,
}

fn item_type_label(item_type: &ItemType) -> String {
    match item_type {
        ItemType::PlantSample => "Plant Sample".to_string(),
        ItemType::Potion { .. } => "Potion".to_string(),
        ItemType::Medicine { .. } => "Medicine".to_string(),
        ItemType::Ingredient => "Ingredient".to_string(),
        ItemType::Equipment(data) => format!("Equipment ({:?})", data.slot),
        ItemType::QuestItem => "Quest Item".to_string(),
        ItemType::Seed => "Seed".to_string(),
        ItemType::Gold(amount) => format!("Gold ({amount})"),
    }
}

fn format_effects(item: &Item) -> Vec<String> {
    match &item.item_type {
        ItemType::Potion { effects } => effects
            .iter()
            .map(|e| match e {
                apothecarys_core::items::AlchemyEffect::Heal { amount } => {
                    format!("Heal {amount} HP")
                }
                apothecarys_core::items::AlchemyEffect::Damage {
                    amount,
                    damage_type,
                } => format!("Deal {amount} {damage_type:?} damage"),
                apothecarys_core::items::AlchemyEffect::Buff { effect, turns } => {
                    format!("{effect:?} for {turns} turns")
                }
                apothecarys_core::items::AlchemyEffect::Cure { cures } => {
                    format!("Cure {cures:?}")
                }
                apothecarys_core::items::AlchemyEffect::StatBoost {
                    attribute,
                    amount,
                    turns,
                } => format!("+{amount} {attribute:?} for {turns} turns"),
            })
            .collect(),
        ItemType::Equipment(data) => {
            let mut effects = Vec::new();
            if data.armor_bonus != 0 {
                effects.push(format!("Armor +{}", data.armor_bonus));
            }
            if data.attack_bonus != 0 {
                effects.push(format!("Attack +{}", data.attack_bonus));
            }
            effects
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_core::items::{AlchemyEffect, EquipmentData, EquipmentSlot};

    #[test]
    fn test_inventory_ui_toggle() {
        let mut ui = InventoryUiState::new();
        assert!(!ui.visible);

        ui.toggle();
        assert!(ui.visible);

        ui.select_slot(0);
        ui.toggle();
        assert!(!ui.visible);
        assert!(ui.selected_slot.is_none());
    }

    #[test]
    fn test_inventory_ui_select_slot() {
        let mut ui = InventoryUiState::new();
        ui.select_slot(3);
        assert_eq!(ui.selected_slot, Some(3));
    }

    #[test]
    fn test_inventory_ui_context_menu() {
        let mut ui = InventoryUiState::new();
        ui.open_context_menu();
        assert!(!ui.context_menu_open); // No slot selected

        ui.select_slot(0);
        ui.open_context_menu();
        assert!(ui.context_menu_open);

        ui.close_context_menu();
        assert!(!ui.context_menu_open);
    }

    #[test]
    fn test_available_actions_potion() {
        let mut ui = InventoryUiState::new();
        let mut inv = Inventory::new(10);
        let potion = Item::new(
            "heal",
            "Healing Potion",
            ItemType::Potion {
                effects: vec![AlchemyEffect::Heal { amount: 10 }],
            },
        );
        inv.add_item(potion, 1);
        ui.select_slot(0);

        let actions = ui.available_actions(&inv);
        assert!(actions.contains(&ItemAction::Inspect));
        assert!(actions.contains(&ItemAction::Use));
        assert!(actions.contains(&ItemAction::Drop));
    }

    #[test]
    fn test_available_actions_equipment() {
        let mut ui = InventoryUiState::new();
        let mut inv = Inventory::new(10);
        let sword = Item::new(
            "sword",
            "Sword",
            ItemType::Equipment(EquipmentData {
                slot: EquipmentSlot::Weapon,
                armor_bonus: 0,
                attack_bonus: 2,
            }),
        );
        inv.add_item(sword, 1);
        ui.select_slot(0);

        let actions = ui.available_actions(&inv);
        assert!(actions.contains(&ItemAction::Equip));
    }

    #[test]
    fn test_available_actions_empty_slot() {
        let ui = InventoryUiState {
            visible: true,
            selected_slot: Some(0),
            context_menu_open: false,
        };
        let inv = Inventory::new(10);
        let actions = ui.available_actions(&inv);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_format_item_details_potion() {
        let potion = Item::new(
            "heal",
            "Healing Potion",
            ItemType::Potion {
                effects: vec![AlchemyEffect::Heal { amount: 15 }],
            },
        );
        let stack = ItemStack::single(potion);
        let details = format_item_details(&stack);
        assert_eq!(details.name, "Healing Potion");
        assert_eq!(details.item_type_label, "Potion");
        assert_eq!(details.effects.len(), 1);
        assert!(details.effects[0].contains("15"));
    }

    #[test]
    fn test_format_item_details_equipment() {
        let armor = Item::new(
            "armor",
            "Chainmail",
            ItemType::Equipment(EquipmentData {
                slot: EquipmentSlot::Armor,
                armor_bonus: 3,
                attack_bonus: 0,
            }),
        );
        let stack = ItemStack::single(armor);
        let details = format_item_details(&stack);
        assert_eq!(details.item_type_label, "Equipment (Armor)");
        assert!(!details.effects.is_empty());
    }

    #[test]
    fn test_item_type_labels() {
        assert_eq!(item_type_label(&ItemType::Ingredient), "Ingredient");
        assert_eq!(item_type_label(&ItemType::Seed), "Seed");
        assert_eq!(item_type_label(&ItemType::QuestItem), "Quest Item");
        assert_eq!(item_type_label(&ItemType::Gold(100)), "Gold (100)");
    }
}
