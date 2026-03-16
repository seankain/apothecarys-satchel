//! Recruitment UI state management: candidate list, stats display, recruit/dismiss.

use apothecarys_party::generation::PartyClass;
use apothecarys_party::recruitment::RecruitmentPool;
use apothecarys_party::roster::Roster;

/// State for the recruitment UI screen.
pub struct RecruitmentUiState {
    pub visible: bool,
    pub selected_candidate: Option<usize>,
    pub show_stats_detail: bool,
}

impl RecruitmentUiState {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected_candidate: None,
            show_stats_detail: false,
        }
    }

    /// Toggle recruitment screen visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.selected_candidate = None;
            self.show_stats_detail = false;
        }
    }

    /// Select a candidate from the pool.
    pub fn select_candidate(&mut self, index: usize) {
        self.selected_candidate = Some(index);
        self.show_stats_detail = true;
    }

    /// Deselect the current candidate.
    pub fn deselect(&mut self) {
        self.selected_candidate = None;
        self.show_stats_detail = false;
    }

    /// Get display data for all candidates in the pool.
    pub fn get_candidate_displays(pool: &RecruitmentPool) -> Vec<CandidateDisplay> {
        pool.candidates
            .iter()
            .enumerate()
            .map(|(i, m)| CandidateDisplay {
                index: i,
                name: m.name.clone(),
                class: m.class,
                level: m.level,
                hp: m.derived.max_hp,
                attack: m.derived.attack_bonus,
                defense: m.derived.armor_class,
            })
            .collect()
    }

    /// Get display data for all current roster members.
    pub fn get_roster_displays(roster: &Roster) -> Vec<RosterMemberDisplay> {
        roster
            .members
            .iter()
            .map(|m| RosterMemberDisplay {
                id: m.id,
                name: m.name.clone(),
                class: m.class,
                level: m.level,
                current_hp: m.derived.current_hp,
                max_hp: m.derived.max_hp,
                alive: m.alive,
            })
            .collect()
    }

    /// Get detailed stats for a candidate.
    pub fn get_candidate_details(pool: &RecruitmentPool, index: usize) -> Option<CandidateDetails> {
        pool.candidates.get(index).map(|m| CandidateDetails {
            name: m.name.clone(),
            class: m.class,
            level: m.level,
            strength: m.attributes.strength,
            dexterity: m.attributes.dexterity,
            constitution: m.attributes.constitution,
            intelligence: m.attributes.intelligence,
            wisdom: m.attributes.wisdom,
            charisma: m.attributes.charisma,
            max_hp: m.derived.max_hp,
            armor_class: m.derived.armor_class,
            attack_bonus: m.derived.attack_bonus,
            backstory: m.backstory.clone(),
            personality_aggression: m.personality.aggression,
            personality_caution: m.personality.caution,
        })
    }
}

impl Default for RecruitmentUiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Brief display data for a recruitment candidate.
#[derive(Debug, Clone)]
pub struct CandidateDisplay {
    pub index: usize,
    pub name: String,
    pub class: PartyClass,
    pub level: u32,
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
}

/// Detailed stats for a candidate.
#[derive(Debug, Clone)]
pub struct CandidateDetails {
    pub name: String,
    pub class: PartyClass,
    pub level: u32,
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
    pub max_hp: i32,
    pub armor_class: i32,
    pub attack_bonus: i32,
    pub backstory: String,
    pub personality_aggression: f32,
    pub personality_caution: f32,
}

/// Display data for a roster member.
#[derive(Debug, Clone)]
pub struct RosterMemberDisplay {
    pub id: uuid::Uuid,
    pub name: String,
    pub class: PartyClass,
    pub level: u32,
    pub current_hp: i32,
    pub max_hp: i32,
    pub alive: bool,
}

impl RosterMemberDisplay {
    pub fn hp_fraction(&self) -> f32 {
        if self.max_hp <= 0 {
            return 0.0;
        }
        (self.current_hp as f32 / self.max_hp as f32).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_party::generation::generate_party_member;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn test_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    #[test]
    fn test_recruitment_ui_toggle() {
        let mut ui = RecruitmentUiState::new();
        assert!(!ui.visible);
        ui.toggle();
        assert!(ui.visible);
        ui.toggle();
        assert!(!ui.visible);
    }

    #[test]
    fn test_select_candidate() {
        let mut ui = RecruitmentUiState::new();
        ui.select_candidate(2);
        assert_eq!(ui.selected_candidate, Some(2));
        assert!(ui.show_stats_detail);
    }

    #[test]
    fn test_deselect() {
        let mut ui = RecruitmentUiState::new();
        ui.select_candidate(0);
        ui.deselect();
        assert!(ui.selected_candidate.is_none());
        assert!(!ui.show_stats_detail);
    }

    #[test]
    fn test_candidate_displays() {
        let mut rng = test_rng();
        let pool = RecruitmentPool::generate(&mut rng, 1);
        let displays = RecruitmentUiState::get_candidate_displays(&pool);
        assert_eq!(displays.len(), pool.candidate_count());
        for d in &displays {
            assert!(!d.name.is_empty());
            assert!(d.hp > 0);
        }
    }

    #[test]
    fn test_roster_displays() {
        let mut rng = test_rng();
        let mut roster = Roster::new();
        roster
            .add_member(generate_party_member(&mut rng, 1))
            .unwrap();
        roster
            .add_member(generate_party_member(&mut rng, 2))
            .unwrap();

        let displays = RecruitmentUiState::get_roster_displays(&roster);
        assert_eq!(displays.len(), 2);
        for d in &displays {
            assert!(d.alive);
        }
    }

    #[test]
    fn test_candidate_details() {
        let mut rng = test_rng();
        let pool = RecruitmentPool::generate(&mut rng, 1);
        let details = RecruitmentUiState::get_candidate_details(&pool, 0);
        assert!(details.is_some());

        let d = details.unwrap();
        assert!(!d.name.is_empty());
        assert!(!d.backstory.is_empty());
        assert!((0.0..=1.0).contains(&d.personality_aggression));
    }

    #[test]
    fn test_candidate_details_out_of_bounds() {
        let mut rng = test_rng();
        let pool = RecruitmentPool::generate(&mut rng, 1);
        let details = RecruitmentUiState::get_candidate_details(&pool, 100);
        assert!(details.is_none());
    }

    #[test]
    fn test_roster_member_hp_fraction() {
        let display = RosterMemberDisplay {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            class: PartyClass::Warrior,
            level: 1,
            current_hp: 15,
            max_hp: 20,
            alive: true,
        };
        assert!((display.hp_fraction() - 0.75).abs() < f32::EPSILON);
    }
}
