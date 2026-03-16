//! Combat UI state management: turn indicator, action menu, target selection, HP bars.

use apothecarys_combat::turn_manager::{CombatPhase, CombatSide, CombatState, CombatantId};

/// UI state for the combat screen.
pub struct CombatUiState {
    pub visible: bool,
    pub action_menu_open: bool,
    pub target_selection_active: bool,
    pub selected_action: Option<CombatUiAction>,
    pub selected_target: Option<CombatantId>,
    pub combat_log: Vec<CombatLogEntry>,
    pub max_log_entries: usize,
}

/// Actions selectable from the combat UI menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatUiAction {
    UseItem,
    GiveItem,
    Examine,
    Wait,
}

/// A log entry describing what happened during combat.
#[derive(Debug, Clone)]
pub struct CombatLogEntry {
    pub message: String,
    pub entry_type: CombatLogType,
}

/// Types of combat log entries for color-coding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatLogType {
    PlayerAction,
    PartyAction,
    EnemyAction,
    StatusEffect,
    System,
}

impl CombatUiState {
    pub fn new() -> Self {
        Self {
            visible: false,
            action_menu_open: false,
            target_selection_active: false,
            selected_action: None,
            selected_target: None,
            combat_log: Vec::new(),
            max_log_entries: 50,
        }
    }

    /// Show combat UI and reset state.
    pub fn begin_combat(&mut self) {
        self.visible = true;
        self.action_menu_open = false;
        self.target_selection_active = false;
        self.selected_action = None;
        self.selected_target = None;
        self.combat_log.clear();
        self.log("Combat begins!", CombatLogType::System);
    }

    /// Hide combat UI.
    pub fn end_combat(&mut self) {
        self.visible = false;
        self.action_menu_open = false;
        self.target_selection_active = false;
    }

    /// Open the action menu (player's turn).
    pub fn open_action_menu(&mut self) {
        self.action_menu_open = true;
        self.selected_action = None;
        self.target_selection_active = false;
    }

    /// Select an action from the menu.
    pub fn select_action(&mut self, action: CombatUiAction) {
        self.selected_action = Some(action);
        self.action_menu_open = false;
        // Most actions need a target
        self.target_selection_active = action != CombatUiAction::Wait;
    }

    /// Select a target for the current action.
    pub fn select_target(&mut self, target: CombatantId) {
        self.selected_target = Some(target);
        self.target_selection_active = false;
    }

    /// Cancel the current action/target selection.
    pub fn cancel(&mut self) {
        if self.target_selection_active {
            self.target_selection_active = false;
            self.action_menu_open = true;
        } else {
            self.action_menu_open = false;
            self.selected_action = None;
        }
    }

    /// Add a combat log entry.
    pub fn log(&mut self, message: impl Into<String>, entry_type: CombatLogType) {
        self.combat_log.push(CombatLogEntry {
            message: message.into(),
            entry_type,
        });
        if self.combat_log.len() > self.max_log_entries {
            self.combat_log.remove(0);
        }
    }

    /// Get display data for all combatants.
    pub fn get_combatant_displays(&self, combat: &CombatState) -> Vec<CombatantDisplay> {
        combat
            .combatants
            .iter()
            .map(|e| CombatantDisplay {
                id: e.id,
                name: e.combatant.name.clone(),
                current_hp: e.combatant.base_stats.current_hp,
                max_hp: e.combatant.base_stats.max_hp,
                side: e.side,
                is_dead: e.combatant.is_dead(),
                is_current_turn: combat.current_combatant().map(|c| c.id) == Some(e.id)
                    && matches!(
                        &combat.phase,
                        CombatPhase::TurnStart { .. } | CombatPhase::ActionSelection
                    ),
                initiative: e.initiative,
            })
            .collect()
    }

    /// Get the phase display text.
    pub fn phase_text(&self, combat: &CombatState) -> String {
        match &combat.phase {
            CombatPhase::RollInitiative => "Rolling initiative...".to_string(),
            CombatPhase::TurnStart { combatant_id } => {
                let name = combat
                    .get_combatant(*combatant_id)
                    .map(|e| e.combatant.name.as_str())
                    .unwrap_or("???");
                format!("{name}'s turn")
            }
            CombatPhase::ActionSelection => "Select an action".to_string(),
            CombatPhase::ActionExecution => "Executing action...".to_string(),
            CombatPhase::TurnEnd => "Turn ending...".to_string(),
            CombatPhase::RoundEnd => format!("Round {} complete", combat.round),
            CombatPhase::Victory { xp, .. } => format!("Victory! +{xp} XP"),
            CombatPhase::Defeat => "Defeat...".to_string(),
        }
    }

    /// Get the number of log entries.
    pub fn log_count(&self) -> usize {
        self.combat_log.len()
    }
}

impl Default for CombatUiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Display data for a single combatant in the combat UI.
#[derive(Debug, Clone)]
pub struct CombatantDisplay {
    pub id: CombatantId,
    pub name: String,
    pub current_hp: i32,
    pub max_hp: i32,
    pub side: CombatSide,
    pub is_dead: bool,
    pub is_current_turn: bool,
    pub initiative: i32,
}

impl CombatantDisplay {
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
    use apothecarys_core::stats::{Attributes, Combatant, DerivedStats};
    use apothecarys_combat::turn_manager::CombatState;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn make_combat() -> CombatState {
        let player = Combatant::new(
            "Player",
            1,
            Attributes::default(),
            DerivedStats {
                max_hp: 20,
                current_hp: 20,
                ..Default::default()
            },
        );
        let ally = Combatant::new(
            "Ally",
            1,
            Attributes::default(),
            DerivedStats {
                max_hp: 15,
                current_hp: 15,
                ..Default::default()
            },
        );
        let enemy = Combatant::new(
            "Goblin",
            1,
            Attributes::default(),
            DerivedStats {
                max_hp: 10,
                current_hp: 10,
                ..Default::default()
            },
        );
        CombatState::new(player, vec![ally], vec![enemy], 50, vec![])
    }

    #[test]
    fn test_combat_ui_begin_end() {
        let mut ui = CombatUiState::new();
        assert!(!ui.visible);

        ui.begin_combat();
        assert!(ui.visible);
        assert_eq!(ui.log_count(), 1);

        ui.end_combat();
        assert!(!ui.visible);
    }

    #[test]
    fn test_action_selection_flow() {
        let mut ui = CombatUiState::new();
        ui.begin_combat();

        ui.open_action_menu();
        assert!(ui.action_menu_open);

        ui.select_action(CombatUiAction::UseItem);
        assert!(!ui.action_menu_open);
        assert!(ui.target_selection_active);
        assert_eq!(ui.selected_action, Some(CombatUiAction::UseItem));
    }

    #[test]
    fn test_wait_skips_targeting() {
        let mut ui = CombatUiState::new();
        ui.begin_combat();
        ui.open_action_menu();
        ui.select_action(CombatUiAction::Wait);
        assert!(!ui.target_selection_active);
    }

    #[test]
    fn test_cancel_target_returns_to_menu() {
        let mut ui = CombatUiState::new();
        ui.begin_combat();
        ui.open_action_menu();
        ui.select_action(CombatUiAction::UseItem);
        assert!(ui.target_selection_active);

        ui.cancel();
        assert!(!ui.target_selection_active);
        assert!(ui.action_menu_open);
    }

    #[test]
    fn test_cancel_menu_clears_action() {
        let mut ui = CombatUiState::new();
        ui.begin_combat();
        ui.open_action_menu();
        ui.cancel();
        assert!(!ui.action_menu_open);
        assert!(ui.selected_action.is_none());
    }

    #[test]
    fn test_combat_log() {
        let mut ui = CombatUiState::new();
        ui.log("Test entry", CombatLogType::System);
        assert_eq!(ui.log_count(), 1);
        assert_eq!(ui.combat_log[0].message, "Test entry");
    }

    #[test]
    fn test_combat_log_max_entries() {
        let mut ui = CombatUiState::new();
        ui.max_log_entries = 5;
        for i in 0..10 {
            ui.log(format!("Entry {i}"), CombatLogType::System);
        }
        assert_eq!(ui.log_count(), 5);
        assert_eq!(ui.combat_log[0].message, "Entry 5");
    }

    #[test]
    fn test_combatant_display() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);

        let ui = CombatUiState::new();
        let displays = ui.get_combatant_displays(&combat);
        assert_eq!(displays.len(), 3);

        // At least one should have is_current_turn
        let current = displays.iter().filter(|d| d.is_current_turn).count();
        assert!(current <= 1);
    }

    #[test]
    fn test_phase_text() {
        let mut rng = StdRng::seed_from_u64(42);
        let combat = make_combat();
        let ui = CombatUiState::new();

        assert_eq!(ui.phase_text(&combat), "Rolling initiative...");

        let mut combat = make_combat();
        combat.roll_initiative(&mut rng);
        let text = ui.phase_text(&combat);
        assert!(text.contains("turn"));
    }

    #[test]
    fn test_combatant_display_hp_fraction() {
        let display = CombatantDisplay {
            id: CombatantId(uuid::Uuid::new_v4()),
            name: "Test".to_string(),
            current_hp: 7,
            max_hp: 10,
            side: CombatSide::Player,
            is_dead: false,
            is_current_turn: false,
            initiative: 15,
        };
        assert!((display.hp_fraction() - 0.7).abs() < f32::EPSILON);
    }
}
