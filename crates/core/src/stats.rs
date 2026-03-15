use serde::{Deserialize, Serialize};

/// The six core attributes, analogous to D&D ability scores.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attributes {
    /// Melee damage, carry capacity
    pub strength: i32,
    /// Initiative, dodge, ranged accuracy
    pub dexterity: i32,
    /// HP pool, poison/disease resistance
    pub constitution: i32,
    /// Magic damage, crafting insight (apothecary only)
    pub intelligence: i32,
    /// Perception, status resist, AI decision quality
    pub wisdom: i32,
    /// Recruitment cost, NPC prices, party morale
    pub charisma: i32,
}

impl Attributes {
    /// Standard D&D modifier formula: (stat - 10) / 2, rounding down (floor division).
    pub fn modifier(stat: i32) -> i32 {
        let diff = stat - 10;
        if diff >= 0 {
            diff / 2
        } else {
            (diff - 1) / 2
        }
    }

    pub fn strength_mod(&self) -> i32 {
        Self::modifier(self.strength)
    }

    pub fn dexterity_mod(&self) -> i32 {
        Self::modifier(self.dexterity)
    }

    pub fn constitution_mod(&self) -> i32 {
        Self::modifier(self.constitution)
    }

    pub fn intelligence_mod(&self) -> i32 {
        Self::modifier(self.intelligence)
    }

    pub fn wisdom_mod(&self) -> i32 {
        Self::modifier(self.wisdom)
    }

    pub fn charisma_mod(&self) -> i32 {
        Self::modifier(self.charisma)
    }
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            strength: 10,
            dexterity: 10,
            constitution: 10,
            intelligence: 10,
            wisdom: 10,
            charisma: 10,
        }
    }
}

/// Dice specification for damage rolls.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DamageDice {
    /// Number of dice to roll
    pub count: u32,
    /// Number of sides per die (d4, d6, d8, d10, d12, d20)
    pub sides: u32,
    /// Flat modifier added to the roll
    pub bonus: i32,
}

impl DamageDice {
    pub fn new(count: u32, sides: u32, bonus: i32) -> Self {
        Self {
            count,
            sides,
            bonus,
        }
    }
}

impl Default for DamageDice {
    fn default() -> Self {
        Self {
            count: 1,
            sides: 6,
            bonus: 0,
        }
    }
}

/// Stats derived from attributes, level, and class.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DerivedStats {
    /// 10 + CON_mod * level + class_bonus
    pub max_hp: i32,
    pub current_hp: i32,
    /// 10 + DEX_mod + equipment
    pub armor_class: i32,
    /// DEX_mod + class_bonus
    pub initiative_bonus: i32,
    /// Base 5.0 + DEX_mod * 0.5
    pub movement_speed: f32,
    /// STR_mod or DEX_mod + proficiency
    pub attack_bonus: i32,
    pub damage_dice: DamageDice,
}

/// Parameters for calculating derived stats from attributes and class/equipment data.
#[derive(Debug, Clone)]
pub struct DerivedStatsParams {
    pub level: u32,
    pub class_hp_bonus: i32,
    pub class_init_bonus: i32,
    pub equipment_ac: i32,
    pub proficiency: i32,
    pub use_dex_for_attack: bool,
    pub damage_dice: DamageDice,
}

impl DerivedStats {
    /// Calculate derived stats from attributes and class/equipment parameters.
    pub fn calculate(attributes: &Attributes, params: &DerivedStatsParams) -> Self {
        let con_mod = attributes.constitution_mod();
        let dex_mod = attributes.dexterity_mod();
        let str_mod = attributes.strength_mod();

        let max_hp = 10 + con_mod * params.level as i32 + params.class_hp_bonus;
        let armor_class = 10 + dex_mod + params.equipment_ac;
        let initiative_bonus = dex_mod + params.class_init_bonus;
        let movement_speed = 5.0 + dex_mod as f32 * 0.5;
        let attack_mod = if params.use_dex_for_attack { dex_mod } else { str_mod };
        let attack_bonus = attack_mod + params.proficiency;

        Self {
            max_hp,
            current_hp: max_hp,
            armor_class,
            initiative_bonus,
            movement_speed,
            attack_bonus,
            damage_dice: params.damage_dice.clone(),
        }
    }
}

impl Default for DerivedStats {
    fn default() -> Self {
        Self {
            max_hp: 10,
            current_hp: 10,
            armor_class: 10,
            initiative_bonus: 0,
            movement_speed: 5.0,
            attack_bonus: 0,
            damage_dice: DamageDice::default(),
        }
    }
}

/// Which attribute a status effect targets.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AttributeType {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
}

/// Damage types for resistance/vulnerability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DamageType {
    Physical,
    Fire,
    Ice,
    Poison,
    Holy,
    Arcane,
}

/// All possible status effect types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StatusEffect {
    // Buffs
    AttackUp { amount: i32 },
    DefenseUp { amount: i32 },
    Regeneration { hp_per_turn: i32 },
    Haste,
    Resistance { damage_type: DamageType },

    // Debuffs
    Poisoned { damage_per_turn: i32 },
    Weakened { attack_penalty: i32 },
    Slowed,
    Stunned,
    Blinded,

    // Special
    StatBoost { attribute: AttributeType, amount: i32 },
}

/// An active status effect with remaining duration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveStatusEffect {
    pub effect: StatusEffect,
    pub remaining_turns: u32,
}

impl ActiveStatusEffect {
    pub fn new(effect: StatusEffect, turns: u32) -> Self {
        Self {
            effect,
            remaining_turns: turns,
        }
    }

    /// Tick the effect: decrement duration, return whether it expired.
    pub fn tick(&mut self) -> bool {
        self.remaining_turns = self.remaining_turns.saturating_sub(1);
        self.remaining_turns == 0
    }

    /// Returns true if this effect is the same type as another (for stacking rules).
    pub fn same_type(&self, other: &StatusEffect) -> bool {
        std::mem::discriminant(&self.effect) == std::mem::discriminant(other)
    }
}

/// Manages a collection of active status effects on a combatant.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StatusEffectTracker {
    pub effects: Vec<ActiveStatusEffect>,
}

impl StatusEffectTracker {
    pub fn new() -> Self {
        Self {
            effects: Vec::new(),
        }
    }

    /// Apply a status effect. If the same type already exists, refresh its duration
    /// (stacking rule: same effect type refreshes duration, doesn't double-stack amount).
    pub fn apply(&mut self, effect: StatusEffect, turns: u32) {
        if let Some(existing) = self.effects.iter_mut().find(|e| e.same_type(&effect)) {
            existing.remaining_turns = turns;
            existing.effect = effect;
        } else {
            self.effects.push(ActiveStatusEffect::new(effect, turns));
        }
    }

    /// Remove all effects of the given type.
    pub fn remove(&mut self, effect: &StatusEffect) {
        self.effects.retain(|e| !e.same_type(effect));
    }

    /// Tick all effects, removing expired ones. Returns per-turn effects to apply.
    pub fn tick(&mut self) -> Vec<TickResult> {
        let mut results = Vec::new();

        for active in &self.effects {
            match &active.effect {
                StatusEffect::Regeneration { hp_per_turn } => {
                    results.push(TickResult::Heal(*hp_per_turn));
                }
                StatusEffect::Poisoned { damage_per_turn } => {
                    results.push(TickResult::Damage(*damage_per_turn));
                }
                _ => {}
            }
        }

        self.effects.retain_mut(|e| !e.tick());

        results
    }

    /// Calculate the total attack bonus modifier from active effects.
    pub fn attack_modifier(&self) -> i32 {
        let mut modifier = 0;
        for active in &self.effects {
            match &active.effect {
                StatusEffect::AttackUp { amount } => modifier += amount,
                StatusEffect::Weakened { attack_penalty } => modifier -= attack_penalty,
                _ => {}
            }
        }
        modifier
    }

    /// Calculate the total AC modifier from active effects.
    pub fn defense_modifier(&self) -> i32 {
        let mut modifier = 0;
        for active in &self.effects {
            if let StatusEffect::DefenseUp { amount } = &active.effect {
                modifier += amount;
            }
        }
        modifier
    }

    /// Check if stunned (cannot act).
    pub fn is_stunned(&self) -> bool {
        self.effects
            .iter()
            .any(|e| matches!(e.effect, StatusEffect::Stunned))
    }

    /// Check if slowed.
    pub fn is_slowed(&self) -> bool {
        self.effects
            .iter()
            .any(|e| matches!(e.effect, StatusEffect::Slowed))
    }

    /// Check if blinded.
    pub fn is_blinded(&self) -> bool {
        self.effects
            .iter()
            .any(|e| matches!(e.effect, StatusEffect::Blinded))
    }

    /// Check if hasted (gets an extra action).
    pub fn is_hasted(&self) -> bool {
        self.effects
            .iter()
            .any(|e| matches!(e.effect, StatusEffect::Haste))
    }

    /// Check if resistant to a specific damage type.
    pub fn has_resistance(&self, damage_type: DamageType) -> bool {
        self.effects.iter().any(|e| {
            matches!(&e.effect, StatusEffect::Resistance { damage_type: dt } if *dt == damage_type)
        })
    }

    /// Get the total stat boost for a specific attribute from active effects.
    pub fn stat_boost(&self, attribute: AttributeType) -> i32 {
        self.effects
            .iter()
            .filter_map(|e| match &e.effect {
                StatusEffect::StatBoost { attribute: attr, amount } if *attr == attribute => {
                    Some(*amount)
                }
                _ => None,
            })
            .sum()
    }
}

/// A combatant in the game — ties together attributes, derived stats, and status effects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Combatant {
    pub name: String,
    pub level: u32,
    pub attributes: Attributes,
    pub base_stats: DerivedStats,
    pub status_effects: StatusEffectTracker,
}

impl Combatant {
    pub fn new(name: impl Into<String>, level: u32, attributes: Attributes, base_stats: DerivedStats) -> Self {
        Self {
            name: name.into(),
            level,
            attributes,
            base_stats,
            status_effects: StatusEffectTracker::new(),
        }
    }

    /// Get effective attack bonus including status effect modifiers.
    pub fn effective_attack_bonus(&self) -> i32 {
        self.base_stats.attack_bonus + self.status_effects.attack_modifier()
    }

    /// Get effective armor class including status effect modifiers.
    pub fn effective_armor_class(&self) -> i32 {
        self.base_stats.armor_class + self.status_effects.defense_modifier()
    }

    /// Get effective movement speed (halved if slowed).
    pub fn effective_movement_speed(&self) -> f32 {
        if self.status_effects.is_slowed() {
            self.base_stats.movement_speed * 0.5
        } else {
            self.base_stats.movement_speed
        }
    }

    /// Get an effective attribute value including stat boosts.
    pub fn effective_attribute(&self, attr: AttributeType) -> i32 {
        let base = match attr {
            AttributeType::Strength => self.attributes.strength,
            AttributeType::Dexterity => self.attributes.dexterity,
            AttributeType::Constitution => self.attributes.constitution,
            AttributeType::Intelligence => self.attributes.intelligence,
            AttributeType::Wisdom => self.attributes.wisdom,
            AttributeType::Charisma => self.attributes.charisma,
        };
        base + self.status_effects.stat_boost(attr)
    }

    /// Apply damage to this combatant. Returns actual damage dealt.
    pub fn take_damage(&mut self, amount: i32, damage_type: DamageType) -> i32 {
        let actual = if self.status_effects.has_resistance(damage_type) {
            amount / 2
        } else {
            amount
        };
        self.base_stats.current_hp -= actual;
        actual
    }

    /// Heal this combatant. Cannot exceed max HP.
    pub fn heal(&mut self, amount: i32) {
        self.base_stats.current_hp = (self.base_stats.current_hp + amount).min(self.base_stats.max_hp);
    }

    /// Returns true if HP <= 0.
    pub fn is_dead(&self) -> bool {
        self.base_stats.current_hp <= 0
    }

    /// Tick status effects, applying per-turn results (regen/poison).
    pub fn tick_effects(&mut self) -> Vec<TickResult> {
        let results = self.status_effects.tick();
        for result in &results {
            match result {
                TickResult::Heal(amount) => self.heal(*amount),
                TickResult::Damage(amount) => {
                    self.base_stats.current_hp -= amount;
                }
            }
        }
        results
    }
}

/// Result from ticking status effects each turn.
#[derive(Debug, Clone, PartialEq)]
pub enum TickResult {
    Heal(i32),
    Damage(i32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifier_calculation() {
        assert_eq!(Attributes::modifier(10), 0);
        assert_eq!(Attributes::modifier(11), 0);
        assert_eq!(Attributes::modifier(12), 1);
        assert_eq!(Attributes::modifier(14), 2);
        assert_eq!(Attributes::modifier(8), -1);
        assert_eq!(Attributes::modifier(6), -2);
        assert_eq!(Attributes::modifier(20), 5);
        assert_eq!(Attributes::modifier(3), -4); // 3d6 minimum
    }

    #[test]
    fn test_derived_stats_calculation() {
        let attrs = Attributes {
            strength: 16,  // +3
            dexterity: 14, // +2
            constitution: 14, // +2
            intelligence: 10,
            wisdom: 12,
            charisma: 8,
        };

        let stats = DerivedStats::calculate(
            &attrs,
            &DerivedStatsParams {
                level: 3,
                class_hp_bonus: 2,
                class_init_bonus: 0,
                equipment_ac: 3,
                proficiency: 2,
                use_dex_for_attack: false,
                damage_dice: DamageDice::new(1, 8, 3),
            },
        );

        // max_hp = 10 + 2 * 3 + 2 = 18
        assert_eq!(stats.max_hp, 18);
        assert_eq!(stats.current_hp, 18);
        // armor_class = 10 + 2 + 3 = 15
        assert_eq!(stats.armor_class, 15);
        // initiative = 2 + 0 = 2
        assert_eq!(stats.initiative_bonus, 2);
        // movement = 5.0 + 2 * 0.5 = 6.0
        assert!((stats.movement_speed - 6.0).abs() < f32::EPSILON);
        // attack = 3 (STR) + 2 (prof) = 5
        assert_eq!(stats.attack_bonus, 5);
    }

    #[test]
    fn test_status_effect_apply_and_stack() {
        let mut tracker = StatusEffectTracker::new();

        tracker.apply(StatusEffect::AttackUp { amount: 2 }, 3);
        assert_eq!(tracker.effects.len(), 1);
        assert_eq!(tracker.attack_modifier(), 2);

        // Same type should refresh, not stack
        tracker.apply(StatusEffect::AttackUp { amount: 3 }, 5);
        assert_eq!(tracker.effects.len(), 1);
        assert_eq!(tracker.attack_modifier(), 3);
        assert_eq!(tracker.effects[0].remaining_turns, 5);
    }

    #[test]
    fn test_status_effect_tick_and_expire() {
        let mut tracker = StatusEffectTracker::new();
        tracker.apply(StatusEffect::Regeneration { hp_per_turn: 5 }, 2);
        tracker.apply(StatusEffect::Poisoned { damage_per_turn: 3 }, 1);

        let results = tracker.tick();
        assert_eq!(results.len(), 2);
        assert!(results.contains(&TickResult::Heal(5)));
        assert!(results.contains(&TickResult::Damage(3)));
        // Poison expired (had 1 turn), regen has 1 left
        assert_eq!(tracker.effects.len(), 1);

        let results = tracker.tick();
        assert_eq!(results.len(), 1);
        assert!(results.contains(&TickResult::Heal(5)));
        // Regen now expired too
        assert_eq!(tracker.effects.len(), 0);
    }

    #[test]
    fn test_status_effect_modifiers() {
        let mut tracker = StatusEffectTracker::new();
        tracker.apply(StatusEffect::AttackUp { amount: 2 }, 3);
        tracker.apply(StatusEffect::Weakened { attack_penalty: 1 }, 2);
        tracker.apply(StatusEffect::DefenseUp { amount: 3 }, 3);

        assert_eq!(tracker.attack_modifier(), 1); // +2 - 1
        assert_eq!(tracker.defense_modifier(), 3);
    }

    #[test]
    fn test_status_effect_condition_checks() {
        let mut tracker = StatusEffectTracker::new();
        assert!(!tracker.is_stunned());

        tracker.apply(StatusEffect::Stunned, 2);
        tracker.apply(StatusEffect::Blinded, 1);
        assert!(tracker.is_stunned());
        assert!(tracker.is_blinded());
        assert!(!tracker.is_slowed());
    }

    #[test]
    fn test_status_effect_remove() {
        let mut tracker = StatusEffectTracker::new();
        tracker.apply(StatusEffect::Poisoned { damage_per_turn: 5 }, 3);
        tracker.apply(StatusEffect::Stunned, 2);
        assert_eq!(tracker.effects.len(), 2);

        tracker.remove(&StatusEffect::Poisoned { damage_per_turn: 0 });
        assert_eq!(tracker.effects.len(), 1);
        assert!(tracker.is_stunned());
    }

    #[test]
    fn test_attributes_serde_roundtrip() {
        let attrs = Attributes {
            strength: 16,
            dexterity: 14,
            constitution: 12,
            intelligence: 10,
            wisdom: 8,
            charisma: 6,
        };
        let json = serde_json::to_string(&attrs).unwrap();
        let deserialized: Attributes = serde_json::from_str(&json).unwrap();
        assert_eq!(attrs, deserialized);
    }

    #[test]
    fn test_derived_stats_serde_roundtrip() {
        let stats = DerivedStats {
            max_hp: 25,
            current_hp: 18,
            armor_class: 15,
            initiative_bonus: 3,
            movement_speed: 6.0,
            attack_bonus: 5,
            damage_dice: DamageDice::new(2, 6, 3),
        };
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: DerivedStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    #[test]
    fn test_has_resistance() {
        let mut tracker = StatusEffectTracker::new();
        assert!(!tracker.has_resistance(DamageType::Fire));

        tracker.apply(StatusEffect::Resistance { damage_type: DamageType::Fire }, 3);
        assert!(tracker.has_resistance(DamageType::Fire));
        assert!(!tracker.has_resistance(DamageType::Ice));
    }

    #[test]
    fn test_is_hasted() {
        let mut tracker = StatusEffectTracker::new();
        assert!(!tracker.is_hasted());
        tracker.apply(StatusEffect::Haste, 2);
        assert!(tracker.is_hasted());
    }

    #[test]
    fn test_stat_boost() {
        let mut tracker = StatusEffectTracker::new();
        tracker.apply(
            StatusEffect::StatBoost { attribute: AttributeType::Strength, amount: 4 },
            3,
        );
        assert_eq!(tracker.stat_boost(AttributeType::Strength), 4);
        assert_eq!(tracker.stat_boost(AttributeType::Dexterity), 0);
    }

    #[test]
    fn test_combatant_effective_stats() {
        let attrs = Attributes {
            strength: 16,
            dexterity: 14,
            constitution: 14,
            intelligence: 10,
            wisdom: 12,
            charisma: 8,
        };
        let stats = DerivedStats::calculate(&attrs, &DerivedStatsParams {
            level: 3, class_hp_bonus: 2, class_init_bonus: 0, equipment_ac: 3,
            proficiency: 2, use_dex_for_attack: false, damage_dice: DamageDice::new(1, 8, 3),
        });
        let mut combatant = Combatant::new("Test Fighter", 3, attrs, stats);

        assert_eq!(combatant.effective_attack_bonus(), 5);
        assert_eq!(combatant.effective_armor_class(), 15);
        assert!((combatant.effective_movement_speed() - 6.0).abs() < f32::EPSILON);

        combatant.status_effects.apply(StatusEffect::AttackUp { amount: 2 }, 3);
        combatant.status_effects.apply(StatusEffect::DefenseUp { amount: 1 }, 3);
        combatant.status_effects.apply(StatusEffect::Slowed, 2);

        assert_eq!(combatant.effective_attack_bonus(), 7);
        assert_eq!(combatant.effective_armor_class(), 16);
        assert!((combatant.effective_movement_speed() - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_combatant_damage_and_healing() {
        let attrs = Attributes::default();
        let stats = DerivedStats { max_hp: 20, current_hp: 20, ..Default::default() };
        let mut combatant = Combatant::new("Test", 1, attrs, stats);

        let dealt = combatant.take_damage(8, DamageType::Physical);
        assert_eq!(dealt, 8);
        assert_eq!(combatant.base_stats.current_hp, 12);

        combatant.heal(5);
        assert_eq!(combatant.base_stats.current_hp, 17);

        // Healing cannot exceed max
        combatant.heal(100);
        assert_eq!(combatant.base_stats.current_hp, 20);
    }

    #[test]
    fn test_combatant_resistance_halves_damage() {
        let attrs = Attributes::default();
        let stats = DerivedStats { max_hp: 20, current_hp: 20, ..Default::default() };
        let mut combatant = Combatant::new("Test", 1, attrs, stats);
        combatant.status_effects.apply(StatusEffect::Resistance { damage_type: DamageType::Fire }, 5);

        let dealt = combatant.take_damage(10, DamageType::Fire);
        assert_eq!(dealt, 5);
        assert_eq!(combatant.base_stats.current_hp, 15);

        // Non-resisted damage is full
        let dealt = combatant.take_damage(10, DamageType::Ice);
        assert_eq!(dealt, 10);
        assert_eq!(combatant.base_stats.current_hp, 5);
    }

    #[test]
    fn test_combatant_tick_effects() {
        let attrs = Attributes::default();
        let stats = DerivedStats { max_hp: 30, current_hp: 20, ..Default::default() };
        let mut combatant = Combatant::new("Test", 1, attrs, stats);

        combatant.status_effects.apply(StatusEffect::Regeneration { hp_per_turn: 3 }, 2);
        combatant.status_effects.apply(StatusEffect::Poisoned { damage_per_turn: 2 }, 1);

        let results = combatant.tick_effects();
        assert_eq!(results.len(), 2);
        // +3 heal, -2 poison = net +1
        assert_eq!(combatant.base_stats.current_hp, 21);
    }

    #[test]
    fn test_combatant_is_dead() {
        let attrs = Attributes::default();
        let stats = DerivedStats { max_hp: 10, current_hp: 5, ..Default::default() };
        let mut combatant = Combatant::new("Test", 1, attrs, stats);

        assert!(!combatant.is_dead());
        combatant.take_damage(5, DamageType::Physical);
        assert!(combatant.is_dead());
    }

    #[test]
    fn test_combatant_effective_attribute() {
        let attrs = Attributes { strength: 14, ..Default::default() };
        let stats = DerivedStats::default();
        let mut combatant = Combatant::new("Test", 1, attrs, stats);

        assert_eq!(combatant.effective_attribute(AttributeType::Strength), 14);
        combatant.status_effects.apply(
            StatusEffect::StatBoost { attribute: AttributeType::Strength, amount: 4 },
            3,
        );
        assert_eq!(combatant.effective_attribute(AttributeType::Strength), 18);
    }

    #[test]
    fn test_combatant_serde_roundtrip() {
        let attrs = Attributes { strength: 16, ..Default::default() };
        let stats = DerivedStats { max_hp: 20, current_hp: 15, ..Default::default() };
        let mut combatant = Combatant::new("Warrior", 3, attrs, stats);
        combatant.status_effects.apply(StatusEffect::AttackUp { amount: 2 }, 3);

        let json = serde_json::to_string(&combatant).unwrap();
        let deserialized: Combatant = serde_json::from_str(&json).unwrap();
        assert_eq!(combatant, deserialized);
    }

    #[test]
    fn test_status_effect_serde_roundtrip() {
        let mut tracker = StatusEffectTracker::new();
        tracker.apply(StatusEffect::AttackUp { amount: 2 }, 3);
        tracker.apply(StatusEffect::Poisoned { damage_per_turn: 5 }, 2);
        tracker.apply(
            StatusEffect::StatBoost {
                attribute: AttributeType::Strength,
                amount: 4,
            },
            5,
        );

        let json = serde_json::to_string(&tracker).unwrap();
        let deserialized: StatusEffectTracker = serde_json::from_str(&json).unwrap();
        assert_eq!(tracker, deserialized);
    }
}
