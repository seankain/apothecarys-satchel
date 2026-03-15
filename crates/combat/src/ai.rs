use apothecarys_core::stats::{Combatant, DamageType};
use apothecarys_party::generation::{Personality, PartyClass};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::turn_manager::CombatantId;

/// An action that a combatant can take during combat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CombatAction {
    /// Basic melee/ranged attack against a target.
    Attack { target: CombatantId },
    /// Defend (raise AC for one round).
    Defend,
    /// Class-specific ability.
    Ability {
        ability_name: String,
        target: Option<CombatantId>,
    },
    /// Skip turn.
    Wait,
}

/// Template for enemy action selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnemyAction {
    pub name: String,
    pub weight: u32,
    pub damage_dice_count: u32,
    pub damage_dice_sides: u32,
    pub damage_bonus: i32,
    pub damage_type: DamageType,
    pub targeting: TargetingRule,
}

/// How an enemy selects its target.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TargetingRule {
    LowestHp,
    HighestThreat,
    Random,
}

/// Template for an enemy type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnemyTemplate {
    pub id: String,
    pub name: String,
    pub actions: Vec<EnemyAction>,
}

/// The result of AI action selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedEnemyAction {
    pub action: EnemyAction,
    pub target: CombatantId,
}

/// Evaluate how much danger a combatant is in (0.0 = safe, 1.0 = critical).
pub fn evaluate_danger(combatant: &Combatant) -> f32 {
    let hp_ratio = combatant.base_stats.current_hp as f32 / combatant.base_stats.max_hp as f32;
    1.0 - hp_ratio.clamp(0.0, 1.0)
}

/// Find the ally in most danger (lowest HP ratio).
pub fn most_endangered_ally<'a>(
    allies: &[(&'a Combatant, CombatantId)],
) -> Option<(&'a Combatant, CombatantId)> {
    allies
        .iter()
        .filter(|(c, _)| !c.is_dead())
        .max_by(|(a, _), (b, _)| {
            evaluate_danger(a)
                .partial_cmp(&evaluate_danger(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
}

/// Find the best enemy target (weakest / lowest HP).
pub fn best_attack_target<'a>(
    enemies: &[(&'a Combatant, CombatantId)],
) -> Option<(&'a Combatant, CombatantId)> {
    enemies
        .iter()
        .filter(|(c, _)| !c.is_dead())
        .min_by_key(|(c, _)| c.base_stats.current_hp)
        .copied()
}

/// Select an action for a party member based on personality and battlefield state.
pub fn select_party_action(
    _class: PartyClass,
    personality: &Personality,
    self_combatant: &Combatant,
    allies: &[(&Combatant, CombatantId)],
    enemies: &[(&Combatant, CombatantId)],
) -> CombatAction {
    let self_danger = evaluate_danger(self_combatant);

    // High danger + cautious -> defend
    if self_danger > 0.7 && personality.caution > 0.5 {
        return CombatAction::Defend;
    }

    // Ally in danger + team-focused -> attack the biggest threat (protect allies)
    if let Some((ally, _)) = most_endangered_ally(allies) {
        let ally_danger = evaluate_danger(ally);
        if ally_danger > 0.6 && personality.team_focus > 0.6 {
            // Attack the enemy most threatening the ally (lowest HP enemy first)
            if let Some((_, target_id)) = best_attack_target(enemies) {
                return CombatAction::Attack { target: target_id };
            }
        }
    }

    // High aggression -> attack strongest/weakest
    if personality.aggression > 0.7 {
        if let Some((_, target_id)) = best_attack_target(enemies) {
            return CombatAction::Attack { target: target_id };
        }
    }

    // Default: attack weakest enemy
    if let Some((_, target_id)) = best_attack_target(enemies) {
        return CombatAction::Attack { target: target_id };
    }

    CombatAction::Wait
}

/// Select an action and target for an enemy using its template.
pub fn select_enemy_action(
    template: &EnemyTemplate,
    allies_of_player: &[(&Combatant, CombatantId)],
    rng: &mut impl Rng,
) -> Option<SelectedEnemyAction> {
    if template.actions.is_empty() || allies_of_player.is_empty() {
        return None;
    }

    // Weighted random action selection
    let total_weight: u32 = template.actions.iter().map(|a| a.weight).sum();
    if total_weight == 0 {
        return None;
    }

    let mut roll = rng.gen_range(0..total_weight);
    let mut chosen_action = &template.actions[0];
    for action in &template.actions {
        if roll < action.weight {
            chosen_action = action;
            break;
        }
        roll -= action.weight;
    }

    // Select target based on targeting rule
    let living: Vec<_> = allies_of_player
        .iter()
        .filter(|(c, _)| !c.is_dead())
        .copied()
        .collect();

    if living.is_empty() {
        return None;
    }

    let target = match chosen_action.targeting {
        TargetingRule::LowestHp => living
            .iter()
            .min_by_key(|(c, _)| c.base_stats.current_hp)
            .map(|(_, id)| *id),
        TargetingRule::HighestThreat => {
            // Approximate threat as highest attack bonus
            living
                .iter()
                .max_by_key(|(c, _)| c.effective_attack_bonus())
                .map(|(_, id)| *id)
        }
        TargetingRule::Random => {
            let idx = rng.gen_range(0..living.len());
            Some(living[idx].1)
        }
    };

    target.map(|t| SelectedEnemyAction {
        action: chosen_action.clone(),
        target: t,
    })
}

/// Roll damage for an enemy action.
pub fn roll_enemy_damage(action: &EnemyAction, rng: &mut impl Rng) -> i32 {
    let mut total = action.damage_bonus;
    for _ in 0..action.damage_dice_count {
        total += rng.gen_range(1..=action.damage_dice_sides as i32);
    }
    total.max(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_core::stats::{Attributes, DerivedStats};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn make_combatant(name: &str, hp: i32, max_hp: i32) -> Combatant {
        Combatant::new(
            name,
            1,
            Attributes::default(),
            DerivedStats {
                max_hp,
                current_hp: hp,
                ..Default::default()
            },
        )
    }

    fn make_id() -> CombatantId {
        CombatantId(uuid::Uuid::new_v4())
    }

    #[test]
    fn test_evaluate_danger() {
        let full = make_combatant("Full", 20, 20);
        let half = make_combatant("Half", 10, 20);
        let critical = make_combatant("Critical", 2, 20);

        assert!((evaluate_danger(&full) - 0.0).abs() < f32::EPSILON);
        assert!((evaluate_danger(&half) - 0.5).abs() < f32::EPSILON);
        assert!((evaluate_danger(&critical) - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_most_endangered_ally() {
        let a = make_combatant("A", 20, 20);
        let b = make_combatant("B", 5, 20);
        let id_a = make_id();
        let id_b = make_id();

        let allies = vec![(&a, id_a), (&b, id_b)];
        let (_, most_id) = most_endangered_ally(&allies).unwrap();
        assert_eq!(most_id, id_b);
    }

    #[test]
    fn test_best_attack_target() {
        let a = make_combatant("A", 15, 20);
        let b = make_combatant("B", 3, 10);
        let id_a = make_id();
        let id_b = make_id();

        let enemies = vec![(&a, id_a), (&b, id_b)];
        let (_, target_id) = best_attack_target(&enemies).unwrap();
        assert_eq!(target_id, id_b);
    }

    #[test]
    fn test_cautious_member_defends_when_hurt() {
        let personality = Personality {
            aggression: 0.3,
            caution: 0.8,
            team_focus: 0.3,
            item_affinity: 0.1,
        };
        let self_c = make_combatant("Self", 4, 20); // 80% danger
        let enemy = make_combatant("Enemy", 10, 10);
        let enemy_id = make_id();

        let action = select_party_action(
            PartyClass::Warrior,
            &personality,
            &self_c,
            &[],
            &[(&enemy, enemy_id)],
        );

        assert_eq!(action, CombatAction::Defend);
    }

    #[test]
    fn test_aggressive_member_attacks() {
        let personality = Personality {
            aggression: 0.9,
            caution: 0.1,
            team_focus: 0.1,
            item_affinity: 0.1,
        };
        let self_c = make_combatant("Self", 20, 20);
        let enemy = make_combatant("Enemy", 10, 10);
        let enemy_id = make_id();

        let action = select_party_action(
            PartyClass::Warrior,
            &personality,
            &self_c,
            &[],
            &[(&enemy, enemy_id)],
        );

        assert!(matches!(action, CombatAction::Attack { .. }));
    }

    #[test]
    fn test_team_focused_member_attacks_when_ally_hurt() {
        let personality = Personality {
            aggression: 0.3,
            caution: 0.3,
            team_focus: 0.9,
            item_affinity: 0.1,
        };
        let self_c = make_combatant("Self", 20, 20);
        let hurt_ally = make_combatant("HurtAlly", 3, 20); // 85% danger
        let ally_id = make_id();
        let enemy = make_combatant("Enemy", 10, 10);
        let enemy_id = make_id();

        let action = select_party_action(
            PartyClass::Warrior,
            &personality,
            &self_c,
            &[(&hurt_ally, ally_id)],
            &[(&enemy, enemy_id)],
        );

        assert!(matches!(action, CombatAction::Attack { .. }));
    }

    #[test]
    fn test_default_action_attacks() {
        let personality = Personality {
            aggression: 0.5,
            caution: 0.5,
            team_focus: 0.5,
            item_affinity: 0.5,
        };
        let self_c = make_combatant("Self", 20, 20);
        let enemy = make_combatant("Enemy", 10, 10);
        let enemy_id = make_id();

        let action = select_party_action(
            PartyClass::Warrior,
            &personality,
            &self_c,
            &[],
            &[(&enemy, enemy_id)],
        );

        assert!(matches!(action, CombatAction::Attack { .. }));
    }

    #[test]
    fn test_no_enemies_waits() {
        let personality = Personality {
            aggression: 0.9,
            caution: 0.1,
            team_focus: 0.1,
            item_affinity: 0.1,
        };
        let self_c = make_combatant("Self", 20, 20);

        let action = select_party_action(
            PartyClass::Warrior,
            &personality,
            &self_c,
            &[],
            &[],
        );

        assert_eq!(action, CombatAction::Wait);
    }

    #[test]
    fn test_enemy_action_selection() {
        let mut rng = StdRng::seed_from_u64(42);
        let template = EnemyTemplate {
            id: "goblin".to_string(),
            name: "Goblin".to_string(),
            actions: vec![
                EnemyAction {
                    name: "Slash".to_string(),
                    weight: 3,
                    damage_dice_count: 1,
                    damage_dice_sides: 6,
                    damage_bonus: 2,
                    damage_type: DamageType::Physical,
                    targeting: TargetingRule::LowestHp,
                },
                EnemyAction {
                    name: "Shield Bash".to_string(),
                    weight: 1,
                    damage_dice_count: 1,
                    damage_dice_sides: 4,
                    damage_bonus: 0,
                    damage_type: DamageType::Physical,
                    targeting: TargetingRule::Random,
                },
            ],
        };

        let ally = make_combatant("Ally", 10, 20);
        let ally_id = make_id();
        let allies = vec![(&ally, ally_id)];

        let result = select_enemy_action(&template, &allies, &mut rng);
        assert!(result.is_some());
        let selected = result.unwrap();
        assert_eq!(selected.target, ally_id);
    }

    #[test]
    fn test_enemy_targets_lowest_hp() {
        let mut rng = StdRng::seed_from_u64(42);
        let template = EnemyTemplate {
            id: "goblin".to_string(),
            name: "Goblin".to_string(),
            actions: vec![EnemyAction {
                name: "Slash".to_string(),
                weight: 1,
                damage_dice_count: 1,
                damage_dice_sides: 6,
                damage_bonus: 0,
                damage_type: DamageType::Physical,
                targeting: TargetingRule::LowestHp,
            }],
        };

        let a = make_combatant("Strong", 20, 20);
        let b = make_combatant("Weak", 5, 20);
        let id_a = make_id();
        let id_b = make_id();
        let allies = vec![(&a, id_a), (&b, id_b)];

        let result = select_enemy_action(&template, &allies, &mut rng).unwrap();
        assert_eq!(result.target, id_b);
    }

    #[test]
    fn test_enemy_no_living_targets() {
        let mut rng = StdRng::seed_from_u64(42);
        let template = EnemyTemplate {
            id: "goblin".to_string(),
            name: "Goblin".to_string(),
            actions: vec![EnemyAction {
                name: "Slash".to_string(),
                weight: 1,
                damage_dice_count: 1,
                damage_dice_sides: 6,
                damage_bonus: 0,
                damage_type: DamageType::Physical,
                targeting: TargetingRule::Random,
            }],
        };

        let dead = make_combatant("Dead", -5, 20);
        let dead_id = make_id();
        let allies = vec![(&dead, dead_id)];

        let result = select_enemy_action(&template, &allies, &mut rng);
        assert!(result.is_none());
    }

    #[test]
    fn test_roll_enemy_damage() {
        let mut rng = StdRng::seed_from_u64(42);
        let action = EnemyAction {
            name: "Slash".to_string(),
            weight: 1,
            damage_dice_count: 2,
            damage_dice_sides: 6,
            damage_bonus: 3,
            damage_type: DamageType::Physical,
            targeting: TargetingRule::Random,
        };

        // Roll many times and check range: min=2+3=5, max=12+3=15
        for _ in 0..100 {
            let dmg = roll_enemy_damage(&action, &mut rng);
            assert!((5..=15).contains(&dmg), "Damage {dmg} out of range");
        }
    }
}
