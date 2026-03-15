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

impl DerivedStats {
    /// Calculate derived stats from attributes, level, and class bonuses.
    pub fn calculate(
        attributes: &Attributes,
        level: u32,
        class_hp_bonus: i32,
        class_init_bonus: i32,
        equipment_ac: i32,
        proficiency: i32,
        use_dex_for_attack: bool,
        damage_dice: DamageDice,
    ) -> Self {
        let con_mod = attributes.constitution_mod();
        let dex_mod = attributes.dexterity_mod();
        let str_mod = attributes.strength_mod();

        let max_hp = 10 + con_mod * level as i32 + class_hp_bonus;
        let armor_class = 10 + dex_mod + equipment_ac;
        let initiative_bonus = dex_mod + class_init_bonus;
        let movement_speed = 5.0 + dex_mod as f32 * 0.5;
        let attack_mod = if use_dex_for_attack { dex_mod } else { str_mod };
        let attack_bonus = attack_mod + proficiency;

        Self {
            max_hp,
            current_hp: max_hp,
            armor_class,
            initiative_bonus,
            movement_speed,
            attack_bonus,
            damage_dice,
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
            3,                         // level
            2,                         // class_hp_bonus (e.g. warrior)
            0,                         // class_init_bonus
            3,                         // equipment_ac
            2,                         // proficiency
            false,                     // use STR for attack
            DamageDice::new(1, 8, 3),  // 1d8+3
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
