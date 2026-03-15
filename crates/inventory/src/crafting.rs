use apothecarys_core::items::{AlchemyEffect, Item, ItemType, StatusEffectType};
use apothecarys_core::stats::{AttributeType, DamageType, StatusEffect};
use serde::{Deserialize, Serialize};

use crate::container::Inventory;

/// Categories of craftable items.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RecipeCategory {
    HealingPotion,
    BuffPotion,
    Poison,
    Medicine,
    Fertilizer,
}

/// What kind of ingredient a slot accepts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IngredientType {
    /// Any plant sample or ingredient.
    AnyPlant,
    /// A plant with minimum trait thresholds (for future genetics integration).
    PlantWithTrait {
        min_healing: Option<f32>,
        min_potency: Option<f32>,
    },
    /// A specific item by template_id.
    SpecificItem(String),
    /// A catalyst reagent that modifies the recipe output.
    Catalyst,
}

/// A slot in a recipe that must be filled with a matching ingredient.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IngredientSlot {
    pub slot_type: IngredientType,
    pub required: bool,
}

/// How the recipe determines its output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResultType {
    /// Fixed output item (template_id + name).
    Fixed {
        template_id: String,
        name: String,
    },
    /// Output depends on ingredient properties (future genetics integration).
    Dynamic,
}

/// A crafting recipe definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub category: RecipeCategory,
    pub ingredients: Vec<IngredientSlot>,
    pub result_type: ResultType,
}

/// Tracks which recipes the player has discovered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipeBook {
    pub known_recipes: Vec<Recipe>,
}

impl RecipeBook {
    pub fn new() -> Self {
        Self {
            known_recipes: Vec::new(),
        }
    }

    /// Add a recipe to the known list. Returns false if already known.
    pub fn discover(&mut self, recipe: Recipe) -> bool {
        if self.known_recipes.iter().any(|r| r.id == recipe.id) {
            return false;
        }
        self.known_recipes.push(recipe);
        true
    }

    /// Check if a recipe is known.
    pub fn is_known(&self, recipe_id: &str) -> bool {
        self.known_recipes.iter().any(|r| r.id == recipe_id)
    }

    /// Get a recipe by ID.
    pub fn get_recipe(&self, recipe_id: &str) -> Option<&Recipe> {
        self.known_recipes.iter().find(|r| r.id == recipe_id)
    }

    /// Load recipes from a RON string.
    pub fn load_from_ron(ron_str: &str) -> Result<Vec<Recipe>, ron::error::SpannedError> {
        ron::from_str(ron_str)
    }
}

impl Default for RecipeBook {
    fn default() -> Self {
        Self::new()
    }
}

/// An ingredient placed in a recipe slot by the player.
#[derive(Debug, Clone)]
pub struct PlacedIngredient {
    pub slot_index: usize,
    pub template_id: String,
    pub inventory_slot: usize,
}

/// Check if an ingredient matches a slot type.
pub fn ingredient_matches_slot(item: &Item, slot: &IngredientSlot) -> bool {
    match &slot.slot_type {
        IngredientType::AnyPlant => matches!(
            item.item_type,
            ItemType::PlantSample | ItemType::Ingredient
        ),
        IngredientType::PlantWithTrait { .. } => {
            // For now, any plant sample qualifies. Full genetics integration
            // will check actual gene values when botany crate is ready.
            matches!(item.item_type, ItemType::PlantSample | ItemType::Ingredient)
        }
        IngredientType::SpecificItem(id) => item.template_id == *id,
        IngredientType::Catalyst => item.template_id.contains("catalyst"),
    }
}

/// Result of a crafting attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum CraftResult {
    Success { item: Item },
    MissingIngredients { missing: Vec<usize> },
}

/// Resolve a recipe with the given ingredients. Consumes ingredients from inventory
/// on success, producing the output item.
pub fn resolve_recipe(
    recipe: &Recipe,
    placed: &[PlacedIngredient],
    inventory: &mut Inventory,
) -> CraftResult {
    // Check all required slots are filled
    let mut missing = Vec::new();
    for (i, slot) in recipe.ingredients.iter().enumerate() {
        if slot.required && !placed.iter().any(|p| p.slot_index == i) {
            missing.push(i);
        }
    }
    if !missing.is_empty() {
        return CraftResult::MissingIngredients { missing };
    }

    // Validate that placed ingredients match their slots
    for placed_ing in placed {
        if placed_ing.slot_index >= recipe.ingredients.len() {
            return CraftResult::MissingIngredients {
                missing: vec![placed_ing.slot_index],
            };
        }
        let slot = &recipe.ingredients[placed_ing.slot_index];
        let item = inventory.get_slot(placed_ing.inventory_slot);
        match item {
            Some(stack) if ingredient_matches_slot(&stack.item, slot) => {}
            _ => {
                return CraftResult::MissingIngredients {
                    missing: vec![placed_ing.slot_index],
                };
            }
        }
    }

    // Consume ingredients
    for placed_ing in placed {
        inventory.remove_item(&placed_ing.template_id, 1);
    }

    // Generate result item
    let result_item = generate_result_item(recipe);
    CraftResult::Success { item: result_item }
}

/// Generate the output item for a recipe based on its category and result type.
fn generate_result_item(recipe: &Recipe) -> Item {
    match &recipe.result_type {
        ResultType::Fixed { template_id, name } => {
            let item_type = match recipe.category {
                RecipeCategory::HealingPotion => ItemType::Potion {
                    effects: vec![AlchemyEffect::Heal { amount: 15 }],
                },
                RecipeCategory::BuffPotion => ItemType::Potion {
                    effects: vec![AlchemyEffect::Buff {
                        effect: StatusEffect::AttackUp { amount: 2 },
                        turns: 3,
                    }],
                },
                RecipeCategory::Poison => ItemType::Potion {
                    effects: vec![AlchemyEffect::Damage {
                        amount: 10,
                        damage_type: DamageType::Poison,
                    }],
                },
                RecipeCategory::Medicine => ItemType::Medicine {
                    cures: vec![StatusEffectType::Poisoned],
                },
                RecipeCategory::Fertilizer => ItemType::Ingredient,
            };
            Item::new(template_id.clone(), name.clone(), item_type)
        }
        ResultType::Dynamic => {
            // Dynamic recipes derive effects from plant genetics.
            // This is a stub that will be fully implemented when the botany crate is ready.
            // For now, produce a basic healing potion.
            let item_type = match recipe.category {
                RecipeCategory::HealingPotion => ItemType::Potion {
                    effects: vec![AlchemyEffect::Heal { amount: 10 }],
                },
                RecipeCategory::BuffPotion => ItemType::Potion {
                    effects: vec![AlchemyEffect::StatBoost {
                        attribute: AttributeType::Strength,
                        amount: 2,
                        turns: 3,
                    }],
                },
                RecipeCategory::Poison => ItemType::Potion {
                    effects: vec![AlchemyEffect::Damage {
                        amount: 8,
                        damage_type: DamageType::Poison,
                    }],
                },
                RecipeCategory::Medicine => ItemType::Medicine {
                    cures: vec![StatusEffectType::Poisoned],
                },
                RecipeCategory::Fertilizer => ItemType::Ingredient,
            };
            let name = format!("Crafted {}", recipe.name);
            Item::new(recipe.id.clone(), name, item_type)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_core::items::Item;

    fn make_herb(tid: &str) -> Item {
        Item::new(tid, tid, ItemType::Ingredient)
    }

    fn make_plant_sample(tid: &str) -> Item {
        Item::new(tid, tid, ItemType::PlantSample)
    }

    fn make_catalyst() -> Item {
        Item::new("fire_catalyst", "Fire Catalyst", ItemType::Ingredient)
    }

    fn basic_healing_recipe() -> Recipe {
        Recipe {
            id: "heal_basic".to_string(),
            name: "Basic Healing Potion".to_string(),
            category: RecipeCategory::HealingPotion,
            ingredients: vec![
                IngredientSlot {
                    slot_type: IngredientType::AnyPlant,
                    required: true,
                },
                IngredientSlot {
                    slot_type: IngredientType::SpecificItem("empty_vial".to_string()),
                    required: true,
                },
            ],
            result_type: ResultType::Fixed {
                template_id: "potion_heal_1".to_string(),
                name: "Minor Healing Potion".to_string(),
            },
        }
    }

    #[test]
    fn test_recipe_book_discover() {
        let mut book = RecipeBook::new();
        let recipe = basic_healing_recipe();

        assert!(book.discover(recipe.clone()));
        assert!(!book.discover(recipe)); // duplicate
        assert!(book.is_known("heal_basic"));
        assert!(!book.is_known("unknown"));
    }

    #[test]
    fn test_recipe_book_get() {
        let mut book = RecipeBook::new();
        let recipe = basic_healing_recipe();
        book.discover(recipe);

        let found = book.get_recipe("heal_basic");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Basic Healing Potion");
    }

    #[test]
    fn test_ingredient_matches_any_plant() {
        let slot = IngredientSlot {
            slot_type: IngredientType::AnyPlant,
            required: true,
        };

        assert!(ingredient_matches_slot(&make_herb("herb"), &slot));
        assert!(ingredient_matches_slot(&make_plant_sample("sample"), &slot));

        let potion = Item::new(
            "potion",
            "Potion",
            ItemType::Potion { effects: vec![] },
        );
        assert!(!ingredient_matches_slot(&potion, &slot));
    }

    #[test]
    fn test_ingredient_matches_specific() {
        let slot = IngredientSlot {
            slot_type: IngredientType::SpecificItem("empty_vial".to_string()),
            required: true,
        };

        let vial = Item::new("empty_vial", "Empty Vial", ItemType::Ingredient);
        assert!(ingredient_matches_slot(&vial, &slot));

        let herb = make_herb("herb");
        assert!(!ingredient_matches_slot(&herb, &slot));
    }

    #[test]
    fn test_ingredient_matches_catalyst() {
        let slot = IngredientSlot {
            slot_type: IngredientType::Catalyst,
            required: false,
        };

        assert!(ingredient_matches_slot(&make_catalyst(), &slot));
        assert!(!ingredient_matches_slot(&make_herb("herb"), &slot));
    }

    #[test]
    fn test_resolve_recipe_success() {
        let recipe = basic_healing_recipe();
        let mut inventory = Inventory::new(10);

        let herb = make_herb("basic_herb");
        let vial = Item::new("empty_vial", "Empty Vial", ItemType::Ingredient);
        inventory.add_item(herb, 1);
        inventory.add_item(vial, 1);

        let placed = vec![
            PlacedIngredient {
                slot_index: 0,
                template_id: "basic_herb".to_string(),
                inventory_slot: 0,
            },
            PlacedIngredient {
                slot_index: 1,
                template_id: "empty_vial".to_string(),
                inventory_slot: 1,
            },
        ];

        let result = resolve_recipe(&recipe, &placed, &mut inventory);
        match result {
            CraftResult::Success { item } => {
                assert_eq!(item.template_id, "potion_heal_1");
                assert_eq!(item.name, "Minor Healing Potion");
                assert!(matches!(item.item_type, ItemType::Potion { .. }));
            }
            other => panic!("Expected Success, got {other:?}"),
        }

        // Ingredients should be consumed
        assert_eq!(inventory.get_count("basic_herb"), 0);
        assert_eq!(inventory.get_count("empty_vial"), 0);
    }

    #[test]
    fn test_resolve_recipe_missing_required() {
        let recipe = basic_healing_recipe();
        let mut inventory = Inventory::new(10);

        let herb = make_herb("basic_herb");
        inventory.add_item(herb, 1);

        // Only place herb, missing vial
        let placed = vec![PlacedIngredient {
            slot_index: 0,
            template_id: "basic_herb".to_string(),
            inventory_slot: 0,
        }];

        let result = resolve_recipe(&recipe, &placed, &mut inventory);
        match result {
            CraftResult::MissingIngredients { missing } => {
                assert_eq!(missing, vec![1]);
            }
            other => panic!("Expected MissingIngredients, got {other:?}"),
        }

        // Ingredients should NOT be consumed on failure
        assert_eq!(inventory.get_count("basic_herb"), 1);
    }

    #[test]
    fn test_resolve_recipe_wrong_ingredient() {
        let recipe = basic_healing_recipe();
        let mut inventory = Inventory::new(10);

        let herb = make_herb("basic_herb");
        let wrong = Item::new("wrong", "Wrong Item", ItemType::QuestItem);
        inventory.add_item(herb, 1);
        inventory.add_item(wrong, 1);

        let placed = vec![
            PlacedIngredient {
                slot_index: 0,
                template_id: "basic_herb".to_string(),
                inventory_slot: 0,
            },
            PlacedIngredient {
                slot_index: 1,
                template_id: "wrong".to_string(),
                inventory_slot: 1,
            },
        ];

        let result = resolve_recipe(&recipe, &placed, &mut inventory);
        assert!(matches!(result, CraftResult::MissingIngredients { .. }));
    }

    #[test]
    fn test_resolve_dynamic_recipe() {
        let recipe = Recipe {
            id: "dynamic_heal".to_string(),
            name: "Herbal Brew".to_string(),
            category: RecipeCategory::HealingPotion,
            ingredients: vec![IngredientSlot {
                slot_type: IngredientType::AnyPlant,
                required: true,
            }],
            result_type: ResultType::Dynamic,
        };

        let mut inventory = Inventory::new(10);
        inventory.add_item(make_herb("magic_herb"), 1);

        let placed = vec![PlacedIngredient {
            slot_index: 0,
            template_id: "magic_herb".to_string(),
            inventory_slot: 0,
        }];

        let result = resolve_recipe(&recipe, &placed, &mut inventory);
        match result {
            CraftResult::Success { item } => {
                assert!(matches!(item.item_type, ItemType::Potion { .. }));
                assert!(item.name.contains("Crafted"));
            }
            other => panic!("Expected Success, got {other:?}"),
        }
    }

    #[test]
    fn test_optional_slot_not_required() {
        let recipe = Recipe {
            id: "optional_recipe".to_string(),
            name: "Enhanced Brew".to_string(),
            category: RecipeCategory::BuffPotion,
            ingredients: vec![
                IngredientSlot {
                    slot_type: IngredientType::AnyPlant,
                    required: true,
                },
                IngredientSlot {
                    slot_type: IngredientType::Catalyst,
                    required: false, // optional
                },
            ],
            result_type: ResultType::Fixed {
                template_id: "potion_buff".to_string(),
                name: "Buff Potion".to_string(),
            },
        };

        let mut inventory = Inventory::new(10);
        inventory.add_item(make_herb("herb"), 1);

        // Only place the required ingredient
        let placed = vec![PlacedIngredient {
            slot_index: 0,
            template_id: "herb".to_string(),
            inventory_slot: 0,
        }];

        let result = resolve_recipe(&recipe, &placed, &mut inventory);
        assert!(matches!(result, CraftResult::Success { .. }));
    }

    #[test]
    fn test_recipe_ron_loading() {
        let ron_str = r#"[
            (
                id: "heal_basic",
                name: "Basic Healing Potion",
                category: HealingPotion,
                ingredients: [
                    (slot_type: AnyPlant, required: true),
                    (slot_type: SpecificItem("empty_vial"), required: true),
                ],
                result_type: Fixed(
                    template_id: "potion_heal_1",
                    name: "Minor Healing Potion",
                ),
            ),
            (
                id: "poison_basic",
                name: "Basic Poison",
                category: Poison,
                ingredients: [
                    (slot_type: AnyPlant, required: true),
                ],
                result_type: Dynamic,
            ),
        ]"#;

        let recipes = RecipeBook::load_from_ron(ron_str).unwrap();
        assert_eq!(recipes.len(), 2);
        assert_eq!(recipes[0].id, "heal_basic");
        assert_eq!(recipes[1].id, "poison_basic");
        assert_eq!(recipes[0].ingredients.len(), 2);
    }

    #[test]
    fn test_each_category_produces_correct_type() {
        let categories = vec![
            (RecipeCategory::HealingPotion, "heal"),
            (RecipeCategory::BuffPotion, "buff"),
            (RecipeCategory::Poison, "poison"),
            (RecipeCategory::Medicine, "medicine"),
            (RecipeCategory::Fertilizer, "fertilizer"),
        ];

        for (category, id) in categories {
            let recipe = Recipe {
                id: id.to_string(),
                name: id.to_string(),
                category,
                ingredients: vec![IngredientSlot {
                    slot_type: IngredientType::AnyPlant,
                    required: true,
                }],
                result_type: ResultType::Fixed {
                    template_id: format!("result_{id}"),
                    name: format!("Result {id}"),
                },
            };

            let mut inventory = Inventory::new(10);
            inventory.add_item(make_herb("herb"), 1);
            let placed = vec![PlacedIngredient {
                slot_index: 0,
                template_id: "herb".to_string(),
                inventory_slot: 0,
            }];

            let result = resolve_recipe(&recipe, &placed, &mut inventory);
            assert!(
                matches!(result, CraftResult::Success { .. }),
                "Failed for category {id}"
            );
        }
    }

    #[test]
    fn test_recipe_book_serde_roundtrip() {
        let mut book = RecipeBook::new();
        book.discover(basic_healing_recipe());

        let json = serde_json::to_string(&book).unwrap();
        let deserialized: RecipeBook = serde_json::from_str(&json).unwrap();
        assert_eq!(book, deserialized);
    }
}
