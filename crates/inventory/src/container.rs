use apothecarys_core::items::{Item, ItemStack};
use serde::{Deserialize, Serialize};

/// A generic inventory container with slot-based storage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
    pub max_slots: usize,
}

impl Inventory {
    pub fn new(max_slots: usize) -> Self {
        Self {
            slots: vec![None; max_slots],
            max_slots,
        }
    }

    /// Add an item to the inventory. Attempts to stack with existing compatible
    /// stacks first, then uses an empty slot. Returns the overflow count (items
    /// that couldn't be added due to capacity).
    pub fn add_item(&mut self, item: Item, count: u32) -> u32 {
        let mut remaining = count;

        // First, try to stack with existing compatible stacks
        if item.stackable {
            for slot in self.slots.iter_mut().flatten() {
                if slot.item.template_id == item.template_id {
                    remaining = slot.try_add(remaining);
                    if remaining == 0 {
                        return 0;
                    }
                }
            }
        }

        // Then, place remaining in empty slots
        while remaining > 0 {
            if let Some(empty_slot) = self.slots.iter_mut().find(|s| s.is_none()) {
                let to_place = if item.stackable {
                    remaining.min(item.max_stack)
                } else {
                    1
                };
                *empty_slot = Some(ItemStack::new(item.clone(), to_place));
                remaining -= to_place;
            } else {
                break;
            }
        }

        remaining
    }

    /// Remove items by template_id. Returns how many couldn't be removed.
    pub fn remove_item(&mut self, template_id: &str, count: u32) -> u32 {
        let mut remaining = count;

        for slot in self.slots.iter_mut() {
            if remaining == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.item.template_id == template_id {
                    remaining = stack.try_remove(remaining);
                    if stack.count == 0 {
                        *slot = None;
                    }
                }
            }
        }

        remaining
    }

    /// Check if the inventory contains at least `count` of the given item.
    pub fn has_item(&self, template_id: &str, count: u32) -> bool {
        self.get_count(template_id) >= count
    }

    /// Get the total count of an item across all stacks.
    pub fn get_count(&self, template_id: &str) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.item.template_id == template_id)
            .map(|s| s.count)
            .sum()
    }

    /// Get a reference to the item stack at the given slot index.
    pub fn get_slot(&self, index: usize) -> Option<&ItemStack> {
        self.slots.get(index).and_then(|s| s.as_ref())
    }

    /// Get a mutable reference to the item stack at the given slot index.
    pub fn get_slot_mut(&mut self, index: usize) -> Option<&mut ItemStack> {
        self.slots.get_mut(index).and_then(|s| s.as_mut())
    }

    /// Remove the entire stack at a given slot index and return it.
    pub fn take_slot(&mut self, index: usize) -> Option<ItemStack> {
        self.slots.get_mut(index).and_then(|s| s.take())
    }

    /// Returns the number of occupied slots.
    pub fn occupied_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Returns the number of empty slots.
    pub fn empty_slots(&self) -> usize {
        self.max_slots - self.occupied_slots()
    }

    /// Returns true if the inventory has no items.
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_none())
    }

    /// Returns true if all slots are occupied.
    pub fn is_full(&self) -> bool {
        self.slots.iter().all(|s| s.is_some())
    }

    /// Iterate over all non-empty item stacks with their slot indices.
    pub fn items(&self) -> impl Iterator<Item = (usize, &ItemStack)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|stack| (i, stack)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_core::items::ItemType;

    fn make_herb(name: &str) -> Item {
        Item::new(name, name, ItemType::Ingredient)
    }

    fn make_quest_item(name: &str) -> Item {
        Item::new(name, name, ItemType::QuestItem)
    }

    fn make_potion(name: &str) -> Item {
        Item::new(
            name,
            name,
            ItemType::Potion { effects: vec![] },
        )
    }

    #[test]
    fn test_new_inventory() {
        let inv = Inventory::new(10);
        assert_eq!(inv.max_slots, 10);
        assert_eq!(inv.slots.len(), 10);
        assert!(inv.is_empty());
        assert_eq!(inv.empty_slots(), 10);
    }

    #[test]
    fn test_add_single_item() {
        let mut inv = Inventory::new(5);
        let herb = make_herb("basic_herb");
        let overflow = inv.add_item(herb, 1);
        assert_eq!(overflow, 0);
        assert!(inv.has_item("basic_herb", 1));
        assert_eq!(inv.get_count("basic_herb"), 1);
        assert_eq!(inv.occupied_slots(), 1);
    }

    #[test]
    fn test_add_stackable_items() {
        let mut inv = Inventory::new(5);
        let herb = make_herb("basic_herb");
        inv.add_item(herb.clone(), 10);
        assert_eq!(inv.get_count("basic_herb"), 10);
        assert_eq!(inv.occupied_slots(), 1);

        // Add more of the same — should stack
        inv.add_item(herb, 5);
        assert_eq!(inv.get_count("basic_herb"), 15);
        assert_eq!(inv.occupied_slots(), 1);
    }

    #[test]
    fn test_add_non_stackable_items() {
        let mut inv = Inventory::new(3);
        let potion = make_potion("heal_potion");
        let overflow = inv.add_item(potion, 5);
        // Only 3 slots, non-stackable → 2 overflow
        assert_eq!(overflow, 2);
        assert_eq!(inv.get_count("heal_potion"), 3);
        assert_eq!(inv.occupied_slots(), 3);
        assert!(inv.is_full());
    }

    #[test]
    fn test_remove_items() {
        let mut inv = Inventory::new(5);
        let herb = make_herb("basic_herb");
        inv.add_item(herb, 20);

        let shortfall = inv.remove_item("basic_herb", 8);
        assert_eq!(shortfall, 0);
        assert_eq!(inv.get_count("basic_herb"), 12);
    }

    #[test]
    fn test_remove_more_than_available() {
        let mut inv = Inventory::new(5);
        let herb = make_herb("basic_herb");
        inv.add_item(herb, 5);

        let shortfall = inv.remove_item("basic_herb", 10);
        assert_eq!(shortfall, 5);
        assert_eq!(inv.get_count("basic_herb"), 0);
        assert!(inv.is_empty());
    }

    #[test]
    fn test_remove_clears_empty_slots() {
        let mut inv = Inventory::new(5);
        let herb = make_herb("basic_herb");
        inv.add_item(herb, 3);
        assert_eq!(inv.occupied_slots(), 1);

        inv.remove_item("basic_herb", 3);
        assert_eq!(inv.occupied_slots(), 0);
        assert!(inv.is_empty());
    }

    #[test]
    fn test_has_item() {
        let mut inv = Inventory::new(5);
        assert!(!inv.has_item("basic_herb", 1));

        let herb = make_herb("basic_herb");
        inv.add_item(herb, 5);
        assert!(inv.has_item("basic_herb", 5));
        assert!(inv.has_item("basic_herb", 3));
        assert!(!inv.has_item("basic_herb", 6));
    }

    #[test]
    fn test_slot_operations() {
        let mut inv = Inventory::new(5);
        let quest = make_quest_item("ancient_key");
        inv.add_item(quest, 1);

        assert!(inv.get_slot(0).is_some());
        assert_eq!(inv.get_slot(0).unwrap().item.name, "ancient_key");
        assert!(inv.get_slot(1).is_none());

        let taken = inv.take_slot(0);
        assert!(taken.is_some());
        assert!(inv.is_empty());
    }

    #[test]
    fn test_stack_overflow_to_new_slot() {
        let mut inv = Inventory::new(5);
        let mut herb = make_herb("basic_herb");
        herb.max_stack = 10;
        let overflow = inv.add_item(herb, 25);
        assert_eq!(overflow, 0);
        // 10 + 10 + 5 = 25 across 3 slots
        assert_eq!(inv.get_count("basic_herb"), 25);
        assert_eq!(inv.occupied_slots(), 3);
    }

    #[test]
    fn test_mixed_items() {
        let mut inv = Inventory::new(5);
        inv.add_item(make_herb("herb_a"), 5);
        inv.add_item(make_herb("herb_b"), 3);
        inv.add_item(make_quest_item("key"), 1);

        assert_eq!(inv.occupied_slots(), 3);
        assert_eq!(inv.get_count("herb_a"), 5);
        assert_eq!(inv.get_count("herb_b"), 3);
        assert!(inv.has_item("key", 1));
    }

    #[test]
    fn test_items_iterator() {
        let mut inv = Inventory::new(5);
        inv.add_item(make_herb("herb_a"), 3);
        inv.add_item(make_quest_item("key"), 1);

        let items: Vec<_> = inv.items().collect();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].0, 0); // slot index
        assert_eq!(items[1].0, 1);
    }

    #[test]
    fn test_inventory_full_rejects_items() {
        let mut inv = Inventory::new(2);
        inv.add_item(make_quest_item("key_1"), 1);
        inv.add_item(make_quest_item("key_2"), 1);
        assert!(inv.is_full());

        let overflow = inv.add_item(make_quest_item("key_3"), 1);
        assert_eq!(overflow, 1);
    }

    #[test]
    fn test_inventory_serde_roundtrip() {
        let mut inv = Inventory::new(5);
        inv.add_item(make_herb("basic_herb"), 10);
        inv.add_item(make_quest_item("ancient_key"), 1);

        let json = serde_json::to_string(&inv).unwrap();
        let deserialized: Inventory = serde_json::from_str(&json).unwrap();
        assert_eq!(inv, deserialized);
    }
}
