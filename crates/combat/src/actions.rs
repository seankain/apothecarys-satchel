use apothecarys_core::items::{AlchemyEffect, Item, ItemType, StatusEffectType};
use apothecarys_core::stats::{Combatant, DamageType, StatusEffect};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::turn_manager::CombatantId;

/// Actions available to the player (Apothecary) during combat.
/// The player cannot attack directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlayerAction {
    /// Use an item on a target (potion on ally = heal/buff, on enemy = damage/debuff).
    UseItem {
        item_id: Uuid,
        target: CombatantId,
    },
    /// Give a consumable item to a party member for their own use.
    GiveItem {
        item_id: Uuid,
        target: CombatantId,
    },
    /// Examine an enemy to reveal stats/weaknesses (INT check).
    Examine {
        target: CombatantId,
    },
    /// Skip the turn.
    Wait,
}

/// Result of examining an enemy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExamineResult {
    pub success: bool,
    pub target_name: String,
    pub revealed_hp: Option<(i32, i32)>,
    pub revealed_weaknesses: Vec<DamageType>,
}

/// Result of using an item in combat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UseItemResult {
    pub item_name: String,
    pub target_name: String,
    pub effects_applied: Vec<String>,
}

/// Apply alchemy effects from a potion or medicine to a combatant.
pub fn apply_item_effects(item: &Item, target: &mut Combatant) -> Vec<String> {
    let mut descriptions = Vec::new();

    match &item.item_type {
        ItemType::Potion { effects } => {
            for effect in effects {
                match effect {
                    AlchemyEffect::Heal { amount } => {
                        target.heal(*amount);
                        descriptions.push(format!("Healed for {amount} HP"));
                    }
                    AlchemyEffect::Damage { amount, damage_type } => {
                        let dealt = target.take_damage(*amount, *damage_type);
                        descriptions.push(format!("Dealt {dealt} {damage_type:?} damage"));
                    }
                    AlchemyEffect::Buff { effect, turns } => {
                        target.status_effects.apply(effect.clone(), *turns);
                        descriptions.push(format!("Applied {effect:?} for {turns} turns"));
                    }
                    AlchemyEffect::Cure { cures } => {
                        let to_remove = match cures {
                            StatusEffectType::Poisoned => {
                                StatusEffect::Poisoned { damage_per_turn: 0 }
                            }
                            StatusEffectType::Weakened => {
                                StatusEffect::Weakened { attack_penalty: 0 }
                            }
                            StatusEffectType::Slowed => StatusEffect::Slowed,
                            StatusEffectType::Stunned => StatusEffect::Stunned,
                            StatusEffectType::Blinded => StatusEffect::Blinded,
                        };
                        target.status_effects.remove(&to_remove);
                        descriptions.push(format!("Cured {cures:?}"));
                    }
                    AlchemyEffect::StatBoost {
                        attribute,
                        amount,
                        turns,
                    } => {
                        target.status_effects.apply(
                            StatusEffect::StatBoost {
                                attribute: *attribute,
                                amount: *amount,
                            },
                            *turns,
                        );
                        descriptions.push(format!(
                            "Boosted {attribute:?} by {amount} for {turns} turns"
                        ));
                    }
                }
            }
        }
        ItemType::Medicine { cures } => {
            for cure in cures {
                let to_remove = match cure {
                    StatusEffectType::Poisoned => {
                        StatusEffect::Poisoned { damage_per_turn: 0 }
                    }
                    StatusEffectType::Weakened => StatusEffect::Weakened { attack_penalty: 0 },
                    StatusEffectType::Slowed => StatusEffect::Slowed,
                    StatusEffectType::Stunned => StatusEffect::Stunned,
                    StatusEffectType::Blinded => StatusEffect::Blinded,
                };
                target.status_effects.remove(&to_remove);
                descriptions.push(format!("Cured {cure:?}"));
            }
        }
        _ => {
            descriptions.push("Item has no combat effect".to_string());
        }
    }

    descriptions
}

/// Perform an INT check to examine an enemy.
/// DC is 10 + enemy level. Roll d20 + INT modifier.
pub fn resolve_examine(
    player_int_mod: i32,
    target: &Combatant,
    rng: &mut impl Rng,
) -> ExamineResult {
    let dc = 10 + target.level as i32;
    let roll = rng.gen_range(1..=20) + player_int_mod;
    let success = roll >= dc;

    ExamineResult {
        success,
        target_name: target.name.clone(),
        revealed_hp: if success {
            Some((
                target.base_stats.current_hp,
                target.base_stats.max_hp,
            ))
        } else {
            None
        },
        revealed_weaknesses: if success && roll >= dc + 5 {
            // Exceptional success reveals weaknesses
            // For now, no specific weakness system; return empty
            Vec::new()
        } else {
            Vec::new()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_core::stats::{AttributeType, Attributes, DerivedStats};
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

    fn make_heal_potion(amount: i32) -> Item {
        Item::new(
            "potion_heal",
            "Healing Potion",
            ItemType::Potion {
                effects: vec![AlchemyEffect::Heal { amount }],
            },
        )
    }

    fn make_damage_potion(amount: i32) -> Item {
        Item::new(
            "potion_fire",
            "Fire Bomb",
            ItemType::Potion {
                effects: vec![AlchemyEffect::Damage {
                    amount,
                    damage_type: DamageType::Fire,
                }],
            },
        )
    }

    #[test]
    fn test_apply_heal_potion() {
        let potion = make_heal_potion(10);
        let mut target = make_combatant("Ally", 10, 20);

        let effects = apply_item_effects(&potion, &mut target);
        assert_eq!(target.base_stats.current_hp, 20);
        assert_eq!(effects.len(), 1);
        assert!(effects[0].contains("Healed"));
    }

    #[test]
    fn test_apply_damage_potion() {
        let potion = make_damage_potion(8);
        let mut target = make_combatant("Enemy", 20, 20);

        let effects = apply_item_effects(&potion, &mut target);
        assert_eq!(target.base_stats.current_hp, 12);
        assert!(effects[0].contains("damage"));
    }

    #[test]
    fn test_apply_buff_potion() {
        let potion = Item::new(
            "potion_attack",
            "Attack Potion",
            ItemType::Potion {
                effects: vec![AlchemyEffect::Buff {
                    effect: StatusEffect::AttackUp { amount: 3 },
                    turns: 3,
                }],
            },
        );
        let mut target = make_combatant("Ally", 20, 20);

        let effects = apply_item_effects(&potion, &mut target);
        assert_eq!(target.status_effects.attack_modifier(), 3);
        assert_eq!(effects.len(), 1);
    }

    #[test]
    fn test_apply_cure_medicine() {
        let medicine = Item::new(
            "antidote",
            "Antidote",
            ItemType::Medicine {
                cures: vec![StatusEffectType::Poisoned],
            },
        );
        let mut target = make_combatant("Ally", 15, 20);
        target
            .status_effects
            .apply(StatusEffect::Poisoned { damage_per_turn: 5 }, 3);

        let effects = apply_item_effects(&medicine, &mut target);
        assert!(!target
            .status_effects
            .effects
            .iter()
            .any(|e| matches!(e.effect, StatusEffect::Poisoned { .. })));
        assert_eq!(effects.len(), 1);
        assert!(effects[0].contains("Cured"));
    }

    #[test]
    fn test_apply_stat_boost() {
        let potion = Item::new(
            "potion_str",
            "Strength Potion",
            ItemType::Potion {
                effects: vec![AlchemyEffect::StatBoost {
                    attribute: AttributeType::Strength,
                    amount: 4,
                    turns: 3,
                }],
            },
        );
        let mut target = make_combatant("Ally", 20, 20);

        apply_item_effects(&potion, &mut target);
        assert_eq!(
            target.status_effects.stat_boost(AttributeType::Strength),
            4
        );
    }

    #[test]
    fn test_apply_multi_effect_potion() {
        let potion = Item::new(
            "potion_combo",
            "Combo Potion",
            ItemType::Potion {
                effects: vec![
                    AlchemyEffect::Heal { amount: 5 },
                    AlchemyEffect::Buff {
                        effect: StatusEffect::DefenseUp { amount: 2 },
                        turns: 2,
                    },
                ],
            },
        );
        let mut target = make_combatant("Ally", 10, 20);

        let effects = apply_item_effects(&potion, &mut target);
        assert_eq!(target.base_stats.current_hp, 15);
        assert_eq!(target.status_effects.defense_modifier(), 2);
        assert_eq!(effects.len(), 2);
    }

    #[test]
    fn test_non_combat_item() {
        let item = Item::new("quest_item", "Ancient Key", ItemType::QuestItem);
        let mut target = make_combatant("Ally", 20, 20);

        let effects = apply_item_effects(&item, &mut target);
        assert_eq!(effects.len(), 1);
        assert!(effects[0].contains("no combat effect"));
    }

    #[test]
    fn test_examine_success() {
        let mut rng = StdRng::seed_from_u64(42);
        let target = make_combatant("Goblin", 8, 10);

        // Try many times; with INT mod +5, should succeed often
        let mut successes = 0;
        for _ in 0..100 {
            let result = resolve_examine(5, &target, &mut rng);
            if result.success {
                successes += 1;
                assert!(result.revealed_hp.is_some());
                let (current, max) = result.revealed_hp.unwrap();
                assert_eq!(current, 8);
                assert_eq!(max, 10);
            }
        }
        // With +5 mod vs DC 11, should succeed on 6+ (75% of the time)
        assert!(successes > 50);
    }

    #[test]
    fn test_examine_failure() {
        let mut rng = StdRng::seed_from_u64(42);
        let target = Combatant::new(
            "Dragon",
            10, // level 10
            Attributes::default(),
            DerivedStats {
                max_hp: 100,
                current_hp: 100,
                ..Default::default()
            },
        );

        // With INT mod 0, DC 20 — need nat 20 to succeed
        let mut failures = 0;
        for _ in 0..100 {
            let result = resolve_examine(0, &target, &mut rng);
            if !result.success {
                failures += 1;
                assert!(result.revealed_hp.is_none());
            }
        }
        // Should fail most of the time
        assert!(failures > 80);
    }

    #[test]
    fn test_damage_with_resistance() {
        let potion = make_damage_potion(10);
        let mut target = make_combatant("Enemy", 20, 20);
        target.status_effects.apply(
            StatusEffect::Resistance {
                damage_type: DamageType::Fire,
            },
            5,
        );

        let effects = apply_item_effects(&potion, &mut target);
        // Resistance halves fire damage: 10 / 2 = 5
        assert_eq!(target.base_stats.current_hp, 15);
        assert!(effects[0].contains("5"));
    }

    #[test]
    fn test_player_action_serde_roundtrip() {
        let actions = vec![
            PlayerAction::UseItem {
                item_id: Uuid::new_v4(),
                target: CombatantId(Uuid::new_v4()),
            },
            PlayerAction::GiveItem {
                item_id: Uuid::new_v4(),
                target: CombatantId(Uuid::new_v4()),
            },
            PlayerAction::Examine {
                target: CombatantId(Uuid::new_v4()),
            },
            PlayerAction::Wait,
        ];

        for action in &actions {
            let json = serde_json::to_string(action).unwrap();
            let deserialized: PlayerAction = serde_json::from_str(&json).unwrap();
            assert_eq!(*action, deserialized);
        }
    }
}
