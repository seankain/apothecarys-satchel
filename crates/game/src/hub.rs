//! Hub location integration: connects recruitment, garden, dungeon, and crafting.
//!
//! The hub is the central location where the player can:
//! - Recruit party members from the recruitment pool
//! - Access the garden for planting/breeding/harvesting
//! - Enter dungeon expeditions
//! - Use the crafting station to brew potions and medicines

use apothecarys_core::items::Item;
use apothecarys_inventory::container::Inventory;
use apothecarys_inventory::crafting::{
    CraftResult, PlacedIngredient, Recipe, RecipeBook, resolve_recipe,
};
use apothecarys_party::generation::PartyMember;
use apothecarys_party::recruitment::RecruitmentPool;
use apothecarys_party::roster::Roster;
use rand::Rng;

/// Interaction points available in the hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubInteraction {
    RecruitmentBoard,
    GardenEntrance,
    DungeonEntrance,
    CraftingStation,
}

/// Result of a recruitment action.
#[derive(Debug)]
pub enum RecruitResult {
    /// Successfully recruited.
    Recruited { member: Box<PartyMember> },
    /// Roster is full.
    RosterFull,
    /// Invalid candidate index.
    InvalidCandidate,
}

/// Result of attempting to enter the dungeon.
#[derive(Debug)]
pub enum DungeonEntryResult {
    /// Party is ready and can enter.
    Allowed { floor: u32, difficulty: u32 },
    /// Need at least one party member.
    PartyEmpty,
}

/// The hub state manager coordinating all hub-level interactions.
pub struct HubState {
    pub roster: Roster,
    pub recruitment_pool: RecruitmentPool,
    pub inventory: Inventory,
    pub recipe_book: RecipeBook,
    pub dead_members: Vec<String>,
    pub dungeon_floor: u32,
    pub dungeon_difficulty: u32,
}

impl HubState {
    /// Create a new hub state with initial values.
    pub fn new(rng: &mut impl Rng) -> Self {
        Self {
            roster: Roster::new(),
            recruitment_pool: RecruitmentPool::generate(rng, 1),
            inventory: Inventory::new(20),
            recipe_book: RecipeBook::new(),
            dead_members: Vec::new(),
            dungeon_floor: 1,
            dungeon_difficulty: 1,
        }
    }

    /// Recruit a candidate from the recruitment pool by index.
    pub fn recruit(&mut self, candidate_index: usize) -> RecruitResult {
        if self.roster.is_full() {
            return RecruitResult::RosterFull;
        }

        match self.recruitment_pool.recruit(candidate_index) {
            Some(member) => {
                let result_member = member.clone();
                let _ = self.roster.add_member(member);
                RecruitResult::Recruited {
                    member: Box::new(result_member),
                }
            }
            None => RecruitResult::InvalidCandidate,
        }
    }

    /// Dismiss a party member by UUID, returning them.
    pub fn dismiss_member(&mut self, id: uuid::Uuid) -> Option<PartyMember> {
        self.roster.dismiss(id)
    }

    /// Refresh the recruitment pool (after returning from a dungeon).
    pub fn refresh_recruitment(&mut self, rng: &mut impl Rng) {
        let avg_level = if self.roster.is_empty() {
            1
        } else {
            let total: u32 = self.roster.members.iter().map(|m| m.level).sum();
            total / self.roster.size() as u32
        };
        self.recruitment_pool.refresh(rng, avg_level);
    }

    /// Check if the party can enter the dungeon.
    pub fn try_enter_dungeon(&self) -> DungeonEntryResult {
        if self.roster.is_empty() {
            DungeonEntryResult::PartyEmpty
        } else {
            DungeonEntryResult::Allowed {
                floor: self.dungeon_floor,
                difficulty: self.dungeon_difficulty,
            }
        }
    }

    /// Called when returning from a dungeon run.
    pub fn return_from_dungeon(&mut self, rng: &mut impl Rng) {
        // Process permadeath for dead members
        let dead = self.roster.remove_dead();
        for member in &dead {
            self.dead_members.push(member.name.clone());
            // Transfer equipment to inventory
            let mut equipment = member.equipment.clone();
            for item in equipment.unequip_all() {
                self.inventory.add_item(item, 1);
            }
        }

        // Refresh recruitment pool
        self.refresh_recruitment(rng);

        // Advance difficulty if floor was cleared
        self.dungeon_floor += 1;
    }

    /// Attempt to craft a recipe with placed ingredients.
    pub fn craft(
        &mut self,
        recipe: &Recipe,
        placed: &[PlacedIngredient],
    ) -> CraftResult {
        resolve_recipe(recipe, placed, &mut self.inventory)
    }

    /// Add a crafted item to inventory. Returns overflow count.
    pub fn add_to_inventory(&mut self, item: Item, count: u32) -> u32 {
        self.inventory.add_item(item, count)
    }

    /// Get the list of available hub interactions.
    pub fn available_interactions(&self) -> Vec<HubInteraction> {
        vec![
            HubInteraction::RecruitmentBoard,
            HubInteraction::GardenEntrance,
            HubInteraction::DungeonEntrance,
            HubInteraction::CraftingStation,
        ]
    }
}

/// Hub location definition in the world graph for scene transitions.
pub fn hub_location_def() -> apothecarys_world::location::LocationDef {
    use apothecarys_world::location::*;

    LocationDef {
        id: LocationId::from("hub"),
        display_name: "Willowmere".to_string(),
        scene_path: "assets/scenes/hub.gltf".to_string(),
        location_type: LocationType::Hub,
        spawn_points: vec![
            SpawnPoint {
                name: "from_garden".to_string(),
                position: [5.0, 0.0, 3.0],
                rotation: 0.0,
                spawn_type: SpawnType::PlayerArrival,
            },
            SpawnPoint {
                name: "from_dungeon".to_string(),
                position: [-5.0, 0.0, 0.0],
                rotation: 180.0,
                spawn_type: SpawnType::PlayerArrival,
            },
            SpawnPoint {
                name: "default".to_string(),
                position: [0.0, 0.0, 0.0],
                rotation: 0.0,
                spawn_type: SpawnType::PlayerArrival,
            },
            SpawnPoint {
                name: "recruiter".to_string(),
                position: [3.0, 0.0, -2.0],
                rotation: 0.0,
                spawn_type: SpawnType::Npc {
                    npc_id: "recruiter".to_string(),
                },
            },
            SpawnPoint {
                name: "crafting_station".to_string(),
                position: [-3.0, 0.0, 2.0],
                rotation: 0.0,
                spawn_type: SpawnType::Npc {
                    npc_id: "crafting_station".to_string(),
                },
            },
        ],
        exits: vec![
            ExitDef {
                target_location: LocationId::from("garden"),
                exit_node_name: "exit_to_garden".to_string(),
                arrival_spawn: "from_hub".to_string(),
            },
            ExitDef {
                target_location: LocationId::from("dungeon_f1"),
                exit_node_name: "exit_to_dungeon".to_string(),
                arrival_spawn: "entrance".to_string(),
            },
        ],
    }
}

/// Garden location definition.
pub fn garden_location_def() -> apothecarys_world::location::LocationDef {
    use apothecarys_world::location::*;

    LocationDef {
        id: LocationId::from("garden"),
        display_name: "Apothecary's Garden".to_string(),
        scene_path: "assets/scenes/garden.gltf".to_string(),
        location_type: LocationType::Garden,
        spawn_points: vec![SpawnPoint {
            name: "from_hub".to_string(),
            position: [0.0, 0.0, -5.0],
            rotation: 0.0,
            spawn_type: SpawnType::PlayerArrival,
        }],
        exits: vec![ExitDef {
            target_location: LocationId::from("hub"),
            exit_node_name: "exit_to_hub".to_string(),
            arrival_spawn: "from_garden".to_string(),
        }],
    }
}

/// Dungeon floor 1 location definition.
pub fn dungeon_f1_location_def() -> apothecarys_world::location::LocationDef {
    use apothecarys_world::location::*;

    LocationDef {
        id: LocationId::from("dungeon_f1"),
        display_name: "Dungeon - Floor 1".to_string(),
        scene_path: "assets/scenes/dungeon_f1.gltf".to_string(),
        location_type: LocationType::Dungeon {
            floor: 1,
            difficulty: 1,
        },
        spawn_points: vec![SpawnPoint {
            name: "entrance".to_string(),
            position: [0.0, 0.0, 0.0],
            rotation: 0.0,
            spawn_type: SpawnType::PlayerArrival,
        }],
        exits: vec![ExitDef {
            target_location: LocationId::from("hub"),
            exit_node_name: "exit_to_hub".to_string(),
            arrival_spawn: "from_dungeon".to_string(),
        }],
    }
}

/// Build the default world graph with hub, garden, and dungeon.
pub fn build_default_world() -> Result<apothecarys_world::map_graph::WorldGraph, apothecarys_world::map_graph::WorldGraphError> {
    apothecarys_world::map_graph::WorldGraph::from_locations(vec![
        hub_location_def(),
        garden_location_def(),
        dungeon_f1_location_def(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_core::items::ItemType;
    use apothecarys_party::generation::generate_party_member;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn test_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn test_hub_state_creation() {
        let mut rng = test_rng();
        let hub = HubState::new(&mut rng);
        assert!(hub.roster.is_empty());
        assert!(!hub.recruitment_pool.is_empty());
        assert!(hub.inventory.is_empty());
        assert_eq!(hub.dungeon_floor, 1);
    }

    #[test]
    fn test_recruit_party_member() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);
        let initial_candidates = hub.recruitment_pool.candidate_count();

        match hub.recruit(0) {
            RecruitResult::Recruited { member } => {
                assert_eq!(hub.roster.size(), 1);
                assert_eq!(hub.recruitment_pool.candidate_count(), initial_candidates - 1);
                assert!(!member.name.is_empty());
            }
            other => panic!("Expected Recruited, got {other:?}"),
        }
    }

    #[test]
    fn test_recruit_invalid_candidate() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);
        match hub.recruit(100) {
            RecruitResult::InvalidCandidate => {}
            other => panic!("Expected InvalidCandidate, got {other:?}"),
        }
    }

    #[test]
    fn test_recruit_roster_full() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);

        // Fill the roster
        for _ in 0..4 {
            let member = generate_party_member(&mut rng, 1);
            hub.roster.add_member(member).unwrap();
        }

        match hub.recruit(0) {
            RecruitResult::RosterFull => {}
            other => panic!("Expected RosterFull, got {other:?}"),
        }
    }

    #[test]
    fn test_dismiss_member() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);
        hub.recruit(0);
        let id = hub.roster.members[0].id;

        let dismissed = hub.dismiss_member(id);
        assert!(dismissed.is_some());
        assert!(hub.roster.is_empty());
    }

    #[test]
    fn test_dungeon_entry_requires_party() {
        let mut rng = test_rng();
        let hub = HubState::new(&mut rng);
        match hub.try_enter_dungeon() {
            DungeonEntryResult::PartyEmpty => {}
            other => panic!("Expected PartyEmpty, got {other:?}"),
        }
    }

    #[test]
    fn test_dungeon_entry_with_party() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);
        hub.recruit(0);

        match hub.try_enter_dungeon() {
            DungeonEntryResult::Allowed { floor, difficulty } => {
                assert_eq!(floor, 1);
                assert_eq!(difficulty, 1);
            }
            other => panic!("Expected Allowed, got {other:?}"),
        }
    }

    #[test]
    fn test_return_from_dungeon_handles_dead() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);

        // Add a member and kill them
        let mut member = generate_party_member(&mut rng, 1);
        let name = member.name.clone();
        member.alive = false;
        hub.roster.add_member(member).unwrap();

        hub.return_from_dungeon(&mut rng);
        assert!(hub.roster.is_empty());
        assert!(hub.dead_members.contains(&name));
    }

    #[test]
    fn test_return_from_dungeon_advances_floor() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);
        assert_eq!(hub.dungeon_floor, 1);

        hub.return_from_dungeon(&mut rng);
        assert_eq!(hub.dungeon_floor, 2);
    }

    #[test]
    fn test_available_interactions() {
        let mut rng = test_rng();
        let hub = HubState::new(&mut rng);
        let interactions = hub.available_interactions();
        assert_eq!(interactions.len(), 4);
        assert!(interactions.contains(&HubInteraction::RecruitmentBoard));
        assert!(interactions.contains(&HubInteraction::GardenEntrance));
        assert!(interactions.contains(&HubInteraction::DungeonEntrance));
        assert!(interactions.contains(&HubInteraction::CraftingStation));
    }

    #[test]
    fn test_hub_crafting() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);

        let herb = Item::new("herb", "Herb", ItemType::Ingredient);
        let vial = Item::new("empty_vial", "Vial", ItemType::Ingredient);
        hub.inventory.add_item(herb, 1);
        hub.inventory.add_item(vial, 1);

        let recipe = Recipe {
            id: "heal_basic".to_string(),
            name: "Healing Potion".to_string(),
            category: apothecarys_inventory::crafting::RecipeCategory::HealingPotion,
            ingredients: vec![
                apothecarys_inventory::crafting::IngredientSlot {
                    slot_type: apothecarys_inventory::crafting::IngredientType::AnyPlant,
                    required: true,
                },
                apothecarys_inventory::crafting::IngredientSlot {
                    slot_type: apothecarys_inventory::crafting::IngredientType::SpecificItem(
                        "empty_vial".to_string(),
                    ),
                    required: true,
                },
            ],
            result_type: apothecarys_inventory::crafting::ResultType::Fixed {
                template_id: "potion_heal".to_string(),
                name: "Healing Potion".to_string(),
            },
        };

        let placed = vec![
            PlacedIngredient {
                slot_index: 0,
                template_id: "herb".to_string(),
                inventory_slot: 0,
            },
            PlacedIngredient {
                slot_index: 1,
                template_id: "empty_vial".to_string(),
                inventory_slot: 1,
            },
        ];

        let result = hub.craft(&recipe, &placed);
        assert!(matches!(result, CraftResult::Success { .. }));
    }

    #[test]
    fn test_hub_location_def() {
        let loc = hub_location_def();
        assert_eq!(loc.id.0, "hub");
        assert!(!loc.spawn_points.is_empty());
        assert!(!loc.exits.is_empty());
    }

    #[test]
    fn test_garden_location_def() {
        let loc = garden_location_def();
        assert_eq!(loc.id.0, "garden");
    }

    #[test]
    fn test_dungeon_location_def() {
        let loc = dungeon_f1_location_def();
        assert_eq!(loc.id.0, "dungeon_f1");
        assert!(matches!(
            loc.location_type,
            apothecarys_world::location::LocationType::Dungeon { .. }
        ));
    }

    #[test]
    fn test_build_default_world() {
        let world = build_default_world().unwrap();
        assert!(world
            .get_location(&apothecarys_world::location::LocationId::from("hub"))
            .is_some());
        assert!(world
            .get_location(&apothecarys_world::location::LocationId::from("garden"))
            .is_some());
        assert!(world
            .get_location(&apothecarys_world::location::LocationId::from("dungeon_f1"))
            .is_some());
    }

    #[test]
    fn test_refresh_recruitment_after_dungeon() {
        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);
        hub.refresh_recruitment(&mut rng);
        // New pool was generated
        assert!((3..=6).contains(&hub.recruitment_pool.candidate_count()));
    }

    #[test]
    fn test_dead_member_equipment_transfer() {
        use apothecarys_core::items::{EquipmentData, EquipmentSlot};

        let mut rng = test_rng();
        let mut hub = HubState::new(&mut rng);

        let mut member = generate_party_member(&mut rng, 1);
        member.alive = false;
        member.equipment.weapon = Some(Item::new(
            "sword",
            "Sword",
            ItemType::Equipment(EquipmentData {
                slot: EquipmentSlot::Weapon,
                armor_bonus: 0,
                attack_bonus: 2,
            }),
        ));
        hub.roster.add_member(member).unwrap();

        hub.return_from_dungeon(&mut rng);
        // Equipment should have been transferred to inventory
        assert!(hub.inventory.has_item("sword", 1));
    }
}
