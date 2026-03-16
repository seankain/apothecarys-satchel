//! Crafting UI state management: recipe list, ingredient slots, brew button.

use apothecarys_inventory::crafting::{Recipe, RecipeBook};

/// State for the crafting UI screen.
pub struct CraftingUiState {
    pub visible: bool,
    pub selected_recipe: Option<usize>,
    pub placed_ingredients: Vec<Option<PlacedSlot>>,
    pub brew_ready: bool,
}

/// A placed ingredient in the crafting UI.
#[derive(Debug, Clone)]
pub struct PlacedSlot {
    pub inventory_slot: usize,
    pub template_id: String,
    pub item_name: String,
}

impl CraftingUiState {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected_recipe: None,
            placed_ingredients: Vec::new(),
            brew_ready: false,
        }
    }

    /// Toggle crafting screen visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.clear();
        }
    }

    /// Select a recipe from the recipe book.
    pub fn select_recipe(&mut self, index: usize, recipe_book: &RecipeBook) {
        if index < recipe_book.known_recipes.len() {
            self.selected_recipe = Some(index);
            let recipe = &recipe_book.known_recipes[index];
            self.placed_ingredients = vec![None; recipe.ingredients.len()];
            self.brew_ready = false;
        }
    }

    /// Place an ingredient from the inventory into a recipe slot.
    pub fn place_ingredient(&mut self, slot_index: usize, inventory_slot: usize, template_id: String, item_name: String) {
        if slot_index < self.placed_ingredients.len() {
            self.placed_ingredients[slot_index] = Some(PlacedSlot {
                inventory_slot,
                template_id,
                item_name,
            });
            self.update_brew_ready();
        }
    }

    /// Remove an ingredient from a recipe slot.
    pub fn remove_ingredient(&mut self, slot_index: usize) {
        if slot_index < self.placed_ingredients.len() {
            self.placed_ingredients[slot_index] = None;
            self.update_brew_ready();
        }
    }

    /// Check if the recipe is ready to brew (all required slots filled).
    fn update_brew_ready(&mut self) {
        // Without access to the recipe, we check if any slots are filled
        // The actual validation happens in resolve_recipe
        self.brew_ready = self.placed_ingredients.iter().any(|s| s.is_some());
    }

    /// Check readiness against a specific recipe's requirements.
    pub fn check_brew_ready(&self, recipe: &Recipe) -> bool {
        for (i, slot) in recipe.ingredients.iter().enumerate() {
            if slot.required && self.placed_ingredients.get(i).and_then(|s| s.as_ref()).is_none() {
                return false;
            }
        }
        true
    }

    /// Clear all state.
    pub fn clear(&mut self) {
        self.selected_recipe = None;
        self.placed_ingredients.clear();
        self.brew_ready = false;
    }

    /// Get display data for the recipe list.
    pub fn recipe_list_data(recipe_book: &RecipeBook) -> Vec<RecipeListItem> {
        recipe_book
            .known_recipes
            .iter()
            .enumerate()
            .map(|(i, r)| RecipeListItem {
                index: i,
                name: r.name.clone(),
                category: format!("{:?}", r.category),
                ingredient_count: r.ingredients.len(),
            })
            .collect()
    }
}

impl Default for CraftingUiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Display data for a recipe in the list.
#[derive(Debug, Clone)]
pub struct RecipeListItem {
    pub index: usize,
    pub name: String,
    pub category: String,
    pub ingredient_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_inventory::crafting::*;

    fn make_recipe_book() -> RecipeBook {
        let mut book = RecipeBook::new();
        book.discover(Recipe {
            id: "heal_basic".to_string(),
            name: "Basic Healing Potion".to_string(),
            category: RecipeCategory::HealingPotion,
            ingredients: vec![
                IngredientSlot {
                    slot_type: IngredientType::AnyPlant,
                    required: true,
                },
                IngredientSlot {
                    slot_type: IngredientType::SpecificItem("vial".to_string()),
                    required: true,
                },
            ],
            result_type: ResultType::Fixed {
                template_id: "potion_heal".to_string(),
                name: "Healing Potion".to_string(),
            },
        });
        book
    }

    #[test]
    fn test_crafting_ui_toggle() {
        let mut ui = CraftingUiState::new();
        assert!(!ui.visible);
        ui.toggle();
        assert!(ui.visible);
        ui.toggle();
        assert!(!ui.visible);
    }

    #[test]
    fn test_select_recipe() {
        let mut ui = CraftingUiState::new();
        let book = make_recipe_book();

        ui.select_recipe(0, &book);
        assert_eq!(ui.selected_recipe, Some(0));
        assert_eq!(ui.placed_ingredients.len(), 2);
    }

    #[test]
    fn test_place_and_remove_ingredient() {
        let mut ui = CraftingUiState::new();
        let book = make_recipe_book();

        ui.select_recipe(0, &book);
        ui.place_ingredient(0, 0, "herb".to_string(), "Herb".to_string());
        assert!(ui.placed_ingredients[0].is_some());
        assert!(ui.brew_ready);

        ui.remove_ingredient(0);
        assert!(ui.placed_ingredients[0].is_none());
    }

    #[test]
    fn test_check_brew_ready() {
        let mut ui = CraftingUiState::new();
        let book = make_recipe_book();
        let recipe = &book.known_recipes[0];

        ui.select_recipe(0, &book);
        assert!(!ui.check_brew_ready(recipe));

        ui.place_ingredient(0, 0, "herb".to_string(), "Herb".to_string());
        assert!(!ui.check_brew_ready(recipe)); // Still missing required slot 1

        ui.place_ingredient(1, 1, "vial".to_string(), "Vial".to_string());
        assert!(ui.check_brew_ready(recipe));
    }

    #[test]
    fn test_recipe_list_data() {
        let book = make_recipe_book();
        let list = CraftingUiState::recipe_list_data(&book);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Basic Healing Potion");
        assert_eq!(list[0].ingredient_count, 2);
    }

    #[test]
    fn test_clear() {
        let mut ui = CraftingUiState::new();
        let book = make_recipe_book();

        ui.select_recipe(0, &book);
        ui.place_ingredient(0, 0, "herb".to_string(), "Herb".to_string());
        ui.clear();

        assert!(ui.selected_recipe.is_none());
        assert!(ui.placed_ingredients.is_empty());
        assert!(!ui.brew_ready);
    }
}
