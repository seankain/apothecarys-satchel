use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::stats::{AttributeType, DamageType, StatusEffect};

/// An alchemy effect produced by plant-based crafting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlchemyEffect {
    Heal { amount: i32 },
    Damage { amount: i32, damage_type: DamageType },
    Buff { effect: StatusEffect, turns: u32 },
    Cure { cures: StatusEffectType },
    StatBoost { attribute: AttributeType, amount: i32, turns: u32 },
}

/// Which status effect type a medicine can cure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StatusEffectType {
    Poisoned,
    Weakened,
    Slowed,
    Stunned,
    Blinded,
}

/// Equipment slot and associated data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EquipmentData {
    pub slot: EquipmentSlot,
    pub armor_bonus: i32,
    pub attack_bonus: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EquipmentSlot {
    Weapon,
    Armor,
    Accessory,
}

/// The category/type of an item, determining its behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ItemType {
    /// A sample harvested from a plant, carrying its genotype data
    PlantSample,
    /// A crafted potion with alchemy effects
    Potion { effects: Vec<AlchemyEffect> },
    /// A medicine that cures specific status effects
    Medicine { cures: Vec<StatusEffectType> },
    /// A raw ingredient for crafting
    Ingredient,
    /// Equippable gear
    Equipment(EquipmentData),
    /// A quest-related item (non-consumable, non-stackable)
    QuestItem,
    /// A seed carrying genotype data for planting
    Seed,
    /// Currency
    Gold(u32),
}

/// A single item definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    pub id: Uuid,
    pub template_id: String,
    pub name: String,
    pub item_type: ItemType,
    pub icon_path: String,
    pub description: String,
    /// Whether multiple of this item can share a stack
    pub stackable: bool,
    /// Maximum stack size (only relevant if stackable)
    pub max_stack: u32,
}

impl Item {
    pub fn new(
        template_id: impl Into<String>,
        name: impl Into<String>,
        item_type: ItemType,
    ) -> Self {
        let stackable = matches!(
            item_type,
            ItemType::Ingredient | ItemType::Gold(_)
        );
        Self {
            id: Uuid::new_v4(),
            template_id: template_id.into(),
            name: name.into(),
            item_type,
            icon_path: String::new(),
            description: String::new(),
            stackable,
            max_stack: if stackable { 99 } else { 1 },
        }
    }
}

/// A stack of identical items occupying one inventory slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemStack {
    pub item: Item,
    pub count: u32,
}

impl ItemStack {
    pub fn new(item: Item, count: u32) -> Self {
        Self { item, count }
    }

    pub fn single(item: Item) -> Self {
        Self { item, count: 1 }
    }

    /// Try to add more items to this stack. Returns the overflow count.
    pub fn try_add(&mut self, amount: u32) -> u32 {
        if !self.item.stackable {
            return amount;
        }
        let space = self.item.max_stack.saturating_sub(self.count);
        let to_add = amount.min(space);
        self.count += to_add;
        amount - to_add
    }

    /// Try to remove items from this stack. Returns how many couldn't be removed.
    pub fn try_remove(&mut self, amount: u32) -> u32 {
        let to_remove = amount.min(self.count);
        self.count -= to_remove;
        amount - to_remove
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_creation() {
        let item = Item::new("potion_heal_1", "Minor Healing Potion", ItemType::Potion {
            effects: vec![AlchemyEffect::Heal { amount: 10 }],
        });
        assert_eq!(item.name, "Minor Healing Potion");
        assert!(!item.stackable);
    }

    #[test]
    fn test_item_stack_operations() {
        let item = Item::new("herb_basic", "Basic Herb", ItemType::Ingredient);
        assert!(item.stackable);

        let mut stack = ItemStack::new(item, 5);
        assert_eq!(stack.count, 5);

        let overflow = stack.try_add(10);
        assert_eq!(stack.count, 15);
        assert_eq!(overflow, 0);

        let shortfall = stack.try_remove(20);
        assert_eq!(stack.count, 0);
        assert_eq!(shortfall, 5);
    }

    #[test]
    fn test_non_stackable_item() {
        let item = Item::new("quest_key", "Ancient Key", ItemType::QuestItem);
        assert!(!item.stackable);

        let mut stack = ItemStack::single(item);
        let overflow = stack.try_add(1);
        assert_eq!(overflow, 1);
        assert_eq!(stack.count, 1);
    }

    #[test]
    fn test_item_serde_roundtrip() {
        let item = Item::new("potion_fire", "Fire Bomb", ItemType::Potion {
            effects: vec![AlchemyEffect::Damage {
                amount: 15,
                damage_type: DamageType::Fire,
            }],
        });
        let stack = ItemStack::new(item, 3);
        let json = serde_json::to_string(&stack).unwrap();
        let deserialized: ItemStack = serde_json::from_str(&json).unwrap();
        assert_eq!(stack.count, deserialized.count);
        assert_eq!(stack.item.name, deserialized.item.name);
    }

    #[test]
    fn test_equipment_item() {
        let item = Item::new("sword_iron", "Iron Sword", ItemType::Equipment(EquipmentData {
            slot: EquipmentSlot::Weapon,
            armor_bonus: 0,
            attack_bonus: 2,
        }));
        assert!(!item.stackable);
        assert_eq!(item.max_stack, 1);
    }
}
