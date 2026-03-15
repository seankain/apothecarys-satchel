use apothecarys_core::items::AlchemyEffect;
use apothecarys_core::stats::{AttributeType, DamageType, StatusEffect};
use serde::{Deserialize, Serialize};

use crate::genetics::PlantGenotype;

/// Configuration for how genetics map to alchemy effects. Data-driven via RON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatMappingConfig {
    /// Threshold above which healing_affinity produces healing effects
    pub healing_threshold: f32,
    /// Min/max heal amount
    pub heal_range: (i32, i32),
    /// Min/max damage amount
    pub damage_range: (i32, i32),
    /// Potency threshold for bonus stat buff
    pub buff_potency_threshold: f32,
    /// Min/max stat boost from high-potency healing plants
    pub buff_range: (i32, i32),
    /// Min/max stat debuff from harmful plants
    pub debuff_range: (i32, i32),
    /// Min/max duration in turns
    pub duration_range: (u32, u32),
    /// Toxicity threshold for side effects
    pub toxicity_threshold: f32,
    /// Min/max side effect damage
    pub side_effect_range: (i32, i32),
}

impl Default for StatMappingConfig {
    fn default() -> Self {
        Self {
            healing_threshold: 0.5,
            heal_range: (5, 30),
            damage_range: (3, 20),
            buff_potency_threshold: 0.7,
            buff_range: (1, 4),
            debuff_range: (1, 3),
            duration_range: (1, 5),
            toxicity_threshold: 0.6,
            side_effect_range: (1, 10),
        }
    }
}

/// Map a value from [0.0, 1.0] to an integer range [min, max].
fn map_range_i32(value: f32, min: i32, max: i32) -> i32 {
    let v = value.clamp(0.0, 1.0);
    min + (v * (max - min) as f32).round() as i32
}

/// Map a value from [0.0, 1.0] to a u32 range [min, max].
fn map_range_u32(value: f32, min: u32, max: u32) -> u32 {
    let v = value.clamp(0.0, 1.0);
    min + (v * (max - min) as f32).round() as u32
}

/// Map a value in a sub-range [from_min, from_max] to an integer range [to_min, to_max].
fn map_subrange_i32(value: f32, from_min: f32, from_max: f32, to_min: i32, to_max: i32) -> i32 {
    let normalized = ((value - from_min) / (from_max - from_min)).clamp(0.0, 1.0);
    map_range_i32(normalized, to_min, to_max)
}

/// Determine which attribute a plant targets based on stat_target gene.
fn resolve_stat_target(stat_target_value: f32) -> AttributeType {
    match (stat_target_value * 6.0) as u32 {
        0 => AttributeType::Strength,
        1 => AttributeType::Dexterity,
        2 => AttributeType::Constitution,
        3 => AttributeType::Intelligence,
        4 => AttributeType::Wisdom,
        _ => AttributeType::Charisma,
    }
}

/// Map a plant's hidden genetics to alchemy effects using the default config.
pub fn genetics_to_effects(genotype: &PlantGenotype) -> Vec<AlchemyEffect> {
    genetics_to_effects_with_config(genotype, &StatMappingConfig::default())
}

/// Map a plant's hidden genetics to alchemy effects using a custom config.
pub fn genetics_to_effects_with_config(
    genotype: &PlantGenotype,
    config: &StatMappingConfig,
) -> Vec<AlchemyEffect> {
    let mut effects = Vec::new();

    let healing = genotype.healing_affinity.express();
    let potency = genotype.potency.express();
    let stat_target_value = genotype.stat_target.express();
    let duration = map_range_u32(
        genotype.duration_gene.express(),
        config.duration_range.0,
        config.duration_range.1,
    );
    let toxicity = genotype.toxicity.express();

    let target_stat = resolve_stat_target(stat_target_value);

    if healing > config.healing_threshold {
        // Healing plant
        let heal_amount = map_range_i32(potency, config.heal_range.0, config.heal_range.1);
        effects.push(AlchemyEffect::Heal {
            amount: heal_amount,
        });

        if potency > config.buff_potency_threshold {
            let boost = map_subrange_i32(
                potency,
                config.buff_potency_threshold,
                1.0,
                config.buff_range.0,
                config.buff_range.1,
            );
            effects.push(AlchemyEffect::StatBoost {
                attribute: target_stat,
                amount: boost,
                turns: duration,
            });
        }
    } else {
        // Harmful plant (poisons, debuffs)
        let damage = map_range_i32(potency, config.damage_range.0, config.damage_range.1);
        effects.push(AlchemyEffect::Damage {
            amount: damage,
            damage_type: DamageType::Poison,
        });

        let debuff_amount =
            map_range_i32(potency, config.debuff_range.0, config.debuff_range.1);
        effects.push(AlchemyEffect::Buff {
            effect: StatusEffect::Weakened {
                attack_penalty: debuff_amount,
            },
            turns: duration,
        });
    }

    // Toxicity side effect
    if toxicity > config.toxicity_threshold {
        let side_damage = map_subrange_i32(
            toxicity,
            config.toxicity_threshold,
            1.0,
            config.side_effect_range.0,
            config.side_effect_range.1,
        );
        effects.push(AlchemyEffect::Damage {
            amount: side_damage,
            damage_type: DamageType::Poison,
        });
    }

    effects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genetics::{Dominance, Gene, PlantGenotype};
    use rand::SeedableRng;

    fn make_genotype_with_alchemy(
        healing: f32,
        potency: f32,
        stat_target: f32,
        duration: f32,
        toxicity: f32,
    ) -> PlantGenotype {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let mut g = PlantGenotype::random_wild(&mut rng);
        // Set alchemy genes to specific values (both alleles same for predictable expression)
        g.healing_affinity = Gene::new(healing, healing, Dominance::Incomplete);
        g.potency = Gene::new(potency, potency, Dominance::Incomplete);
        g.stat_target = Gene::new(stat_target, stat_target, Dominance::Incomplete);
        g.duration_gene = Gene::new(duration, duration, Dominance::Incomplete);
        g.toxicity = Gene::new(toxicity, toxicity, Dominance::Incomplete);
        g
    }

    #[test]
    fn test_high_healing_produces_heal_effect() {
        let g = make_genotype_with_alchemy(0.8, 0.5, 0.0, 0.5, 0.0);
        let effects = genetics_to_effects(&g);

        assert!(
            effects.iter().any(|e| matches!(e, AlchemyEffect::Heal { .. })),
            "Expected healing effect"
        );
    }

    #[test]
    fn test_low_healing_produces_damage_effect() {
        let g = make_genotype_with_alchemy(0.2, 0.5, 0.0, 0.5, 0.0);
        let effects = genetics_to_effects(&g);

        assert!(
            effects.iter().any(|e| matches!(e, AlchemyEffect::Damage { .. })),
            "Expected damage effect"
        );
    }

    #[test]
    fn test_high_potency_healing_adds_stat_boost() {
        let g = make_genotype_with_alchemy(0.8, 0.9, 0.0, 0.5, 0.0);
        let effects = genetics_to_effects(&g);

        assert!(
            effects.iter().any(|e| matches!(e, AlchemyEffect::StatBoost { .. })),
            "Expected stat boost from high potency healing"
        );
    }

    #[test]
    fn test_low_potency_healing_no_stat_boost() {
        let g = make_genotype_with_alchemy(0.8, 0.3, 0.0, 0.5, 0.0);
        let effects = genetics_to_effects(&g);

        assert!(
            !effects.iter().any(|e| matches!(e, AlchemyEffect::StatBoost { .. })),
            "Should not have stat boost with low potency"
        );
    }

    #[test]
    fn test_potency_scales_heal_amount() {
        let g_low = make_genotype_with_alchemy(0.8, 0.1, 0.0, 0.5, 0.0);
        let g_high = make_genotype_with_alchemy(0.8, 0.9, 0.0, 0.5, 0.0);

        let effects_low = genetics_to_effects(&g_low);
        let effects_high = genetics_to_effects(&g_high);

        let heal_low = effects_low.iter().find_map(|e| match e {
            AlchemyEffect::Heal { amount } => Some(*amount),
            _ => None,
        }).unwrap();

        let heal_high = effects_high.iter().find_map(|e| match e {
            AlchemyEffect::Heal { amount } => Some(*amount),
            _ => None,
        }).unwrap();

        assert!(heal_high > heal_low, "Higher potency should heal more");
    }

    #[test]
    fn test_stat_target_selects_correct_attribute() {
        // stat_target 0.0 → Strength (0.0 * 6 = 0)
        let g = make_genotype_with_alchemy(0.8, 0.9, 0.0, 0.5, 0.0);
        let effects = genetics_to_effects(&g);
        let boost = effects.iter().find(|e| matches!(e, AlchemyEffect::StatBoost { .. }));
        if let Some(AlchemyEffect::StatBoost { attribute, .. }) = boost {
            assert_eq!(*attribute, AttributeType::Strength);
        }

        // stat_target ~0.5 → Intelligence (0.5 * 6 = 3)
        let g2 = make_genotype_with_alchemy(0.8, 0.9, 0.5, 0.5, 0.0);
        let effects2 = genetics_to_effects(&g2);
        let boost2 = effects2.iter().find(|e| matches!(e, AlchemyEffect::StatBoost { .. }));
        if let Some(AlchemyEffect::StatBoost { attribute, .. }) = boost2 {
            assert_eq!(*attribute, AttributeType::Intelligence);
        }
    }

    #[test]
    fn test_duration_maps_to_turns() {
        let g = make_genotype_with_alchemy(0.8, 0.9, 0.0, 1.0, 0.0);
        let effects = genetics_to_effects(&g);

        let turns = effects.iter().find_map(|e| match e {
            AlchemyEffect::StatBoost { turns, .. } => Some(*turns),
            _ => None,
        }).unwrap();

        assert_eq!(turns, 5, "Max duration gene should map to 5 turns");
    }

    #[test]
    fn test_high_toxicity_adds_side_effect() {
        let g = make_genotype_with_alchemy(0.8, 0.5, 0.0, 0.5, 0.9);
        let effects = genetics_to_effects(&g);

        let damage_count = effects
            .iter()
            .filter(|e| matches!(e, AlchemyEffect::Damage { .. }))
            .count();

        assert!(damage_count > 0, "High toxicity should add side effect damage");
    }

    #[test]
    fn test_low_toxicity_no_side_effect() {
        let g = make_genotype_with_alchemy(0.8, 0.5, 0.0, 0.5, 0.2);
        let effects = genetics_to_effects(&g);

        let damage_count = effects
            .iter()
            .filter(|e| matches!(e, AlchemyEffect::Damage { .. }))
            .count();

        assert_eq!(damage_count, 0, "Low toxicity should not add side effects");
    }

    #[test]
    fn test_custom_config() {
        let config = StatMappingConfig {
            healing_threshold: 0.3, // Lower threshold
            heal_range: (10, 50),   // Stronger heals
            ..Default::default()
        };

        // With threshold 0.3, healing=0.4 should produce healing
        let g = make_genotype_with_alchemy(0.4, 0.5, 0.0, 0.5, 0.0);
        let effects = genetics_to_effects_with_config(&g, &config);

        assert!(effects.iter().any(|e| matches!(e, AlchemyEffect::Heal { .. })));
    }

    #[test]
    fn test_random_genotypes_produce_valid_effects() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        for _ in 0..100 {
            let g = PlantGenotype::random_wild(&mut rng);
            let effects = genetics_to_effects(&g);

            // Every plant should produce at least one effect
            assert!(!effects.is_empty());

            // Verify all amounts are positive
            for effect in &effects {
                match effect {
                    AlchemyEffect::Heal { amount } => assert!(*amount > 0),
                    AlchemyEffect::Damage { amount, .. } => assert!(*amount > 0),
                    AlchemyEffect::StatBoost { amount, turns, .. } => {
                        assert!(*amount > 0);
                        assert!(*turns > 0);
                    }
                    AlchemyEffect::Buff { turns, .. } => assert!(*turns > 0),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = StatMappingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: StatMappingConfig = serde_json::from_str(&json).unwrap();
        assert!((config.healing_threshold - deserialized.healing_threshold).abs() < f32::EPSILON);
    }
}
