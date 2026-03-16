//! HUD overlay showing party HP bars, minimap indicator, and notifications.

/// A single party member's HUD display data.
#[derive(Debug, Clone)]
pub struct PartyMemberHudData {
    pub name: String,
    pub current_hp: i32,
    pub max_hp: i32,
    pub level: u32,
    pub class_name: String,
    pub status_effect_count: usize,
}

impl PartyMemberHudData {
    /// HP as a fraction from 0.0 to 1.0.
    pub fn hp_fraction(&self) -> f32 {
        if self.max_hp <= 0 {
            return 0.0;
        }
        (self.current_hp as f32 / self.max_hp as f32).clamp(0.0, 1.0)
    }

    /// Whether this member is in danger (below 25% HP).
    pub fn is_danger(&self) -> bool {
        self.hp_fraction() < 0.25
    }

    /// Whether this member is at low health (below 50% HP).
    pub fn is_low_health(&self) -> bool {
        self.hp_fraction() < 0.5
    }
}

/// A notification message to display on the HUD.
#[derive(Debug, Clone)]
pub struct HudNotification {
    pub message: String,
    pub remaining_time: f32,
    pub notification_type: NotificationType,
}

/// Types of notifications affecting visual style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    Info,
    Warning,
    Success,
    Combat,
}

/// The complete HUD state.
pub struct HudState {
    pub party_data: Vec<PartyMemberHudData>,
    pub player_data: Option<PartyMemberHudData>,
    pub notifications: Vec<HudNotification>,
    pub current_location_name: String,
    pub visible: bool,
    notification_duration: f32,
}

impl HudState {
    pub fn new() -> Self {
        Self {
            party_data: Vec::new(),
            player_data: None,
            notifications: Vec::new(),
            current_location_name: String::new(),
            visible: true,
            notification_duration: 3.0,
        }
    }

    /// Push a notification with default duration.
    pub fn notify(&mut self, message: impl Into<String>, notification_type: NotificationType) {
        self.notifications.push(HudNotification {
            message: message.into(),
            remaining_time: self.notification_duration,
            notification_type,
        });
    }

    /// Update HUD state (tick notification timers).
    pub fn update(&mut self, dt: f32) {
        for notif in &mut self.notifications {
            notif.remaining_time -= dt;
        }
        self.notifications.retain(|n| n.remaining_time > 0.0);
    }

    /// Set the location display name.
    pub fn set_location(&mut self, name: impl Into<String>) {
        self.current_location_name = name.into();
    }

    /// Update party member display data.
    pub fn update_party_data(&mut self, members: &[PartyMemberHudData]) {
        self.party_data = members.to_vec();
    }

    /// Number of active notifications.
    pub fn notification_count(&self) -> usize {
        self.notifications.len()
    }
}

impl Default for HudState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hp_fraction() {
        let data = PartyMemberHudData {
            name: "Test".to_string(),
            current_hp: 15,
            max_hp: 20,
            level: 1,
            class_name: "Warrior".to_string(),
            status_effect_count: 0,
        };
        assert!((data.hp_fraction() - 0.75).abs() < f32::EPSILON);
        assert!(!data.is_danger());
        assert!(!data.is_low_health());
    }

    #[test]
    fn test_hp_fraction_danger() {
        let data = PartyMemberHudData {
            name: "Test".to_string(),
            current_hp: 3,
            max_hp: 20,
            level: 1,
            class_name: "Mage".to_string(),
            status_effect_count: 0,
        };
        assert!(data.is_danger());
        assert!(data.is_low_health());
    }

    #[test]
    fn test_hp_fraction_zero_max() {
        let data = PartyMemberHudData {
            name: "Test".to_string(),
            current_hp: 0,
            max_hp: 0,
            level: 1,
            class_name: "Test".to_string(),
            status_effect_count: 0,
        };
        assert!((data.hp_fraction()).abs() < f32::EPSILON);
    }

    #[test]
    fn test_notification_lifecycle() {
        let mut hud = HudState::new();
        hud.notify("Item picked up!", NotificationType::Info);
        assert_eq!(hud.notification_count(), 1);

        // Tick but not expired
        hud.update(1.0);
        assert_eq!(hud.notification_count(), 1);

        // Expire
        hud.update(3.0);
        assert_eq!(hud.notification_count(), 0);
    }

    #[test]
    fn test_hud_set_location() {
        let mut hud = HudState::new();
        hud.set_location("Willowmere");
        assert_eq!(hud.current_location_name, "Willowmere");
    }

    #[test]
    fn test_update_party_data() {
        let mut hud = HudState::new();
        let data = vec![PartyMemberHudData {
            name: "Fighter".to_string(),
            current_hp: 20,
            max_hp: 20,
            level: 3,
            class_name: "Warrior".to_string(),
            status_effect_count: 1,
        }];
        hud.update_party_data(&data);
        assert_eq!(hud.party_data.len(), 1);
    }
}
