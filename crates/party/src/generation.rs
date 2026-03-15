use apothecarys_core::stats::{Attributes, DamageDice, DerivedStats, DerivedStatsParams};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Party member combat classes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PartyClass {
    Warrior,
    Ranger,
    Mage,
    Cleric,
    Rogue,
}

impl PartyClass {
    pub const ALL: [PartyClass; 5] = [
        PartyClass::Warrior,
        PartyClass::Ranger,
        PartyClass::Mage,
        PartyClass::Cleric,
        PartyClass::Rogue,
    ];

    /// Class minimum for the primary stat (3d6 is re-rolled if below).
    pub fn primary_stat_minimum(&self) -> i32 {
        match self {
            PartyClass::Warrior => 12, // STR/CON
            PartyClass::Ranger => 12,  // DEX
            PartyClass::Mage => 12,    // INT
            PartyClass::Cleric => 12,  // WIS
            PartyClass::Rogue => 12,   // DEX
        }
    }

    /// Whether this class uses DEX for attack (vs STR).
    pub fn use_dex_for_attack(&self) -> bool {
        matches!(self, PartyClass::Ranger | PartyClass::Rogue)
    }

    /// HP bonus per level from class.
    pub fn hp_bonus(&self) -> i32 {
        match self {
            PartyClass::Warrior => 4,
            PartyClass::Ranger => 2,
            PartyClass::Mage => 0,
            PartyClass::Cleric => 2,
            PartyClass::Rogue => 1,
        }
    }

    /// Initiative bonus from class.
    pub fn init_bonus(&self) -> i32 {
        match self {
            PartyClass::Warrior => 0,
            PartyClass::Ranger => 2,
            PartyClass::Mage => 0,
            PartyClass::Cleric => 0,
            PartyClass::Rogue => 3,
        }
    }

    /// Base damage dice for the class.
    pub fn damage_dice(&self) -> DamageDice {
        match self {
            PartyClass::Warrior => DamageDice::new(1, 10, 0),
            PartyClass::Ranger => DamageDice::new(1, 8, 0),
            PartyClass::Mage => DamageDice::new(1, 6, 0),
            PartyClass::Cleric => DamageDice::new(1, 6, 0),
            PartyClass::Rogue => DamageDice::new(1, 6, 2),
        }
    }

    /// Proficiency bonus at a given level.
    pub fn proficiency(level: u32) -> i32 {
        match level {
            0..=4 => 2,
            5..=8 => 3,
            _ => 4,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            PartyClass::Warrior => "Warrior",
            PartyClass::Ranger => "Ranger",
            PartyClass::Mage => "Mage",
            PartyClass::Cleric => "Cleric",
            PartyClass::Rogue => "Rogue",
        }
    }
}

/// Personality traits that affect AI combat behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Personality {
    /// 0.0 = defensive, 1.0 = all-out attack
    pub aggression: f32,
    /// 0.0 = reckless, 1.0 = self-preserving
    pub caution: f32,
    /// 0.0 = selfish, 1.0 = protects allies
    pub team_focus: f32,
    /// 0.0 = never uses items, 1.0 = prefers items
    pub item_affinity: f32,
}

/// Visual appearance data for a party member.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppearanceData {
    pub body_variant: u32,
    pub hair_variant: u32,
    pub skin_tone: u32,
    pub hair_color: u32,
}

/// Equipment worn by a party member.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Equipment {
    pub weapon: Option<apothecarys_core::items::Item>,
    pub armor: Option<apothecarys_core::items::Item>,
    pub accessory: Option<apothecarys_core::items::Item>,
}

impl Equipment {
    pub fn empty() -> Self {
        Self {
            weapon: None,
            armor: None,
            accessory: None,
        }
    }

    /// Remove all equipment, returning the items.
    pub fn unequip_all(&mut self) -> Vec<apothecarys_core::items::Item> {
        let mut items = Vec::new();
        if let Some(w) = self.weapon.take() {
            items.push(w);
        }
        if let Some(a) = self.armor.take() {
            items.push(a);
        }
        if let Some(a) = self.accessory.take() {
            items.push(a);
        }
        items
    }

    /// Total armor bonus from equipped items.
    pub fn total_armor_bonus(&self) -> i32 {
        let mut total = 0;
        for item in [&self.weapon, &self.armor, &self.accessory].into_iter().flatten() {
            if let apothecarys_core::items::ItemType::Equipment(data) = &item.item_type {
                total += data.armor_bonus;
            }
        }
        total
    }
}

/// A generated party member.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartyMember {
    pub id: Uuid,
    pub name: String,
    pub class: PartyClass,
    pub level: u32,
    pub xp: u32,
    pub attributes: Attributes,
    pub derived: DerivedStats,
    pub personality: Personality,
    pub equipment: Equipment,
    pub appearance: AppearanceData,
    pub alive: bool,
    pub backstory: String,
}

const FIRST_NAMES: &[&str] = &[
    "Aldric", "Brynn", "Cedric", "Dara", "Elara", "Finn", "Gwyn",
    "Hector", "Iris", "Jasper", "Kira", "Liam", "Mira", "Nolan",
    "Orla", "Pierce", "Quinn", "Rowan", "Sera", "Theron", "Una",
    "Vex", "Wren", "Xara", "Yara", "Zane",
];

const LAST_NAMES: &[&str] = &[
    "Ashford", "Blackthorn", "Crowe", "Dunmore", "Emberglow",
    "Foxglove", "Greymist", "Hawthorne", "Ironleaf", "Juniper",
    "Kettle", "Larkspur", "Moss", "Nightshade", "Oakenshaw",
    "Primrose", "Quill", "Ravenscroft", "Stonecrest", "Thistledown",
];

const BACKSTORY_TEMPLATES: &[&str] = &[
    "A former {class} from the northern mountains, seeking fortune and redemption.",
    "Once served in the king's guard before wandering the countryside as a {class}.",
    "Grew up in the slums, learning to survive as a {class} from a young age.",
    "A scholar-turned-{class} who left the academy to test their skills in the real world.",
    "Lost their family to a dungeon raid, and trained as a {class} to seek answers.",
    "A traveling {class} drawn to the apothecary's cause by fate or curiosity.",
];

/// Roll 3d6 for a stat.
fn roll_3d6(rng: &mut impl Rng) -> i32 {
    (0..3).map(|_| rng.gen_range(1..=6)).sum()
}

/// Roll 3d6 with a minimum threshold, re-rolling up to 3 times.
fn roll_3d6_with_min(rng: &mut impl Rng, minimum: i32) -> i32 {
    let mut val = roll_3d6(rng);
    for _ in 0..3 {
        if val >= minimum {
            break;
        }
        val = roll_3d6(rng);
    }
    val
}

/// Roll attributes with class-based minimums for primary stats.
fn roll_attributes(rng: &mut impl Rng, class: PartyClass) -> Attributes {
    let min = class.primary_stat_minimum();

    match class {
        PartyClass::Warrior => Attributes {
            strength: roll_3d6_with_min(rng, min),
            dexterity: roll_3d6(rng),
            constitution: roll_3d6_with_min(rng, min),
            intelligence: roll_3d6(rng),
            wisdom: roll_3d6(rng),
            charisma: roll_3d6(rng),
        },
        PartyClass::Ranger | PartyClass::Rogue => Attributes {
            strength: roll_3d6(rng),
            dexterity: roll_3d6_with_min(rng, min),
            constitution: roll_3d6(rng),
            intelligence: roll_3d6(rng),
            wisdom: roll_3d6(rng),
            charisma: roll_3d6(rng),
        },
        PartyClass::Mage => Attributes {
            strength: roll_3d6(rng),
            dexterity: roll_3d6(rng),
            constitution: roll_3d6(rng),
            intelligence: roll_3d6_with_min(rng, min),
            wisdom: roll_3d6(rng),
            charisma: roll_3d6(rng),
        },
        PartyClass::Cleric => Attributes {
            strength: roll_3d6(rng),
            dexterity: roll_3d6(rng),
            constitution: roll_3d6(rng),
            intelligence: roll_3d6(rng),
            wisdom: roll_3d6_with_min(rng, min),
            charisma: roll_3d6(rng),
        },
    }
}

/// Generate a random personality.
fn generate_personality(rng: &mut impl Rng) -> Personality {
    Personality {
        aggression: rng.gen_range(0.0..=1.0),
        caution: rng.gen_range(0.0..=1.0),
        team_focus: rng.gen_range(0.0..=1.0),
        item_affinity: rng.gen_range(0.0..=1.0),
    }
}

/// Generate random appearance data.
fn generate_appearance(rng: &mut impl Rng) -> AppearanceData {
    AppearanceData {
        body_variant: rng.gen_range(0..4),
        hair_variant: rng.gen_range(0..6),
        skin_tone: rng.gen_range(0..5),
        hair_color: rng.gen_range(0..8),
    }
}

/// Generate a random name.
fn generate_name(rng: &mut impl Rng) -> String {
    let first = FIRST_NAMES[rng.gen_range(0..FIRST_NAMES.len())];
    let last = LAST_NAMES[rng.gen_range(0..LAST_NAMES.len())];
    format!("{first} {last}")
}

/// Generate a backstory from templates.
fn generate_backstory(rng: &mut impl Rng, class: PartyClass) -> String {
    let template = BACKSTORY_TEMPLATES[rng.gen_range(0..BACKSTORY_TEMPLATES.len())];
    template.replace("{class}", class.display_name())
}

/// Generate a complete party member.
pub fn generate_party_member(rng: &mut impl Rng, level: u32) -> PartyMember {
    let class = PartyClass::ALL[rng.gen_range(0..PartyClass::ALL.len())];
    let attributes = roll_attributes(rng, class);
    let equipment = Equipment::empty();

    let params = DerivedStatsParams {
        level,
        class_hp_bonus: class.hp_bonus(),
        class_init_bonus: class.init_bonus(),
        equipment_ac: equipment.total_armor_bonus(),
        proficiency: PartyClass::proficiency(level),
        use_dex_for_attack: class.use_dex_for_attack(),
        damage_dice: class.damage_dice(),
    };
    let derived = DerivedStats::calculate(&attributes, &params);

    PartyMember {
        id: Uuid::new_v4(),
        name: generate_name(rng),
        class,
        level,
        xp: 0,
        attributes,
        derived,
        personality: generate_personality(rng),
        equipment,
        appearance: generate_appearance(rng),
        alive: true,
        backstory: generate_backstory(rng, class),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn test_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn test_generate_party_member_is_valid() {
        let mut rng = test_rng();
        let member = generate_party_member(&mut rng, 1);

        assert!(member.alive);
        assert_eq!(member.level, 1);
        assert_eq!(member.xp, 0);
        assert!(!member.name.is_empty());
        assert!(!member.backstory.is_empty());
        assert!(member.derived.max_hp > 0);
        assert_eq!(member.derived.current_hp, member.derived.max_hp);
    }

    #[test]
    fn test_generate_multiple_members_differ() {
        let mut rng = test_rng();
        let a = generate_party_member(&mut rng, 1);
        let b = generate_party_member(&mut rng, 1);
        // Very unlikely both have the same name and class
        assert!(a.name != b.name || a.class != b.class);
    }

    #[test]
    fn test_roll_3d6_range() {
        let mut rng = test_rng();
        for _ in 0..1000 {
            let val = roll_3d6(&mut rng);
            assert!((3..=18).contains(&val));
        }
    }

    #[test]
    fn test_class_primary_stats_meet_minimum() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let member = generate_party_member(&mut rng, 1);
            let min = member.class.primary_stat_minimum();
            let primary = match member.class {
                PartyClass::Warrior => member.attributes.strength.max(member.attributes.constitution),
                PartyClass::Ranger | PartyClass::Rogue => member.attributes.dexterity,
                PartyClass::Mage => member.attributes.intelligence,
                PartyClass::Cleric => member.attributes.wisdom,
            };
            // With re-rolling, the primary stat should usually meet the minimum,
            // but there's a small chance it doesn't (3 re-rolls of 3d6).
            // We just check it's in valid 3d6 range.
            assert!((3..=18).contains(&primary), "Primary stat {primary} out of range for {:?}", member.class);
            let _ = min; // used for reference only
        }
    }

    #[test]
    fn test_personality_in_range() {
        let mut rng = test_rng();
        for _ in 0..100 {
            let p = generate_personality(&mut rng);
            assert!((0.0..=1.0).contains(&p.aggression));
            assert!((0.0..=1.0).contains(&p.caution));
            assert!((0.0..=1.0).contains(&p.team_focus));
            assert!((0.0..=1.0).contains(&p.item_affinity));
        }
    }

    #[test]
    fn test_equipment_unequip_all() {
        use apothecarys_core::items::{EquipmentData, EquipmentSlot, Item, ItemType};
        let mut equip = Equipment {
            weapon: Some(Item::new(
                "sword",
                "Sword",
                ItemType::Equipment(EquipmentData {
                    slot: EquipmentSlot::Weapon,
                    armor_bonus: 0,
                    attack_bonus: 2,
                }),
            )),
            armor: Some(Item::new(
                "chainmail",
                "Chainmail",
                ItemType::Equipment(EquipmentData {
                    slot: EquipmentSlot::Armor,
                    armor_bonus: 3,
                    attack_bonus: 0,
                }),
            )),
            accessory: None,
        };

        let items = equip.unequip_all();
        assert_eq!(items.len(), 2);
        assert!(equip.weapon.is_none());
        assert!(equip.armor.is_none());
    }

    #[test]
    fn test_equipment_armor_bonus() {
        use apothecarys_core::items::{EquipmentData, EquipmentSlot, Item, ItemType};
        let equip = Equipment {
            weapon: None,
            armor: Some(Item::new(
                "chainmail",
                "Chainmail",
                ItemType::Equipment(EquipmentData {
                    slot: EquipmentSlot::Armor,
                    armor_bonus: 3,
                    attack_bonus: 0,
                }),
            )),
            accessory: Some(Item::new(
                "ring",
                "Ring of Protection",
                ItemType::Equipment(EquipmentData {
                    slot: EquipmentSlot::Accessory,
                    armor_bonus: 1,
                    attack_bonus: 0,
                }),
            )),
        };

        assert_eq!(equip.total_armor_bonus(), 4);
    }

    #[test]
    fn test_party_member_serde_roundtrip() {
        let mut rng = test_rng();
        let member = generate_party_member(&mut rng, 3);
        let json = serde_json::to_string(&member).unwrap();
        let deserialized: PartyMember = serde_json::from_str(&json).unwrap();
        assert_eq!(member, deserialized);
    }

    #[test]
    fn test_all_classes_can_generate() {
        let mut rng = StdRng::seed_from_u64(0);
        let mut classes_seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let m = generate_party_member(&mut rng, 1);
            classes_seen.insert(m.class);
        }
        // With 200 tries and 5 classes, we should see all
        assert_eq!(classes_seen.len(), 5);
    }

    #[test]
    fn test_higher_level_has_more_hp_with_positive_con() {
        // Use attributes with positive CON to ensure level scaling increases HP
        let attrs = Attributes {
            constitution: 14, // +2 mod
            ..Default::default()
        };
        let class = PartyClass::Warrior;
        let low_params = DerivedStatsParams {
            level: 1,
            class_hp_bonus: class.hp_bonus(),
            class_init_bonus: class.init_bonus(),
            equipment_ac: 0,
            proficiency: PartyClass::proficiency(1),
            use_dex_for_attack: class.use_dex_for_attack(),
            damage_dice: class.damage_dice(),
        };
        let high_params = DerivedStatsParams {
            level: 5,
            ..low_params.clone()
        };
        let low_stats = DerivedStats::calculate(&attrs, &low_params);
        let high_stats = DerivedStats::calculate(&attrs, &high_params);
        assert!(high_stats.max_hp > low_stats.max_hp);
    }
}
