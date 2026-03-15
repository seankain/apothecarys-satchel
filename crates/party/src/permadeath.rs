use apothecarys_inventory::container::Inventory;

use crate::generation::PartyMember;

/// Handle the death of a party member:
/// 1. Mark them as dead
/// 2. Transfer their equipment to the party inventory
///
/// Returns the list of transferred item names for UI notification.
pub fn handle_death(member: &mut PartyMember, party_inventory: &mut Inventory) -> Vec<String> {
    member.alive = false;
    member.derived.current_hp = 0;

    let items = member.equipment.unequip_all();
    let mut transferred = Vec::new();

    for item in items {
        let name = item.name.clone();
        let overflow = party_inventory.add_item(item, 1);
        if overflow == 0 {
            transferred.push(name);
        }
    }

    transferred
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::generate_party_member;
    use apothecarys_core::items::{EquipmentData, EquipmentSlot, Item, ItemType};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn test_handle_death_marks_dead() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut member = generate_party_member(&mut rng, 1);
        let mut inventory = Inventory::new(10);

        handle_death(&mut member, &mut inventory);

        assert!(!member.alive);
        assert_eq!(member.derived.current_hp, 0);
    }

    #[test]
    fn test_handle_death_transfers_equipment() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut member = generate_party_member(&mut rng, 1);
        member.equipment.weapon = Some(Item::new(
            "sword",
            "Iron Sword",
            ItemType::Equipment(EquipmentData {
                slot: EquipmentSlot::Weapon,
                armor_bonus: 0,
                attack_bonus: 2,
            }),
        ));
        member.equipment.armor = Some(Item::new(
            "chainmail",
            "Chainmail",
            ItemType::Equipment(EquipmentData {
                slot: EquipmentSlot::Armor,
                armor_bonus: 3,
                attack_bonus: 0,
            }),
        ));

        let mut inventory = Inventory::new(10);
        let transferred = handle_death(&mut member, &mut inventory);

        assert_eq!(transferred.len(), 2);
        assert!(transferred.contains(&"Iron Sword".to_string()));
        assert!(transferred.contains(&"Chainmail".to_string()));
        assert!(member.equipment.weapon.is_none());
        assert!(member.equipment.armor.is_none());
        assert_eq!(inventory.occupied_slots(), 2);
    }

    #[test]
    fn test_handle_death_full_inventory_drops() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut member = generate_party_member(&mut rng, 1);
        member.equipment.weapon = Some(Item::new(
            "sword",
            "Iron Sword",
            ItemType::Equipment(EquipmentData {
                slot: EquipmentSlot::Weapon,
                armor_bonus: 0,
                attack_bonus: 2,
            }),
        ));

        // Full inventory
        let mut inventory = Inventory::new(0);
        let transferred = handle_death(&mut member, &mut inventory);

        // Equipment was unequipped but couldn't be added to full inventory
        assert!(transferred.is_empty());
        assert!(!member.alive);
    }

    #[test]
    fn test_handle_death_no_equipment() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut member = generate_party_member(&mut rng, 1);
        let mut inventory = Inventory::new(10);

        let transferred = handle_death(&mut member, &mut inventory);
        assert!(transferred.is_empty());
        assert!(inventory.is_empty());
    }
}
