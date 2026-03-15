use apothecarys_core::components::{Interactable, InteractionType};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// An object in the world that can be interacted with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractableObject {
    /// Unique identifier for this object.
    pub id: u64,
    /// World-space position of the object.
    pub position: Vec3,
    /// The interaction data.
    pub interactable: Interactable,
}

/// Result of a hover/raycast query against interactable objects.
#[derive(Debug, Clone)]
pub struct HoverResult {
    pub object_id: u64,
    pub object_name: String,
    pub interaction_type: InteractionType,
    pub distance_to_player: f32,
    pub in_range: bool,
}

/// Result of attempting to interact with an object.
#[derive(Debug, Clone, PartialEq)]
pub enum InteractionResult {
    /// Player is close enough — execute the interaction.
    Execute {
        object_id: u64,
        interaction_type: InteractionType,
    },
    /// Player needs to navigate closer first.
    NavigateTo {
        object_id: u64,
        target_position: Vec3,
    },
    /// Object is disabled or doesn't exist.
    Unavailable,
}

/// Manages interaction detection and dispatch.
#[derive(Debug, Default)]
pub struct InteractionSystem {
    /// All registered interactable objects.
    objects: Vec<InteractableObject>,
    /// Currently hovered object ID, if any.
    hovered: Option<u64>,
    /// Object the player is navigating to for deferred interaction.
    pending_interaction: Option<u64>,
}

impl InteractionSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an interactable object.
    pub fn add_object(&mut self, object: InteractableObject) {
        self.objects.push(object);
    }

    /// Remove an interactable object by ID.
    pub fn remove_object(&mut self, id: u64) {
        self.objects.retain(|o| o.id != id);
        if self.hovered == Some(id) {
            self.hovered = None;
        }
        if self.pending_interaction == Some(id) {
            self.pending_interaction = None;
        }
    }

    /// Get an object by ID.
    pub fn get_object(&self, id: u64) -> Option<&InteractableObject> {
        self.objects.iter().find(|o| o.id == id)
    }

    /// Get a mutable reference to an object by ID.
    pub fn get_object_mut(&mut self, id: u64) -> Option<&mut InteractableObject> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    /// Find the nearest interactable object to a raycast hit point.
    /// `hit_point` is the world position where the mouse ray hits the scene.
    /// `max_pick_distance` is how close the hit must be to an object center.
    pub fn find_at_point(&self, hit_point: Vec3, max_pick_distance: f32) -> Option<&InteractableObject> {
        self.objects
            .iter()
            .filter(|o| o.interactable.enabled)
            .filter(|o| o.position.distance(hit_point) <= max_pick_distance)
            .min_by(|a, b| {
                let da = a.position.distance(hit_point);
                let db = b.position.distance(hit_point);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Update hover state based on a raycast hit point.
    /// Returns hover info if an object is under the cursor.
    pub fn update_hover(
        &mut self,
        hit_point: Option<Vec3>,
        player_position: Vec3,
        max_pick_distance: f32,
    ) -> Option<HoverResult> {
        let hit_point = hit_point?;
        let obj = self.find_at_point(hit_point, max_pick_distance)?;
        let distance = obj.position.distance(player_position);
        let in_range = distance <= obj.interactable.interaction_range;
        let id = obj.id;
        let name = obj.interactable.name.clone();
        let itype = obj.interactable.interaction_type.clone();

        self.hovered = Some(id);

        Some(HoverResult {
            object_id: id,
            object_name: name,
            interaction_type: itype,
            distance_to_player: distance,
            in_range,
        })
    }

    /// Clear hover state (mouse moved off interactable objects).
    pub fn clear_hover(&mut self) {
        self.hovered = None;
    }

    /// Get the currently hovered object ID.
    pub fn hovered_object(&self) -> Option<u64> {
        self.hovered
    }

    /// Attempt to interact with the object at the given point.
    /// If the player is in range, returns Execute. Otherwise, returns NavigateTo.
    pub fn try_interact(
        &mut self,
        hit_point: Vec3,
        player_position: Vec3,
        max_pick_distance: f32,
    ) -> InteractionResult {
        let obj = match self.find_at_point(hit_point, max_pick_distance) {
            Some(obj) => obj,
            None => return InteractionResult::Unavailable,
        };

        if !obj.interactable.enabled {
            return InteractionResult::Unavailable;
        }

        let distance = obj.position.distance(player_position);
        let id = obj.id;
        let interaction_type = obj.interactable.interaction_type.clone();
        let position = obj.position;
        let range = obj.interactable.interaction_range;

        if distance <= range {
            self.pending_interaction = None;
            InteractionResult::Execute {
                object_id: id,
                interaction_type,
            }
        } else {
            self.pending_interaction = Some(id);
            InteractionResult::NavigateTo {
                object_id: id,
                target_position: position,
            }
        }
    }

    /// Check if the player has reached a pending interaction target.
    /// Call this after movement updates to handle deferred interactions.
    pub fn check_pending_interaction(
        &mut self,
        player_position: Vec3,
    ) -> Option<InteractionResult> {
        let pending_id = self.pending_interaction?;
        let obj = self.objects.iter().find(|o| o.id == pending_id)?;

        if !obj.interactable.enabled {
            self.pending_interaction = None;
            return None;
        }

        let distance = obj.position.distance(player_position);
        if distance <= obj.interactable.interaction_range {
            self.pending_interaction = None;
            Some(InteractionResult::Execute {
                object_id: obj.id,
                interaction_type: obj.interactable.interaction_type.clone(),
            })
        } else {
            None
        }
    }

    /// Cancel any pending interaction.
    pub fn cancel_pending(&mut self) {
        self.pending_interaction = None;
    }

    /// Get all registered objects.
    pub fn objects(&self) -> &[InteractableObject] {
        &self.objects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pickup(id: u64, pos: Vec3) -> InteractableObject {
        InteractableObject {
            id,
            position: pos,
            interactable: Interactable::new("Healing Herb", InteractionType::Pickup),
        }
    }

    fn make_npc(id: u64, pos: Vec3) -> InteractableObject {
        InteractableObject {
            id,
            position: pos,
            interactable: Interactable::new("Merchant", InteractionType::Npc),
        }
    }

    fn make_exit(id: u64, pos: Vec3) -> InteractableObject {
        InteractableObject {
            id,
            position: pos,
            interactable: Interactable::new("Dungeon Entrance", InteractionType::Exit)
                .with_range(3.0),
        }
    }

    #[test]
    fn test_add_and_find_objects() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));
        sys.add_object(make_npc(2, Vec3::new(10.0, 0.0, 10.0)));

        assert_eq!(sys.objects().len(), 2);
        assert!(sys.get_object(1).is_some());
        assert!(sys.get_object(3).is_none());
    }

    #[test]
    fn test_remove_object() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));
        sys.remove_object(1);
        assert!(sys.objects().is_empty());
    }

    #[test]
    fn test_find_at_point() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));
        sys.add_object(make_npc(2, Vec3::new(15.0, 0.0, 15.0)));

        // Hit near pickup
        let found = sys.find_at_point(Vec3::new(5.5, 0.0, 5.0), 2.0);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 1);

        // Hit near NPC
        let found = sys.find_at_point(Vec3::new(14.5, 0.0, 15.0), 2.0);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 2);

        // Hit nowhere near anything
        let found = sys.find_at_point(Vec3::new(50.0, 0.0, 50.0), 2.0);
        assert!(found.is_none());
    }

    #[test]
    fn test_hover_detection() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));

        let player_pos = Vec3::new(5.5, 0.0, 5.0);
        let result = sys.update_hover(Some(Vec3::new(5.0, 0.0, 5.0)), player_pos, 2.0);
        assert!(result.is_some());
        let hover = result.unwrap();
        assert_eq!(hover.object_id, 1);
        assert_eq!(hover.object_name, "Healing Herb");
        assert!(hover.in_range); // within 2.0 (default range)
        assert_eq!(sys.hovered_object(), Some(1));
    }

    #[test]
    fn test_hover_out_of_range() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));

        let player_pos = Vec3::new(20.0, 0.0, 20.0); // far away
        let result = sys.update_hover(Some(Vec3::new(5.0, 0.0, 5.0)), player_pos, 2.0);
        assert!(result.is_some());
        assert!(!result.unwrap().in_range);
    }

    #[test]
    fn test_interact_in_range() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));

        let player_pos = Vec3::new(5.5, 0.0, 5.0); // close
        let result = sys.try_interact(Vec3::new(5.0, 0.0, 5.0), player_pos, 2.0);
        assert_eq!(
            result,
            InteractionResult::Execute {
                object_id: 1,
                interaction_type: InteractionType::Pickup,
            }
        );
    }

    #[test]
    fn test_interact_out_of_range_navigates() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));

        let player_pos = Vec3::new(20.0, 0.0, 20.0); // far away
        let result = sys.try_interact(Vec3::new(5.0, 0.0, 5.0), player_pos, 2.0);
        match result {
            InteractionResult::NavigateTo { object_id, target_position } => {
                assert_eq!(object_id, 1);
                assert_eq!(target_position, Vec3::new(5.0, 0.0, 5.0));
            }
            _ => panic!("Expected NavigateTo"),
        }
    }

    #[test]
    fn test_pending_interaction() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));

        // Player clicks from far away
        let _ = sys.try_interact(Vec3::new(5.0, 0.0, 5.0), Vec3::new(20.0, 0.0, 20.0), 2.0);

        // Player hasn't arrived yet
        assert!(sys.check_pending_interaction(Vec3::new(10.0, 0.0, 10.0)).is_none());

        // Player arrives
        let result = sys.check_pending_interaction(Vec3::new(5.5, 0.0, 5.0));
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            InteractionResult::Execute {
                object_id: 1,
                interaction_type: InteractionType::Pickup,
            }
        );
    }

    #[test]
    fn test_interact_disabled_object() {
        let mut sys = InteractionSystem::new();
        let mut obj = make_pickup(1, Vec3::new(5.0, 0.0, 5.0));
        obj.interactable.enabled = false;
        sys.add_object(obj);

        let result = sys.try_interact(Vec3::new(5.0, 0.0, 5.0), Vec3::new(5.5, 0.0, 5.0), 2.0);
        assert_eq!(result, InteractionResult::Unavailable);
    }

    #[test]
    fn test_interact_nothing() {
        let sys = InteractionSystem::new();
        let result = sys.find_at_point(Vec3::new(5.0, 0.0, 5.0), 2.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_exit_interaction_range() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_exit(1, Vec3::new(5.0, 0.0, 5.0)));

        // Exit has range 3.0
        let player_pos = Vec3::new(7.5, 0.0, 5.0); // 2.5 away, within 3.0
        let result = sys.try_interact(Vec3::new(5.0, 0.0, 5.0), player_pos, 2.0);
        assert_eq!(
            result,
            InteractionResult::Execute {
                object_id: 1,
                interaction_type: InteractionType::Exit,
            }
        );
    }

    #[test]
    fn test_cancel_pending() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));

        let _ = sys.try_interact(Vec3::new(5.0, 0.0, 5.0), Vec3::new(20.0, 0.0, 20.0), 2.0);
        sys.cancel_pending();

        let result = sys.check_pending_interaction(Vec3::new(5.5, 0.0, 5.0));
        assert!(result.is_none());
    }

    #[test]
    fn test_clear_hover() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));

        sys.update_hover(Some(Vec3::new(5.0, 0.0, 5.0)), Vec3::ZERO, 2.0);
        assert_eq!(sys.hovered_object(), Some(1));

        sys.clear_hover();
        assert_eq!(sys.hovered_object(), None);
    }

    #[test]
    fn test_nearest_object_picked() {
        let mut sys = InteractionSystem::new();
        sys.add_object(make_pickup(1, Vec3::new(5.0, 0.0, 5.0)));
        sys.add_object(make_pickup(2, Vec3::new(5.5, 0.0, 5.0)));

        // Hit closer to object 2
        let found = sys.find_at_point(Vec3::new(5.4, 0.0, 5.0), 2.0);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 2);
    }
}
