use apothecarys_botany::genetics::PlantGenotype;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A live plant instance growing in a garden plot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlantInstance {
    pub id: Uuid,
    pub genotype: PlantGenotype,
    pub species_name: String,
    pub generation: u32,
    pub parent_ids: Option<(Uuid, Uuid)>,
}

impl PlantInstance {
    pub fn new_wild(genotype: PlantGenotype, species_name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            genotype,
            species_name: species_name.into(),
            generation: 0,
            parent_ids: None,
        }
    }

    pub fn new_bred(
        genotype: PlantGenotype,
        species_name: impl Into<String>,
        generation: u32,
        parent_a: Uuid,
        parent_b: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            genotype,
            species_name: species_name.into(),
            generation,
            parent_ids: Some((parent_a, parent_b)),
        }
    }
}

/// State of a garden plot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlotState {
    Empty,
    Planted {
        plant: Box<PlantInstance>,
        growth_stage: f32,
        watered: bool,
        health: f32,
    },
}

impl PlotState {
    pub fn is_empty(&self) -> bool {
        matches!(self, PlotState::Empty)
    }

    pub fn is_mature(&self) -> bool {
        matches!(self, PlotState::Planted { growth_stage, .. } if *growth_stage >= 1.0)
    }

    pub fn plant(&self) -> Option<&PlantInstance> {
        match self {
            PlotState::Planted { plant, .. } => Some(plant),
            PlotState::Empty => None,
        }
    }
}

/// A single garden plot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GardenPlot {
    pub index: usize,
    pub state: PlotState,
}

impl GardenPlot {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            state: PlotState::Empty,
        }
    }

    /// Plant a seed with the given genotype.
    pub fn plant_seed(&mut self, plant: PlantInstance) -> Result<(), GardenError> {
        if !self.state.is_empty() {
            return Err(GardenError::PlotOccupied(self.index));
        }
        self.state = PlotState::Planted {
            plant: Box::new(plant),
            growth_stage: 0.0,
            watered: false,
            health: 1.0,
        };
        Ok(())
    }

    /// Water this plot.
    pub fn water(&mut self) -> Result<(), GardenError> {
        match &mut self.state {
            PlotState::Planted { watered, .. } => {
                *watered = true;
                Ok(())
            }
            PlotState::Empty => Err(GardenError::PlotEmpty(self.index)),
        }
    }

    /// Harvest a mature plant, returning its instance and clearing the plot.
    pub fn harvest(&mut self) -> Result<PlantInstance, GardenError> {
        if !self.state.is_mature() {
            return Err(GardenError::PlantNotMature(self.index));
        }
        match std::mem::replace(&mut self.state, PlotState::Empty) {
            PlotState::Planted { plant, .. } => Ok(*plant),
            PlotState::Empty => Err(GardenError::PlotEmpty(self.index)),
        }
    }

    /// Remove a plant (dead or otherwise), clearing the plot.
    pub fn clear(&mut self) -> Option<PlantInstance> {
        match std::mem::replace(&mut self.state, PlotState::Empty) {
            PlotState::Planted { plant, .. } => Some(*plant),
            PlotState::Empty => None,
        }
    }
}

/// The garden containing all plots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Garden {
    pub plots: Vec<GardenPlot>,
    pub max_plots: usize,
}

impl Garden {
    pub fn new(initial_plots: usize) -> Self {
        let plots = (0..initial_plots).map(GardenPlot::new).collect();
        Self {
            plots,
            max_plots: initial_plots + 4, // Room to expand
        }
    }

    /// Add a new plot if under the max limit.
    pub fn unlock_plot(&mut self) -> Result<usize, GardenError> {
        if self.plots.len() >= self.max_plots {
            return Err(GardenError::MaxPlotsReached);
        }
        let index = self.plots.len();
        self.plots.push(GardenPlot::new(index));
        Ok(index)
    }

    /// Upgrade the max plot limit.
    pub fn upgrade_capacity(&mut self, additional: usize) {
        self.max_plots += additional;
    }

    /// Get a plot by index.
    pub fn get_plot(&self, index: usize) -> Result<&GardenPlot, GardenError> {
        self.plots.get(index).ok_or(GardenError::InvalidPlot(index))
    }

    /// Get a mutable plot by index.
    pub fn get_plot_mut(&mut self, index: usize) -> Result<&mut GardenPlot, GardenError> {
        self.plots
            .get_mut(index)
            .ok_or(GardenError::InvalidPlot(index))
    }

    /// Count of plots with growing plants.
    pub fn planted_count(&self) -> usize {
        self.plots
            .iter()
            .filter(|p| !p.state.is_empty())
            .count()
    }

    /// Count of mature plants ready for harvest or breeding.
    pub fn mature_count(&self) -> usize {
        self.plots.iter().filter(|p| p.state.is_mature()).count()
    }

    /// Water all plots that have plants.
    pub fn water_all(&mut self) {
        for plot in &mut self.plots {
            if let PlotState::Planted { watered, .. } = &mut plot.state {
                *watered = true;
            }
        }
    }
}

/// Errors from garden operations.
#[derive(Debug, Clone, PartialEq)]
pub enum GardenError {
    PlotOccupied(usize),
    PlotEmpty(usize),
    PlantNotMature(usize),
    InvalidPlot(usize),
    MaxPlotsReached,
    PlotsNotAdjacent(usize, usize),
    PlantNotMatureForBreeding(usize),
}

impl std::fmt::Display for GardenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GardenError::PlotOccupied(i) => write!(f, "Plot {i} is already occupied"),
            GardenError::PlotEmpty(i) => write!(f, "Plot {i} is empty"),
            GardenError::PlantNotMature(i) => write!(f, "Plant in plot {i} is not yet mature"),
            GardenError::InvalidPlot(i) => write!(f, "Plot {i} does not exist"),
            GardenError::MaxPlotsReached => write!(f, "Maximum plot count reached"),
            GardenError::PlotsNotAdjacent(a, b) => write!(f, "Plots {a} and {b} are not adjacent"),
            GardenError::PlantNotMatureForBreeding(i) => {
                write!(f, "Plant in plot {i} is not mature enough for breeding")
            }
        }
    }
}

impl std::error::Error for GardenError {}

#[cfg(test)]
mod tests {
    use super::*;
    use apothecarys_botany::genetics::PlantGenotype;
    use rand::SeedableRng;

    fn make_plant() -> PlantInstance {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Test Plant")
    }

    #[test]
    fn test_new_garden() {
        let garden = Garden::new(4);
        assert_eq!(garden.plots.len(), 4);
        assert_eq!(garden.max_plots, 8);
        assert_eq!(garden.planted_count(), 0);
    }

    #[test]
    fn test_plant_seed() {
        let mut garden = Garden::new(4);
        let plant = make_plant();

        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();
        assert_eq!(garden.planted_count(), 1);
        assert!(!garden.get_plot(0).unwrap().state.is_empty());
    }

    #[test]
    fn test_plant_in_occupied_plot_fails() {
        let mut garden = Garden::new(4);
        let plant1 = make_plant();
        let plant2 = make_plant();

        garden.get_plot_mut(0).unwrap().plant_seed(plant1).unwrap();
        let result = garden.get_plot_mut(0).unwrap().plant_seed(plant2);
        assert_eq!(result, Err(GardenError::PlotOccupied(0)));
    }

    #[test]
    fn test_water_plot() {
        let mut garden = Garden::new(4);
        let plant = make_plant();

        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();
        garden.get_plot_mut(0).unwrap().water().unwrap();

        if let PlotState::Planted { watered, .. } = &garden.get_plot(0).unwrap().state {
            assert!(*watered);
        } else {
            panic!("Expected planted state");
        }
    }

    #[test]
    fn test_water_empty_plot_fails() {
        let mut garden = Garden::new(4);
        let result = garden.get_plot_mut(0).unwrap().water();
        assert_eq!(result, Err(GardenError::PlotEmpty(0)));
    }

    #[test]
    fn test_harvest_mature_plant() {
        let mut garden = Garden::new(4);
        let plant = make_plant();
        let expected_name = plant.species_name.clone();

        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();

        // Manually set to mature
        if let PlotState::Planted { growth_stage, .. } =
            &mut garden.get_plot_mut(0).unwrap().state
        {
            *growth_stage = 1.0;
        }

        let harvested = garden.get_plot_mut(0).unwrap().harvest().unwrap();
        assert_eq!(harvested.species_name, expected_name);
        assert!(garden.get_plot(0).unwrap().state.is_empty());
    }

    #[test]
    fn test_harvest_immature_plant_fails() {
        let mut garden = Garden::new(4);
        let plant = make_plant();
        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();

        let result = garden.get_plot_mut(0).unwrap().harvest();
        assert_eq!(result, Err(GardenError::PlantNotMature(0)));
    }

    #[test]
    fn test_unlock_plot() {
        let mut garden = Garden::new(4);
        let index = garden.unlock_plot().unwrap();
        assert_eq!(index, 4);
        assert_eq!(garden.plots.len(), 5);
    }

    #[test]
    fn test_unlock_plot_at_max_fails() {
        let mut garden = Garden::new(4);
        garden.max_plots = 4; // Already at max
        let result = garden.unlock_plot();
        assert_eq!(result, Err(GardenError::MaxPlotsReached));
    }

    #[test]
    fn test_upgrade_capacity() {
        let mut garden = Garden::new(4);
        assert_eq!(garden.max_plots, 8);
        garden.upgrade_capacity(4);
        assert_eq!(garden.max_plots, 12);
    }

    #[test]
    fn test_water_all() {
        let mut garden = Garden::new(4);
        let plant1 = make_plant();
        let plant2 = make_plant();

        garden.get_plot_mut(0).unwrap().plant_seed(plant1).unwrap();
        garden.get_plot_mut(2).unwrap().plant_seed(plant2).unwrap();

        garden.water_all();

        for plot in &garden.plots {
            if let PlotState::Planted { watered, .. } = &plot.state {
                assert!(*watered);
            }
        }
    }

    #[test]
    fn test_clear_plot() {
        let mut garden = Garden::new(4);
        let plant = make_plant();
        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();

        let cleared = garden.get_plot_mut(0).unwrap().clear();
        assert!(cleared.is_some());
        assert!(garden.get_plot(0).unwrap().state.is_empty());
    }

    #[test]
    fn test_mature_count() {
        let mut garden = Garden::new(4);
        let plant1 = make_plant();
        let plant2 = make_plant();

        garden.get_plot_mut(0).unwrap().plant_seed(plant1).unwrap();
        garden.get_plot_mut(1).unwrap().plant_seed(plant2).unwrap();

        // Set one to mature
        if let PlotState::Planted { growth_stage, .. } =
            &mut garden.get_plot_mut(0).unwrap().state
        {
            *growth_stage = 1.0;
        }

        assert_eq!(garden.mature_count(), 1);
    }

    #[test]
    fn test_plant_instance_wild() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let plant = PlantInstance::new_wild(PlantGenotype::random_wild(&mut rng), "Wild Herb");
        assert_eq!(plant.generation, 0);
        assert!(plant.parent_ids.is_none());
    }

    #[test]
    fn test_plant_instance_bred() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let parent_a = Uuid::new_v4();
        let parent_b = Uuid::new_v4();
        let plant = PlantInstance::new_bred(
            PlantGenotype::random_wild(&mut rng),
            "Hybrid",
            2,
            parent_a,
            parent_b,
        );
        assert_eq!(plant.generation, 2);
        assert_eq!(plant.parent_ids, Some((parent_a, parent_b)));
    }

    #[test]
    fn test_garden_serde_roundtrip() {
        let mut garden = Garden::new(4);
        let plant = make_plant();
        garden.get_plot_mut(0).unwrap().plant_seed(plant).unwrap();

        let json = serde_json::to_string(&garden).unwrap();
        let deserialized: Garden = serde_json::from_str(&json).unwrap();
        assert_eq!(garden, deserialized);
    }
}
