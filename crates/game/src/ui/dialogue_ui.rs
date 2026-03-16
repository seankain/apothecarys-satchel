//! Dialogue UI state management: text box, choices, speaker portrait.

use apothecarys_dialogue::runner::DialogueState;

/// State for the dialogue UI overlay.
pub struct DialogueUiState {
    pub visible: bool,
    pub speaker_name: Option<String>,
    pub dialogue_text: String,
    pub choices: Vec<ChoiceDisplay>,
    pub show_continue_prompt: bool,
}

/// Display data for a dialogue choice.
#[derive(Debug, Clone)]
pub struct ChoiceDisplay {
    pub index: usize,
    pub text: String,
    pub enabled: bool,
}

impl DialogueUiState {
    pub fn new() -> Self {
        Self {
            visible: false,
            speaker_name: None,
            dialogue_text: String::new(),
            choices: Vec::new(),
            show_continue_prompt: false,
        }
    }

    /// Update the UI from the dialogue runner's state.
    pub fn sync_from_state(&mut self, state: &DialogueState) {
        match state {
            DialogueState::Idle => {
                self.visible = false;
                self.clear();
            }
            DialogueState::ShowingLine { speaker, text } => {
                self.visible = true;
                self.speaker_name = speaker.clone();
                self.dialogue_text = text.clone();
                self.choices.clear();
                self.show_continue_prompt = true;
            }
            DialogueState::WaitingForChoice { choices } => {
                self.visible = true;
                self.choices = choices
                    .iter()
                    .map(|c| ChoiceDisplay {
                        index: c.index,
                        text: c.text.clone(),
                        enabled: c.enabled,
                    })
                    .collect();
                self.show_continue_prompt = false;
            }
            DialogueState::ExecutingCommand { .. } => {
                // Commands are invisible to the player
                self.visible = false;
            }
            DialogueState::Finished => {
                self.visible = false;
                self.clear();
            }
        }
    }

    /// Get the number of available (enabled) choices.
    pub fn available_choice_count(&self) -> usize {
        self.choices.iter().filter(|c| c.enabled).count()
    }

    /// Check if the dialogue is currently showing choices.
    pub fn is_showing_choices(&self) -> bool {
        !self.choices.is_empty()
    }

    /// Clear all dialogue display state.
    pub fn clear(&mut self) {
        self.speaker_name = None;
        self.dialogue_text.clear();
        self.choices.clear();
        self.show_continue_prompt = false;
    }
}

impl Default for DialogueUiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_dialogue::runner::AvailableChoice;

    #[test]
    fn test_dialogue_ui_initial_state() {
        let ui = DialogueUiState::new();
        assert!(!ui.visible);
        assert!(ui.speaker_name.is_none());
        assert!(!ui.is_showing_choices());
    }

    #[test]
    fn test_sync_showing_line() {
        let mut ui = DialogueUiState::new();
        let state = DialogueState::ShowingLine {
            speaker: Some("NPC".to_string()),
            text: "Hello there!".to_string(),
        };
        ui.sync_from_state(&state);
        assert!(ui.visible);
        assert_eq!(ui.speaker_name, Some("NPC".to_string()));
        assert_eq!(ui.dialogue_text, "Hello there!");
        assert!(ui.show_continue_prompt);
        assert!(!ui.is_showing_choices());
    }

    #[test]
    fn test_sync_waiting_for_choice() {
        let mut ui = DialogueUiState::new();
        let state = DialogueState::WaitingForChoice {
            choices: vec![
                AvailableChoice {
                    index: 0,
                    text: "Yes".to_string(),
                    enabled: true,
                },
                AvailableChoice {
                    index: 1,
                    text: "No".to_string(),
                    enabled: true,
                },
                AvailableChoice {
                    index: 2,
                    text: "Maybe".to_string(),
                    enabled: false,
                },
            ],
        };
        ui.sync_from_state(&state);
        assert!(ui.visible);
        assert!(ui.is_showing_choices());
        assert_eq!(ui.choices.len(), 3);
        assert_eq!(ui.available_choice_count(), 2);
        assert!(!ui.show_continue_prompt);
    }

    #[test]
    fn test_sync_finished() {
        let mut ui = DialogueUiState::new();
        // First show a line
        ui.sync_from_state(&DialogueState::ShowingLine {
            speaker: Some("NPC".to_string()),
            text: "Hello".to_string(),
        });
        assert!(ui.visible);

        // Then finish
        ui.sync_from_state(&DialogueState::Finished);
        assert!(!ui.visible);
        assert!(ui.dialogue_text.is_empty());
    }

    #[test]
    fn test_sync_command_invisible() {
        let mut ui = DialogueUiState::new();
        let state = DialogueState::ExecutingCommand {
            command: "give_item".to_string(),
            args: vec!["potion".to_string()],
        };
        ui.sync_from_state(&state);
        assert!(!ui.visible);
    }

    #[test]
    fn test_sync_idle() {
        let mut ui = DialogueUiState::new();
        ui.visible = true;
        ui.sync_from_state(&DialogueState::Idle);
        assert!(!ui.visible);
    }
}
