//! Garden UI state management: plot grid, plant info, breed/harvest buttons.

use apothecarys_garden::plots::{Garden, PlotState};

/// State for the garden UI screen.
pub struct GardenUiState {
    pub visible: bool,
    pub selected_plot: Option<usize>,
    pub breeding_mode: bool,
    pub breeding_first_plot: Option<usize>,
}

impl GardenUiState {
    pub fn new() -> Self {
        Self {
            visible: false,
            selected_plot: None,
            breeding_mode: false,
            breeding_first_plot: None,
        }
    }

    /// Toggle garden UI visibility.
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.clear();
        }
    }

    /// Select a garden plot.
    pub fn select_plot(&mut self, index: usize) {
        if self.breeding_mode {
            if self.breeding_first_plot.is_none() {
                self.breeding_first_plot = Some(index);
            } else {
                // Second plot selected — breeding pair ready
                self.selected_plot = Some(index);
            }
        } else {
            self.selected_plot = Some(index);
        }
    }

    /// Enter breeding mode.
    pub fn start_breeding(&mut self) {
        self.breeding_mode = true;
        self.breeding_first_plot = None;
    }

    /// Cancel breeding mode.
    pub fn cancel_breeding(&mut self) {
        self.breeding_mode = false;
        self.breeding_first_plot = None;
    }

    /// Check if a breeding pair is fully selected.
    pub fn breeding_pair_ready(&self) -> bool {
        self.breeding_mode
            && self.breeding_first_plot.is_some()
            && self.selected_plot.is_some()
    }

    /// Get the breeding pair indices if ready.
    pub fn breeding_pair(&self) -> Option<(usize, usize)> {
        if self.breeding_pair_ready() {
            Some((self.breeding_first_plot.unwrap(), self.selected_plot.unwrap()))
        } else {
            None
        }
    }

    /// Get available actions for the selected plot.
    pub fn available_actions(&self, garden: &Garden) -> Vec<GardenAction> {
        let plot_idx = match self.selected_plot {
            Some(i) => i,
            None => return Vec::new(),
        };

        let plot = match garden.get_plot(plot_idx) {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        let mut actions = Vec::new();

        match &plot.state {
            PlotState::Empty => {
                actions.push(GardenAction::Plant);
            }
            PlotState::Planted {
                growth_stage,
                watered,
                ..
            } => {
                if !watered {
                    actions.push(GardenAction::Water);
                }
                if *growth_stage >= 1.0 {
                    actions.push(GardenAction::Harvest);
                    actions.push(GardenAction::Breed);
                }
                actions.push(GardenAction::Clear);
            }
        }

        actions.push(GardenAction::Inspect);
        actions
    }

    /// Get display data for all plots.
    pub fn get_plot_displays(&self, garden: &Garden) -> Vec<PlotDisplay> {
        garden
            .plots
            .iter()
            .map(|plot| {
                let (species, growth, watered, health) = match &plot.state {
                    PlotState::Empty => (None, 0.0, false, 1.0),
                    PlotState::Planted {
                        plant,
                        growth_stage,
                        watered,
                        health,
                    } => (
                        Some(plant.species_name.clone()),
                        *growth_stage,
                        *watered,
                        *health,
                    ),
                };

                PlotDisplay {
                    index: plot.index,
                    species_name: species,
                    growth_stage: growth,
                    is_mature: plot.state.is_mature(),
                    is_empty: plot.state.is_empty(),
                    watered,
                    health,
                    is_selected: self.selected_plot == Some(plot.index),
                }
            })
            .collect()
    }

    /// Clear selection state.
    pub fn clear(&mut self) {
        self.selected_plot = None;
        self.breeding_mode = false;
        self.breeding_first_plot = None;
    }
}

impl Default for GardenUiState {
    fn default() -> Self {
        Self::new()
    }
}

/// Actions available for a garden plot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GardenAction {
    Plant,
    Water,
    Harvest,
    Breed,
    Clear,
    Inspect,
}

/// Display data for a garden plot.
#[derive(Debug, Clone)]
pub struct PlotDisplay {
    pub index: usize,
    pub species_name: Option<String>,
    pub growth_stage: f32,
    pub is_mature: bool,
    pub is_empty: bool,
    pub watered: bool,
    pub health: f32,
    pub is_selected: bool,
}

impl PlotDisplay {
    /// Growth as a percentage string.
    pub fn growth_percent(&self) -> String {
        format!("{}%", (self.growth_stage * 100.0) as u32)
    }

    /// Health as a percentage string.
    pub fn health_percent(&self) -> String {
        format!("{}%", (self.health * 100.0) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_garden::plots::{Garden, PlantInstance};
    use apothecarys_botany::genetics::PlantGenotype;
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    fn make_garden_with_plant() -> Garden {
        let mut garden = Garden::new(4);
        let mut rng = StdRng::seed_from_u64(42);
        let plant = PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Test Herb");
        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();
        garden
    }

    #[test]
    fn test_garden_ui_toggle() {
        let mut ui = GardenUiState::new();
        assert!(!ui.visible);
        ui.toggle();
        assert!(ui.visible);
        ui.toggle();
        assert!(!ui.visible);
    }

    #[test]
    fn test_select_plot() {
        let mut ui = GardenUiState::new();
        ui.select_plot(2);
        assert_eq!(ui.selected_plot, Some(2));
    }

    #[test]
    fn test_breeding_mode() {
        let mut ui = GardenUiState::new();
        ui.start_breeding();
        assert!(ui.breeding_mode);
        assert!(!ui.breeding_pair_ready());

        ui.select_plot(0); // First parent
        assert!(ui.breeding_first_plot.is_some());
        assert!(!ui.breeding_pair_ready());

        ui.select_plot(1); // Second parent
        assert!(ui.breeding_pair_ready());
        assert_eq!(ui.breeding_pair(), Some((0, 1)));
    }

    #[test]
    fn test_cancel_breeding() {
        let mut ui = GardenUiState::new();
        ui.start_breeding();
        ui.select_plot(0);
        ui.cancel_breeding();
        assert!(!ui.breeding_mode);
        assert!(ui.breeding_first_plot.is_none());
    }

    #[test]
    fn test_available_actions_empty_plot() {
        let ui = GardenUiState {
            visible: true,
            selected_plot: Some(0),
            breeding_mode: false,
            breeding_first_plot: None,
        };
        let garden = Garden::new(4);
        let actions = ui.available_actions(&garden);
        assert!(actions.contains(&GardenAction::Plant));
        assert!(actions.contains(&GardenAction::Inspect));
    }

    #[test]
    fn test_available_actions_planted() {
        let ui = GardenUiState {
            visible: true,
            selected_plot: Some(0),
            breeding_mode: false,
            breeding_first_plot: None,
        };
        let garden = make_garden_with_plant();
        let actions = ui.available_actions(&garden);
        assert!(actions.contains(&GardenAction::Water));
        assert!(actions.contains(&GardenAction::Clear));
        assert!(!actions.contains(&GardenAction::Harvest)); // Not mature yet
    }

    #[test]
    fn test_available_actions_mature() {
        let ui = GardenUiState {
            visible: true,
            selected_plot: Some(0),
            breeding_mode: false,
            breeding_first_plot: None,
        };
        let mut garden = make_garden_with_plant();
        // Set to mature
        if let PlotState::Planted { growth_stage, .. } =
            &mut garden.get_plot_mut(0).unwrap().state
        {
            *growth_stage = 1.0;
        }
        let actions = ui.available_actions(&garden);
        assert!(actions.contains(&GardenAction::Harvest));
        assert!(actions.contains(&GardenAction::Breed));
    }

    #[test]
    fn test_plot_displays() {
        let mut ui = GardenUiState::new();
        ui.selected_plot = Some(0);
        let garden = make_garden_with_plant();

        let displays = ui.get_plot_displays(&garden);
        assert_eq!(displays.len(), 4);

        assert!(!displays[0].is_empty);
        assert!(displays[0].is_selected);
        assert_eq!(displays[0].species_name, Some("Test Herb".to_string()));

        assert!(displays[1].is_empty);
        assert!(!displays[1].is_selected);
    }

    #[test]
    fn test_plot_display_formatting() {
        let display = PlotDisplay {
            index: 0,
            species_name: Some("Herb".to_string()),
            growth_stage: 0.75,
            is_mature: false,
            is_empty: false,
            watered: true,
            health: 0.9,
            is_selected: false,
        };
        assert_eq!(display.growth_percent(), "75%");
        assert_eq!(display.health_percent(), "90%");
    }
}
