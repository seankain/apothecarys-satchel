use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::generation::{generate_party_member, PartyMember};

/// Pool of recruitable party members available at the hub.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecruitmentPool {
    pub candidates: Vec<PartyMember>,
}

impl RecruitmentPool {
    /// Generate a fresh recruitment pool with 3-6 candidates.
    pub fn generate(rng: &mut impl Rng, level: u32) -> Self {
        let count = rng.gen_range(3..=6);
        let candidates = (0..count)
            .map(|_| generate_party_member(rng, level))
            .collect();
        Self { candidates }
    }

    /// Recruit a candidate by index, removing them from the pool.
    /// Returns `None` if the index is out of bounds.
    pub fn recruit(&mut self, index: usize) -> Option<PartyMember> {
        if index < self.candidates.len() {
            Some(self.candidates.remove(index))
        } else {
            None
        }
    }

    /// Refresh the pool (called after dungeon runs). Old candidates leave,
    /// new ones arrive.
    pub fn refresh(&mut self, rng: &mut impl Rng, level: u32) {
        *self = Self::generate(rng, level);
    }

    /// Number of available candidates.
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    /// Check if the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn test_generate_pool_size() {
        let mut rng = StdRng::seed_from_u64(42);
        let pool = RecruitmentPool::generate(&mut rng, 1);
        assert!((3..=6).contains(&pool.candidate_count()));
    }

    #[test]
    fn test_recruit_removes_from_pool() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut pool = RecruitmentPool::generate(&mut rng, 1);
        let initial = pool.candidate_count();

        let recruited = pool.recruit(0);
        assert!(recruited.is_some());
        assert_eq!(pool.candidate_count(), initial - 1);
    }

    #[test]
    fn test_recruit_out_of_bounds() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut pool = RecruitmentPool::generate(&mut rng, 1);
        assert!(pool.recruit(100).is_none());
    }

    #[test]
    fn test_refresh_replaces_pool() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut pool = RecruitmentPool::generate(&mut rng, 1);
        let old_names: Vec<_> = pool.candidates.iter().map(|c| c.name.clone()).collect();

        pool.refresh(&mut rng, 2);
        let new_names: Vec<_> = pool.candidates.iter().map(|c| c.name.clone()).collect();

        // Pool was refreshed (at different level), very likely different members
        assert!((3..=6).contains(&pool.candidate_count()));
        // Names could theoretically match but extremely unlikely
        assert_ne!(old_names, new_names);
    }

    #[test]
    fn test_pool_serde_roundtrip() {
        let mut rng = StdRng::seed_from_u64(42);
        let pool = RecruitmentPool::generate(&mut rng, 1);
        let json = serde_json::to_string(&pool).unwrap();
        let deserialized: RecruitmentPool = serde_json::from_str(&json).unwrap();
        assert_eq!(pool, deserialized);
    }
}
