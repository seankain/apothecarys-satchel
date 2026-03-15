use apothecarys_core::stats::{Combatant, DamageType, TickResult};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifies a combatant in the combat state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CombatantId(pub Uuid);

/// Which side a combatant is on.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CombatSide {
    Player,
    Party,
    Enemy,
}

/// A combatant entry in the combat state, wrapping the core Combatant with combat metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CombatantEntry {
    pub id: CombatantId,
    pub combatant: Combatant,
    pub side: CombatSide,
    pub initiative: i32,
}

/// Loot dropped by enemies on defeat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemDrop {
    pub template_id: String,
    pub name: String,
    pub count: u32,
}

/// The current phase of combat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CombatPhase {
    RollInitiative,
    TurnStart {
        combatant_id: CombatantId,
    },
    ActionSelection,
    ActionExecution,
    TurnEnd,
    RoundEnd,
    Victory {
        xp: u32,
        loot: Vec<ItemDrop>,
    },
    Defeat,
}

/// The full combat state machine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CombatState {
    pub combatants: Vec<CombatantEntry>,
    pub current_turn: usize,
    pub round: u32,
    pub phase: CombatPhase,
    /// XP reward for victory (sum of enemy XP values).
    pub xp_reward: u32,
    /// Loot for victory.
    pub loot: Vec<ItemDrop>,
}

impl CombatState {
    /// Create a new combat with the given party and enemy combatants.
    pub fn new(
        player: Combatant,
        party: Vec<Combatant>,
        enemies: Vec<Combatant>,
        xp_reward: u32,
        loot: Vec<ItemDrop>,
    ) -> Self {
        let mut combatants = Vec::new();

        combatants.push(CombatantEntry {
            id: CombatantId(Uuid::new_v4()),
            combatant: player,
            side: CombatSide::Player,
            initiative: 0,
        });

        for c in party {
            combatants.push(CombatantEntry {
                id: CombatantId(Uuid::new_v4()),
                combatant: c,
                side: CombatSide::Party,
                initiative: 0,
            });
        }

        for c in enemies {
            combatants.push(CombatantEntry {
                id: CombatantId(Uuid::new_v4()),
                combatant: c,
                side: CombatSide::Enemy,
                initiative: 0,
            });
        }

        Self {
            combatants,
            current_turn: 0,
            round: 0,
            phase: CombatPhase::RollInitiative,
            xp_reward,
            loot,
        }
    }

    /// Roll initiative for all combatants and sort by initiative (descending).
    /// Ties broken by dexterity (higher first).
    pub fn roll_initiative(&mut self, rng: &mut impl Rng) {
        for entry in &mut self.combatants {
            let d20 = rng.gen_range(1..=20);
            entry.initiative = d20 + entry.combatant.base_stats.initiative_bonus;
        }

        self.combatants.sort_by(|a, b| {
            b.initiative
                .cmp(&a.initiative)
                .then_with(|| b.combatant.attributes.dexterity.cmp(&a.combatant.attributes.dexterity))
        });

        self.current_turn = 0;
        self.round = 1;
        self.phase = CombatPhase::TurnStart {
            combatant_id: self.combatants[0].id,
        };
    }

    /// Get the current combatant entry.
    pub fn current_combatant(&self) -> Option<&CombatantEntry> {
        self.combatants.get(self.current_turn)
    }

    /// Get a combatant by ID.
    pub fn get_combatant(&self, id: CombatantId) -> Option<&CombatantEntry> {
        self.combatants.iter().find(|e| e.id == id)
    }

    /// Get a mutable combatant by ID.
    pub fn get_combatant_mut(&mut self, id: CombatantId) -> Option<&mut CombatantEntry> {
        self.combatants.iter_mut().find(|e| e.id == id)
    }

    /// Advance from TurnStart to ActionSelection.
    /// If the current combatant is stunned, skip to TurnEnd.
    pub fn begin_action_selection(&mut self) {
        if let CombatPhase::TurnStart { combatant_id } = self.phase {
            if let Some(entry) = self.get_combatant(combatant_id) {
                if entry.combatant.status_effects.is_stunned() {
                    self.phase = CombatPhase::TurnEnd;
                    return;
                }
            }
            self.phase = CombatPhase::ActionSelection;
        }
    }

    /// Advance from ActionSelection to ActionExecution.
    pub fn begin_action_execution(&mut self) {
        if self.phase == CombatPhase::ActionSelection {
            self.phase = CombatPhase::ActionExecution;
        }
    }

    /// Apply damage from one combatant to another. Returns actual damage dealt.
    pub fn apply_damage(
        &mut self,
        target_id: CombatantId,
        amount: i32,
        damage_type: DamageType,
    ) -> i32 {
        if let Some(entry) = self.get_combatant_mut(target_id) {
            entry.combatant.take_damage(amount, damage_type)
        } else {
            0
        }
    }

    /// Heal a combatant.
    pub fn apply_heal(&mut self, target_id: CombatantId, amount: i32) {
        if let Some(entry) = self.get_combatant_mut(target_id) {
            entry.combatant.heal(amount);
        }
    }

    /// Advance from ActionExecution to TurnEnd, ticking status effects
    /// on the current combatant. Returns tick results.
    pub fn end_turn(&mut self) -> Vec<TickResult> {
        if self.phase != CombatPhase::ActionExecution && self.phase != CombatPhase::TurnEnd {
            return Vec::new();
        }

        let idx = self.current_turn;
        let results = if let Some(entry) = self.combatants.get_mut(idx) {
            entry.combatant.tick_effects()
        } else {
            Vec::new()
        };

        // Check if combat is over
        if self.check_victory() {
            self.phase = CombatPhase::Victory {
                xp: self.xp_reward,
                loot: self.loot.clone(),
            };
        } else if self.check_defeat() {
            self.phase = CombatPhase::Defeat;
        } else {
            self.phase = CombatPhase::RoundEnd;
        }

        results
    }

    /// Advance to the next combatant's turn (or the next round).
    pub fn advance_turn(&mut self) {
        if !matches!(self.phase, CombatPhase::RoundEnd) {
            return;
        }

        // Find next living combatant
        let total = self.combatants.len();
        let mut next = (self.current_turn + 1) % total;
        let mut checked = 0;

        while checked < total {
            if !self.combatants[next].combatant.is_dead() {
                break;
            }
            next = (next + 1) % total;
            checked += 1;
        }

        if checked >= total {
            // All dead — shouldn't happen if victory/defeat was checked
            self.phase = CombatPhase::Defeat;
            return;
        }

        // Check if we've wrapped around (new round)
        if next <= self.current_turn {
            self.round += 1;
        }

        self.current_turn = next;
        self.phase = CombatPhase::TurnStart {
            combatant_id: self.combatants[next].id,
        };
    }

    /// Check if all enemies are dead.
    pub fn check_victory(&self) -> bool {
        self.combatants
            .iter()
            .filter(|e| e.side == CombatSide::Enemy)
            .all(|e| e.combatant.is_dead())
    }

    /// Check if all party members and the player are dead.
    pub fn check_defeat(&self) -> bool {
        self.combatants
            .iter()
            .filter(|e| e.side == CombatSide::Player || e.side == CombatSide::Party)
            .all(|e| e.combatant.is_dead())
    }

    /// Get all living combatants on a given side.
    pub fn living_on_side(&self, side: CombatSide) -> Vec<&CombatantEntry> {
        self.combatants
            .iter()
            .filter(|e| e.side == side && !e.combatant.is_dead())
            .collect()
    }

    /// Get all living enemies.
    pub fn living_enemies(&self) -> Vec<&CombatantEntry> {
        self.living_on_side(CombatSide::Enemy)
    }

    /// Get all living allies (player + party).
    pub fn living_allies(&self) -> Vec<&CombatantEntry> {
        self.combatants
            .iter()
            .filter(|e| {
                (e.side == CombatSide::Player || e.side == CombatSide::Party)
                    && !e.combatant.is_dead()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_core::stats::{Attributes, DerivedStats, StatusEffect};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn make_combatant(name: &str, hp: i32) -> Combatant {
        Combatant::new(
            name,
            1,
            Attributes::default(),
            DerivedStats {
                max_hp: hp,
                current_hp: hp,
                ..Default::default()
            },
        )
    }

    fn make_combat() -> CombatState {
        let player = make_combatant("Player", 20);
        let ally = make_combatant("Ally", 15);
        let enemy1 = make_combatant("Goblin", 10);
        let enemy2 = make_combatant("Orc", 20);

        CombatState::new(
            player,
            vec![ally],
            vec![enemy1, enemy2],
            100,
            vec![ItemDrop {
                template_id: "gold".to_string(),
                name: "Gold".to_string(),
                count: 50,
            }],
        )
    }

    #[test]
    fn test_combat_creation() {
        let combat = make_combat();
        assert_eq!(combat.combatants.len(), 4);
        assert_eq!(combat.round, 0);
        assert_eq!(combat.phase, CombatPhase::RollInitiative);
    }

    #[test]
    fn test_roll_initiative_sorts() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        assert_eq!(combat.round, 1);
        // Initiative should be sorted descending
        for i in 1..combat.combatants.len() {
            assert!(combat.combatants[i - 1].initiative >= combat.combatants[i].initiative);
        }
        // Phase should be TurnStart
        assert!(matches!(combat.phase, CombatPhase::TurnStart { .. }));
    }

    #[test]
    fn test_phase_transitions() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        // TurnStart -> ActionSelection
        combat.begin_action_selection();
        assert_eq!(combat.phase, CombatPhase::ActionSelection);

        // ActionSelection -> ActionExecution
        combat.begin_action_execution();
        assert_eq!(combat.phase, CombatPhase::ActionExecution);

        // ActionExecution -> RoundEnd (via end_turn)
        combat.end_turn();
        assert_eq!(combat.phase, CombatPhase::RoundEnd);

        // RoundEnd -> TurnStart (next combatant)
        combat.advance_turn();
        assert!(matches!(combat.phase, CombatPhase::TurnStart { .. }));
    }

    #[test]
    fn test_apply_damage() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        let enemy_id = combat.combatants.iter().find(|e| e.side == CombatSide::Enemy).unwrap().id;
        let dealt = combat.apply_damage(enemy_id, 5, DamageType::Physical);
        assert_eq!(dealt, 5);
    }

    #[test]
    fn test_apply_heal() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        let ally_id = combat.combatants.iter().find(|e| e.side == CombatSide::Party).unwrap().id;
        // Damage then heal
        combat.apply_damage(ally_id, 10, DamageType::Physical);
        combat.apply_heal(ally_id, 5);

        let ally = combat.get_combatant(ally_id).unwrap();
        assert_eq!(ally.combatant.base_stats.current_hp, 10);
    }

    #[test]
    fn test_victory_detection() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        // Kill all enemies
        let enemy_ids: Vec<_> = combat
            .combatants
            .iter()
            .filter(|e| e.side == CombatSide::Enemy)
            .map(|e| e.id)
            .collect();

        for id in enemy_ids {
            combat.apply_damage(id, 100, DamageType::Physical);
        }

        assert!(combat.check_victory());
        assert!(!combat.check_defeat());
    }

    #[test]
    fn test_defeat_detection() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        // Kill all allies
        let ally_ids: Vec<_> = combat
            .combatants
            .iter()
            .filter(|e| e.side != CombatSide::Enemy)
            .map(|e| e.id)
            .collect();

        for id in ally_ids {
            combat.apply_damage(id, 100, DamageType::Physical);
        }

        assert!(!combat.check_victory());
        assert!(combat.check_defeat());
    }

    #[test]
    fn test_stunned_combatant_skips_turn() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        // Stun the first combatant
        if let CombatPhase::TurnStart { combatant_id } = combat.phase {
            let entry = combat.get_combatant_mut(combatant_id).unwrap();
            entry.combatant.status_effects.apply(StatusEffect::Stunned, 1);
        }

        // begin_action_selection should skip to TurnEnd
        combat.begin_action_selection();
        assert_eq!(combat.phase, CombatPhase::TurnEnd);
    }

    #[test]
    fn test_status_effect_ticking() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        // Apply poison to first combatant
        if let CombatPhase::TurnStart { combatant_id } = combat.phase {
            let entry = combat.get_combatant_mut(combatant_id).unwrap();
            entry
                .combatant
                .status_effects
                .apply(StatusEffect::Poisoned { damage_per_turn: 3 }, 2);
        }

        combat.begin_action_selection();
        combat.begin_action_execution();
        let results = combat.end_turn();

        assert!(results.contains(&TickResult::Damage(3)));
    }

    #[test]
    fn test_advance_turn_skips_dead() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        // Kill the second combatant
        let second_id = combat.combatants[1].id;
        combat.apply_damage(second_id, 100, DamageType::Physical);

        // Go through first combatant's turn
        combat.begin_action_selection();
        combat.begin_action_execution();
        combat.end_turn();
        combat.advance_turn();

        // Should skip the dead second combatant
        if let CombatPhase::TurnStart { combatant_id } = combat.phase {
            assert_ne!(combatant_id, second_id);
        } else {
            panic!("Expected TurnStart phase");
        }
    }

    #[test]
    fn test_victory_ends_combat() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        // Kill all enemies
        let enemy_ids: Vec<_> = combat
            .combatants
            .iter()
            .filter(|e| e.side == CombatSide::Enemy)
            .map(|e| e.id)
            .collect();
        for id in enemy_ids {
            combat.apply_damage(id, 100, DamageType::Physical);
        }

        // Progress through a turn
        combat.begin_action_selection();
        combat.begin_action_execution();
        combat.end_turn();

        assert!(matches!(combat.phase, CombatPhase::Victory { .. }));
    }

    #[test]
    fn test_living_enemies_and_allies() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        assert_eq!(combat.living_enemies().len(), 2);
        assert_eq!(combat.living_allies().len(), 2);

        // Kill one enemy
        let enemy_id = combat
            .combatants
            .iter()
            .find(|e| e.side == CombatSide::Enemy)
            .unwrap()
            .id;
        combat.apply_damage(enemy_id, 100, DamageType::Physical);

        assert_eq!(combat.living_enemies().len(), 1);
        assert_eq!(combat.living_allies().len(), 2);
    }

    #[test]
    fn test_round_counter_increments() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);
        assert_eq!(combat.round, 1);

        // Complete all turns in round 1
        let combatant_count = combat.combatants.len();
        for _ in 0..combatant_count {
            combat.begin_action_selection();
            combat.begin_action_execution();
            combat.end_turn();
            if matches!(combat.phase, CombatPhase::Victory { .. } | CombatPhase::Defeat) {
                break;
            }
            combat.advance_turn();
        }

        assert_eq!(combat.round, 2);
    }

    #[test]
    fn test_combat_serde_roundtrip() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        let json = serde_json::to_string(&combat).unwrap();
        let deserialized: CombatState = serde_json::from_str(&json).unwrap();
        assert_eq!(combat, deserialized);
    }
}
