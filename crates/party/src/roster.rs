use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::generation::PartyMember;

/// Maximum number of active party members (excluding the player).
pub const MAX_ACTIVE_PARTY: usize = 4;

/// Manages the active party roster.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Roster {
    pub members: Vec<PartyMember>,
}

impl Roster {
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    /// Add a member to the active roster. Returns `Err` with the member back
    /// if the roster is full.
    pub fn add_member(&mut self, member: PartyMember) -> Result<(), Box<PartyMember>> {
        if self.members.len() >= MAX_ACTIVE_PARTY {
            return Err(Box::new(member));
        }
        self.members.push(member);
        Ok(())
    }

    /// Dismiss a member by ID. Returns the dismissed member, or `None`.
    pub fn dismiss(&mut self, id: Uuid) -> Option<PartyMember> {
        if let Some(idx) = self.members.iter().position(|m| m.id == id) {
            Some(self.members.remove(idx))
        } else {
            None
        }
    }

    /// Get a reference to a member by ID.
    pub fn get_member(&self, id: Uuid) -> Option<&PartyMember> {
        self.members.iter().find(|m| m.id == id)
    }

    /// Get a mutable reference to a member by ID.
    pub fn get_member_mut(&mut self, id: Uuid) -> Option<&mut PartyMember> {
        self.members.iter_mut().find(|m| m.id == id)
    }

    /// Remove dead members from the roster, returning them.
    pub fn remove_dead(&mut self) -> Vec<PartyMember> {
        let mut dead = Vec::new();
        self.members.retain(|m| {
            if m.alive {
                true
            } else {
                dead.push(m.clone());
                false
            }
        });
        dead
    }

    /// Get all living members.
    pub fn living_members(&self) -> Vec<&PartyMember> {
        self.members.iter().filter(|m| m.alive).collect()
    }

    /// Number of active members.
    pub fn size(&self) -> usize {
        self.members.len()
    }

    /// Check if the roster is full.
    pub fn is_full(&self) -> bool {
        self.members.len() >= MAX_ACTIVE_PARTY
    }

    /// Check if the roster is empty.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Check if all members are dead.
    pub fn all_dead(&self) -> bool {
        !self.members.is_empty() && self.members.iter().all(|m| !m.alive)
    }
}

impl Default for Roster {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::generate_party_member;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn test_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn test_add_and_size() {
        let mut rng = test_rng();
        let mut roster = Roster::new();
        assert!(roster.is_empty());

        let member = generate_party_member(&mut rng, 1);
        roster.add_member(member).unwrap();
        assert_eq!(roster.size(), 1);
        assert!(!roster.is_empty());
    }

    #[test]
    fn test_roster_full() {
        let mut rng = test_rng();
        let mut roster = Roster::new();

        for _ in 0..MAX_ACTIVE_PARTY {
            let m = generate_party_member(&mut rng, 1);
            roster.add_member(m).unwrap();
        }
        assert!(roster.is_full());

        let extra = generate_party_member(&mut rng, 1);
        assert!(roster.add_member(extra).is_err());
    }

    #[test]
    fn test_dismiss() {
        let mut rng = test_rng();
        let mut roster = Roster::new();
        let member = generate_party_member(&mut rng, 1);
        let id = member.id;
        roster.add_member(member).unwrap();

        let dismissed = roster.dismiss(id);
        assert!(dismissed.is_some());
        assert!(roster.is_empty());
    }

    #[test]
    fn test_dismiss_nonexistent() {
        let mut roster = Roster::new();
        assert!(roster.dismiss(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_get_member() {
        let mut rng = test_rng();
        let mut roster = Roster::new();
        let member = generate_party_member(&mut rng, 1);
        let id = member.id;
        let name = member.name.clone();
        roster.add_member(member).unwrap();

        assert_eq!(roster.get_member(id).unwrap().name, name);
        assert!(roster.get_member(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_remove_dead() {
        let mut rng = test_rng();
        let mut roster = Roster::new();

        let alive = generate_party_member(&mut rng, 1);
        let mut dead = generate_party_member(&mut rng, 1);
        dead.alive = false;

        roster.add_member(alive).unwrap();
        roster.add_member(dead).unwrap();
        assert_eq!(roster.size(), 2);

        let removed = roster.remove_dead();
        assert_eq!(removed.len(), 1);
        assert_eq!(roster.size(), 1);
        assert!(roster.members[0].alive);
    }

    #[test]
    fn test_all_dead() {
        let mut rng = test_rng();
        let mut roster = Roster::new();
        assert!(!roster.all_dead()); // Empty roster is not "all dead"

        let mut m1 = generate_party_member(&mut rng, 1);
        let mut m2 = generate_party_member(&mut rng, 1);
        m1.alive = false;
        m2.alive = false;
        roster.add_member(m1).unwrap();
        roster.add_member(m2).unwrap();
        assert!(roster.all_dead());
    }

    #[test]
    fn test_living_members() {
        let mut rng = test_rng();
        let mut roster = Roster::new();

        let alive = generate_party_member(&mut rng, 1);
        let mut dead = generate_party_member(&mut rng, 1);
        dead.alive = false;

        roster.add_member(alive).unwrap();
        roster.add_member(dead).unwrap();

        let living = roster.living_members();
        assert_eq!(living.len(), 1);
    }

    #[test]
    fn test_roster_serde_roundtrip() {
        let mut rng = test_rng();
        let mut roster = Roster::new();
        roster.add_member(generate_party_member(&mut rng, 1)).unwrap();
        roster.add_member(generate_party_member(&mut rng, 2)).unwrap();

        let json = serde_json::to_string(&roster).unwrap();
        let deserialized: Roster = serde_json::from_str(&json).unwrap();
        assert_eq!(roster, deserialized);
    }
}
